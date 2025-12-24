//! # Clementine - GBA Emulator Entry Point
//!
//! This is the main entry point for the Clementine GBA emulator.
//!
//! ## How the Emulator Starts
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                        Clementine Startup Flow                              │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │  1. Parse command line arguments                                           │
//! │     └─► cargo run -- <rom_path> [--log-on-file]                            │
//! │                                                                             │
//! │  2. Initialize logger (if 'logger' feature enabled)                        │
//! │     └─► Logs to stdout by default, or file with --log-on-file              │
//! │                                                                             │
//! │  3. Load ROM file from disk                                                │
//! │     └─► Read entire .gba file into memory                                  │
//! │                                                                             │
//! │  4. Create UI Application (see ui::app::App)                               │
//! │     │                                                                       │
//! │     ├─► Load BIOS from ./gba_bios.bin (16KB required)                      │
//! │     │                                                                       │
//! │     ├─► Parse cartridge header (game title, checksum, etc.)                │
//! │     │   └─► See emu::cartridge_header::CartridgeHeader                     │
//! │     │                                                                       │
//! │     ├─► Create GBA instance (see emu::gba::Gba)                            │
//! │     │   ├─► Initialize internal memory with BIOS + ROM                     │
//! │     │   ├─► Create memory bus connecting all hardware                      │
//! │     │   └─► Create ARM7TDMI CPU in Supervisor mode                         │
//! │     │                                                                       │
//! │     └─► Create UI tools (display, registers, controls, etc.)               │
//! │                                                                             │
//! │  5. Run eframe event loop                                                  │
//! │     └─► UI updates trigger CPU steps → LCD renders → Display updates       │
//! │                                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │   main.rs   │────▶│   ui crate  │────▶│  emu crate  │
//! │  (binary)   │     │   (GUI)     │     │ (emulation) │
//! └─────────────┘     └─────────────┘     └─────────────┘
//!                            │                   │
//!                            │                   ▼
//!                            │            ┌─────────────┐
//!                            │            │    Gba      │
//!                            │            │  ┌───────┐  │
//!                            │            │  │ARM7TDMI  │
//!                            │            │  └───┬───┘  │
//!                            │            │      │      │
//!                            │            │  ┌───▼───┐  │
//!                            └───────────▶│  │  Bus  │  │
//!                              (display)  │  └───┬───┘  │
//!                                         │      │      │
//!                                         │  ┌───▼───┐  │
//!                                         │  │Memory │  │
//!                                         │  │ LCD   │  │
//!                                         │  │Timers │  │
//!                                         │  └───────┘  │
//!                                         └─────────────┘
//! ```

extern crate logger;
extern crate ui;
use logger::log;

#[cfg(feature = "logger")]
use logger::{LogKind, init_logger};

/// Entry point for the Clementine GBA emulator.
///
/// Parses command-line arguments, initializes logging (if enabled),
/// loads the ROM, and starts the UI event loop.
///
/// # Arguments (via command line)
///
/// - `<rom_path>` - Path to a .gba ROM file (required)
/// - `--log-on-file` - Write logs to file instead of stdout (optional)
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<String>>();

    #[cfg(feature = "logger")]
    if args.len() > 1 {
        if args.last().unwrap().as_str() == "--log-on-file" {
            init_logger(LogKind::FILE);
        }
    } else {
        init_logger(LogKind::STDOUT);
    }

    let cartridge_name = args.first().map_or_else(
        || {
            log("no cartridge found :(");
            std::process::exit(1)
        },
        |name| {
            log(format!("loading {name}"));
            name.clone()
        },
    );

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Clementine - A GBA Emulator",
        options,
        Box::new(|_cc| Ok(Box::new(ui::app::App::new(cartridge_name)))),
    )
    .ok();
}
