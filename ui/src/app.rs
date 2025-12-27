//! # Clementine UI Application
//!
//! This module contains the main application struct that orchestrates
//! the emulator UI and ties together all the components.
//!
//! ## Initialization Flow
//!
//! When [`App::new`] is called:
//!
//! ```text
//! App::new(cartridge_name)
//!     │
//!     ├─► read_file(cartridge_name)
//!     │   └─► Load entire ROM into Vec<u8>
//!     │
//!     ├─► Load BIOS from ./gba_bios.bin
//!     │   └─► Must be exactly 16KB (0x4000 bytes)
//!     │
//!     ├─► CartridgeHeader::new(&rom_data)
//!     │   └─► Parse header, validate checksum
//!     │
//!     ├─► Gba::new(header, bios, rom)
//!     │   ├─► InternalMemory::new(bios, rom)
//!     │   ├─► Bus::with_memory(memory)
//!     │   └─► Arm7tdmi::new(bus)
//!     │
//!     └─► Create UI tools:
//!         ├─► About (version info)
//!         ├─► CpuRegisters (register viewer)
//!         ├─► CpuHandler (run/pause/step controls)
//!         ├─► GbaDisplay (LCD output)
//!         ├─► SaveGame (save/load state)
//!         └─► Disassembler (if feature enabled)
//! ```
//!
//! ## Shared State
//!
//! The [`Gba`] instance is wrapped in `Arc<Mutex<Gba>>` so multiple UI
//! components can access it safely. Each UI tool receives a clone of
//! this Arc and locks the mutex when it needs to read or modify state.
//!
//! ## UI Tools
//!
//! Each UI component implements the [`UiTool`] trait, which provides:
//! - `name()` - Display name for the tool panel
//! - `show()` - Render the tool's UI
//!
//! Tools can be toggled on/off via the sidebar checkboxes.

#[cfg(feature = "disassembler")]
use crate::disassembler::Disassembler;
use emu::{cartridge_header::CartridgeHeader, gba::Gba};
use std::io::Read;

use super::cpu_registers::CpuRegisters;
use crate::{
    about, cpu_handler::CpuHandler, gba_display::GbaDisplay, savegame::SaveGame, ui_traits::UiTool,
};

use std::{
    collections::BTreeSet,
    error,
    sync::{Arc, Mutex},
};

/// The main Clementine application.
///
/// Holds the emulator state (via UI tools that reference `Arc<Mutex<Gba>>`)
/// and manages which tool windows are currently open.
///
/// ## Creating the Application
///
/// ```no_run
/// use ui::app::App;
///
/// let bios_data: Vec<u8> = std::fs::read("gba_bios.bin").unwrap();
/// let app = App::new(&bios_data, "path/to/game.gba".to_string());
/// // Then pass to eframe::run_native()
/// ```
///
/// ## How It Works
///
/// 1. On creation, receives BIOS data and loads ROM to initialize the GBA
/// 2. Creates UI tool windows that share access to the GBA via `Arc<Mutex<Gba>>`
/// 3. In the update loop, each tool renders and may step the CPU
/// 4. The `GbaDisplay` tool is responsible for actually running the CPU
pub struct App {
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
        let cartridge_header = CartridgeHeader::new(cartridge_data).unwrap();
            cartridge_header,
            bios_data[0..0x0000_4000].try_into().unwrap(),
            data,
        )));

        #[cfg(feature = "disassembler")]
        let disassembler = Disassembler::new(Arc::clone(&arc_gba));

        let tools: Vec<Box<dyn UiTool>> = vec![
            Box::<about::About>::default(),
            Box::new(CpuRegisters::new(Arc::clone(&arc_gba))),
            Box::new(CpuHandler::new(Arc::clone(&arc_gba))),
            Box::new(GbaDisplay::new(Arc::clone(&arc_gba))),
            Box::new(SaveGame::new(Arc::clone(&arc_gba))),
        ];

        #[cfg(feature = "disassembler")]
        let mut tools = tools;
        #[cfg(feature = "disassembler")]
        tools.push(Box::new(disassembler));

        Self::from_tools(tools)
    }

    fn from_tools(tools: Vec<Box<dyn UiTool>>) -> Self {
        let mut open = BTreeSet::new();

        open.insert(tools[1].name().to_owned());
        open.insert(tools[2].name().to_owned());
        open.insert(tools[3].name().to_owned());
        open.insert(tools[4].name().to_owned());
        #[cfg(feature = "disassembler")]
        open.insert(tools[5].name().to_owned());

        Self { tools, open }
    }

    pub fn checkboxes(&mut self, ui: &mut egui::Ui) {
        let Self { tools, open } = self;
        for tool in tools {
            let mut is_open = open.contains(tool.name());
            ui.toggle_value(&mut is_open, tool.name());
            set_open(open, tool.name(), is_open);
        }
    }

    fn windows(&mut self, ctx: &egui::Context) {
        let Self { tools, open } = self;
        for tool in tools {
            let mut is_open = open.contains(tool.name());
            tool.show(ctx, &mut is_open);
            set_open(open, tool.name(), is_open);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

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

fn read_file(filepath: String) -> Result<Vec<u8>, Box<dyn error::Error>> {
    let mut f = std::fs::File::open(filepath)?;
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;

    Ok(buf)
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
