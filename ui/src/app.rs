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

use crate::audio::AudioPlayer;
use crate::disassembler::Disassembler;
use crate::emu_thread::{self, EmuCommand, EmuHandle, GbaButton};
use crate::keypad_debug::KeypadDebug;
use crate::memory_inspector::MemoryInspector;
use crate::pokemon_debugger::PokemonDebugger;
use crate::rom_info::RomInfo;
use crate::sound_controls::SoundControls;
use emu::gba::Gba;

use super::cpu_registers::CpuRegisters;
use crate::{
    about, cpu_handler::CpuHandler, gba_display::GbaDisplay, savegame::SaveGame, ui_traits::UiTool,
};

use egui::Key;
use std::{
    collections::BTreeSet,
    path::PathBuf,
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
/// let app = App::new(&bios_data, &cartridge_data, None);
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
    /// Kept alive to keep audio playing, `None` when no output device.
    _audio: Option<crate::audio::AudioPlayer>,
    /// True while the fast-forward key is held.
    fast_forwarding: bool,
    /// Speed to restore when the fast-forward key is released.
    saved_speed: f32,
    /// Key that triggers hold-to-fast-forward, configurable from the speed panel.
    fast_forward_key: Arc<Mutex<Key>>,
    /// Master volume and mute handle, kept so settings can be saved on exit.
    audio_controls: Option<Arc<crate::audio::AudioControls>>,
    /// Where persistent UI settings are stored.
    settings_path: PathBuf,
}

impl App {
    /// Create a new `ClementineApp` instance
    ///
    /// # Panics
    /// It panics if the cartridge can't be opened.
    #[must_use]
    pub fn new(bios_data: &[u8], cartridge_data: &[u8], battery_path: Option<PathBuf>) -> Self {
        let mut gba = Gba::new(
            bios_data[0..0x0000_4000].try_into().unwrap(),
            cartridge_data,
        );

        // Take consumer for disassembler channel before spawning thread
        let disasm_rx = gba.disasm_rx.take().expect("disasm_rx should be present");

        // Start audio before the emulator moves onto its thread.
        // The ring holds ~0.5s of stereo samples so brief scheduling hiccups do not underrun.
        let audio = crate::audio::start(|rate| gba.init_audio(rate, 1 << 15));
        let audio_controls = audio.as_ref().map(AudioPlayer::controls);

        // Spawn the emulator thread and get the handle
        let emu_handle = Arc::new(Mutex::new(emu_thread::spawn(gba, disasm_rx, battery_path)));

        // Restore persisted settings and apply them.
        let settings_path = crate::settings::Settings::path();
        let settings = crate::settings::Settings::load(&settings_path);
        if let Some(controls) = &audio_controls {
            controls.set_volume(settings.volume);
            controls.set_muted(settings.muted);
        }
        if let Ok(mut handle) = emu_handle.lock() {
            handle.send(EmuCommand::SetSpeed(settings.speed));
        }

        // Fast-forward key is shared so the speed panel can rebind it while the
        // input handler reads it each frame.
        let fast_forward_key = Arc::new(Mutex::new(settings.fast_forward_key()));

        let tools: Vec<Box<dyn UiTool>> = vec![
            Box::new(RomInfo::new(Arc::clone(&emu_handle))),
            Box::new(SaveGame::new(Arc::clone(&emu_handle))),
            Box::new(CpuHandler::new(
                Arc::clone(&emu_handle),
                Arc::clone(&fast_forward_key),
            )),
            Box::new(GbaDisplay::new(Arc::clone(&emu_handle))),
            Box::new(CpuRegisters::new(Arc::clone(&emu_handle))),
            Box::new(Disassembler::new(Arc::clone(&emu_handle))),
            Box::new(KeypadDebug::new(Arc::clone(&emu_handle))),
            Box::new(MemoryInspector::new(Arc::clone(&emu_handle))),
            Box::new(PokemonDebugger::new(Arc::clone(&emu_handle))),
            Box::new(SoundControls::new(audio_controls.clone())),
            Box::<about::About>::default(),
        ];

        // Open only the game view and the run controls at launch. The other
        // tools are debug panels the user can toggle on from the sidebar, so
        // showing all of them at once just buries the display under windows.
        let mut open = BTreeSet::new();
        for name in [
            "Gba Display",
            "Cpu Handler",
            "ROM Info",
            "Save Game",
            "Sound",
        ] {
            open.insert(name.to_owned());
        }

        Self {
            emu_handle,
            tools,
            open,
            _audio: audio,
            fast_forwarding: false,
            saved_speed: 1.0,
            fast_forward_key,
            audio_controls,
            settings_path,
        }
    }

