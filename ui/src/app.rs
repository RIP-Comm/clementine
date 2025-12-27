//! # Clementine UI Application
//!
//! This module contains the main application struct that orchestrates
//! the emulator UI and ties together all the components.
//!
//! ## Architecture Overview
//!
//! The emulator runs on a **dedicated CPU thread**, communicating with the UI
//! via lock-free SPSC channels. The UI thread only reads cached state and sends
//! commands - it never blocks on the emulator.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                          CPU Thread                                     │
//! │                                                                         │
//! │   EmuThread::run()                                                      │
//! │         │                                                               │
//! │         ▼                                                               │
//! │   loop {                                                                │
//! │       process_commands()   ◄── receives Run/Pause/Step from UI         │
//! │       if running:                                                       │
//! │           gba.step()                                                    │
//! │           send events      ──► State/Frame to UI                        │
//! │   }                                                                     │
//! └─────────────────────────────────────────────────────────────────────────┘
//!
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                          UI Thread                                      │
//! │                                                                         │
//! │   eframe::run_native()                                                  │
//! │         │                                                               │
//! │         ▼                                                               │
//! │   ┌─────────────────────────────────────────────────────────────────┐  │
//! │   │  App::update() called ~60 times/sec (each frame)                │  │
//! │   │       │                                                         │  │
//! │   │       ▼                                                         │  │
//! │   │  emu_handle.poll()      ◄── drains events, updates cached state │  │
//! │   │                                                                 │  │
//! │   │  for each tool in tools:                                        │  │
//! │   │       tool.show(ctx, open)                                      │  │
//! │   │                                                                 │  │
//! │   │  GbaDisplay::ui() does:                                         │  │
//! │   │       1. read emu_handle.frame  ◄── cached, no lock             │  │
//! │   │       2. draw LCD frame                                         │  │
//! │   │                                                                 │  │
//! │   │  CpuHandler::ui() does:                                         │  │
//! │   │       1. emu_handle.send(Run)   ──► command to CPU thread       │  │
//! │   │                                                                 │  │
//! │   │  Disassembler::ui() does:                                       │  │
//! │   │       1. drain_entries()        ◄── reads from SPSC channel     │  │
//! │   │       2. draw text                                              │  │
//! │   └─────────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Initialization Flow
//!
//! When [`App::new`] is called:
//!
//! ```text
//! App::new(bios_data, cartridge_data)
//!     │
//!     ├─► CartridgeHeader::new(&cartridge_data)
//!     │   └─► Parse header, validate checksum
//!     │
//!     ├─► Gba::new(header, bios, rom)
//!     │   ├─► InternalMemory::new(bios, rom)
//!     │   ├─► Bus::with_memory(memory)
//!     │   ├─► Arm7tdmi::new(bus)
//!     │   └─► Create SPSC channel for disassembler
//!     │
//!     ├─► Take disasm_rx from Gba
//!     │
//!     ├─► emu_thread::spawn(gba, disasm_rx)
//!     │   └─► Returns EmuHandle for UI to communicate with CPU thread
//!     │
//!     └─► Create UI tools:
//!         ├─► About (version info)
//!         ├─► CpuRegisters (register viewer, reads EmuHandle::state)
//!         ├─► CpuHandler (run/pause/step, sends commands via EmuHandle)
//!         ├─► GbaDisplay (LCD output, reads EmuHandle::frame)
//!         ├─► SaveGame (save/load state via EmuHandle commands)
//!         └─► Disassembler (reads from EmuHandle::disasm_rx)
//! ```
//!
//! ## Shared State
//!
//! The [`EmuHandle`] is wrapped in `Arc<Mutex<EmuHandle>>` for sharing between
//! UI tools. The mutex is only held briefly for:
//! - Reading cached state (registers, frame buffer)
//! - Sending commands to the CPU thread
//!
//! The actual emulation runs lock-free on the CPU thread.
//!
//! ## UI Tools
//!
//! Each UI component implements the `UiTool` trait, which provides:
//! - `name()` - Display name for the tool panel
//! - `show()` - Render the tool's UI (calls `ui()` internally)
//!
//! Tools can be toggled on/off via the sidebar checkboxes.
//!
//! [`App::update()`]: eframe::App::update
//! [`EmuHandle`]: crate::emu_thread::EmuHandle