    /// Persist the current UI settings to disk.
    fn save_settings(&self) {
        let (volume, muted) = self
            .audio_controls
            .as_ref()
            .map_or((1.0, false), |controls| {
                (controls.volume(), controls.muted())
            });
        let speed = self.emu_handle.lock().map_or(1.0, |handle| handle.speed);
        let fast_forward_key = self
            .fast_forward_key
            .lock()
            .map_or(Key::Space, |key| *key)
            .name()
            .to_string();

        let settings = crate::settings::Settings {
            fast_forward_key,
            speed,
            volume,
            muted,
        };
        settings.save(&self.settings_path);
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

impl Drop for App {
    fn drop(&mut self) {
        self.save_settings();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Poll the emulator for new events (frames, state updates, etc.)
        let is_running = self.emu_handle.lock().is_ok_and(|mut handle| {
            handle.poll();
            handle.state.is_running
        });

        // Drive repaints at ~60 FPS only while the emulator is running, so a new
        // frame shows up promptly. When it is paused there is nothing animating,
        // so fall back to a slow tick that still picks up asynchronous events
        // (a finished step, a load result) without spinning a core at 60 FPS.
        // egui also repaints on its own whenever there is user interaction.
        let repaint_after = if is_running { 16 } else { 200 };
        ctx.request_repaint_after(std::time::Duration::from_millis(repaint_after));

        self.handle_input(&ctx);

        egui::Panel::right("Clementine Tools")
            .resizable(false)
            .default_size(200.0)
            .show(ui, |ui| {
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

        self.windows(&ctx);
    }
}

impl App {
    /// Handle keyboard input and send button commands to the emulator.
    fn handle_input(&mut self, ctx: &egui::Context) {
        const FAST_FORWARD_SPEED: f32 = 4.0;
        const KEY_MAPPINGS: &[(Key, GbaButton)] = &[
            (Key::Z, GbaButton::A),
            (Key::X, GbaButton::B),
            (Key::Enter, GbaButton::Start),
            (Key::Backspace, GbaButton::Select),
            (Key::ArrowUp, GbaButton::Up),
            (Key::ArrowDown, GbaButton::Down),
            (Key::ArrowLeft, GbaButton::Left),
            (Key::ArrowRight, GbaButton::Right),
            (Key::A, GbaButton::L),
            (Key::S, GbaButton::R),
        ];

        let fast_forward_key = self.fast_forward_key.lock().map_or(Key::Space, |key| *key);

        let (toggle_fullscreen, is_fullscreen, fast_forward_pressed, fast_forward_released) = ctx
            .input(|input| {
                for &(key, button) in KEY_MAPPINGS {
                    if input.key_pressed(key)
                        && let Ok(mut handle) = self.emu_handle.lock()
                    {
                        handle.send(EmuCommand::SetKey {
                            button,
                            pressed: true,
                        });
                    }
                    if input.key_released(key)
                        && let Ok(mut handle) = self.emu_handle.lock()
                    {
                        handle.send(EmuCommand::SetKey {
                            button,
                            pressed: false,
                        });
                    }
                }

                (
                    input.key_pressed(Key::F11),
                    input.viewport().fullscreen.unwrap_or(false),
                    input.key_pressed(fast_forward_key),
                    input.key_released(fast_forward_key),
                )
            });

        if toggle_fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
        }

        // Hold the fast-forward key to run at FAST_FORWARD_SPEED, then drop back
        // to whatever speed was set before the key was pressed.
        if fast_forward_pressed && !self.fast_forwarding {
            self.fast_forwarding = true;
            self.saved_speed = self.emu_handle.lock().map_or(1.0, |handle| handle.speed);
            self.send(EmuCommand::SetSpeed(FAST_FORWARD_SPEED));
        } else if fast_forward_released && self.fast_forwarding {
            self.fast_forwarding = false;
            self.send(EmuCommand::SetSpeed(self.saved_speed));
        }
    }

    fn send(&self, cmd: EmuCommand) {
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(cmd);
        }
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