use crate::disassembler::Disassembler;
use crate::emu_thread::{self, EmuHandle};
use emu::cartridge_header::CartridgeHeader;
use emu::gba::Gba;

use super::cpu_registers::CpuRegisters;
use crate::{
    about, cpu_handler::CpuHandler, gba_display::GbaDisplay, savegame::SaveGame, ui_traits::UiTool,
};

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

/// The main Clementine application.
///
/// Holds the emulator handle and manages which tool windows are currently open.
///
/// ## Creating the Application
///
/// ```no_run
/// use ui::app::App;
///
/// let bios_data = std::fs::read("gba_bios.bin").unwrap();
/// let cartridge_data = std::fs::read("path/to/game.gba").unwrap();
/// let app = App::new(&bios_data, &cartridge_data);
/// // Then pass to eframe::run_native()
/// ```
///
/// ## How It Works
///
/// 1. On creation, receives BIOS and cartridge data to initialize the GBA
/// 2. Spawns a dedicated CPU thread that owns the GBA
/// 3. Creates UI tool windows that share access via `Arc<Mutex<EmuHandle>>`
/// 4. In the update loop, polls for events and renders each tool
pub struct App {
    emu_handle: Arc<Mutex<EmuHandle>>,
    tools: Vec<Box<dyn UiTool>>,
    open: BTreeSet<String>,
}

impl App {
    /// Create a new `ClementineApp` instance
    ///
    /// # Panics
    /// It panics if the cartridge can't be opened.
    #[must_use]
    pub fn new(bios_data: &[u8], cartridge_data: &[u8]) -> Self {
        let cartridge_header = CartridgeHeader::new(cartridge_data);
        let mut gba = Gba::new(
            cartridge_header,
            bios_data[0..0x0000_4000].try_into().unwrap(),
            cartridge_data.to_vec(),
        );

        // Take consumer for disassembler channel before spawning thread
        let disasm_rx = gba.disasm_rx.take().expect("disasm_rx should be present");

        // Spawn the emulator thread and get the handle
        let emu_handle = Arc::new(Mutex::new(emu_thread::spawn(gba, disasm_rx)));

        let tools: Vec<Box<dyn UiTool>> = vec![
            Box::new(SaveGame::new(Arc::clone(&emu_handle))),
            Box::new(CpuHandler::new(Arc::clone(&emu_handle))),
            Box::new(GbaDisplay::new(Arc::clone(&emu_handle))),
            Box::new(CpuRegisters::new(Arc::clone(&emu_handle))),
            Box::new(Disassembler::new(Arc::clone(&emu_handle))),
            Box::<about::About>::default(),
        ];

        let mut open = BTreeSet::new();
        for tool in &tools {
            open.insert(tool.name().to_owned());
        }

        Self {
            emu_handle,
            tools,
            open,
        }
    }

    pub fn checkboxes(&mut self, ui: &mut egui::Ui) {
        for tool in &self.tools {
            let mut is_open = self.open.contains(tool.name());
            ui.toggle_value(&mut is_open, tool.name());
            set_open(&mut self.open, tool.name(), is_open);
        }
    }

    fn windows(&mut self, ctx: &egui::Context) {
        for tool in &mut self.tools {
            let mut is_open = self.open.contains(tool.name());
            tool.show(ctx, &mut is_open);
            set_open(&mut self.open, tool.name(), is_open);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        // Poll the emulator for new events (frames, state updates, etc.)
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.poll();
        }

        egui::SidePanel::right("Clementine Tools")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("✒ Clementine Tools");
                });

                ui.separator();
                ui.label("Links");
                ui.hyperlink_to(
                    format!("{} Clementine", egui::special_emojis::GITHUB),
                    "https://github.com/RIP-Comm/clementine",
                );

                ui.separator();

                self.checkboxes(ui);
            });

        self.windows(ctx);
    }
}

fn set_open(open: &mut BTreeSet<String>, key: &'static str, is_open: bool) {
    if is_open {
        if !open.contains(key) {
            open.insert(key.to_owned());
        }
    } else {
        open.remove(key);
    }
}
