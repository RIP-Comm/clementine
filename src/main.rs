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
//! │     └─► cargo run -- <rom_path> [--log-to-file]                            │
//! │                                                                             │
//! │  2. Initialize tracing subscriber                                          │
//! │     └─► No output by default, or file with --log-to-file                   │
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

use tracing::info;
use tracing_subscriber::fmt::writer::MakeWriterExt;

/// Entry point for the Clementine GBA emulator.
///
/// Parses command-line arguments, initializes logging,
/// loads the ROM, and starts the UI event loop.
///
/// # Arguments (via command line)
///
/// - `<rom_path>` - Path to a .gba ROM file (required)
/// - `--log-to-file` - Write logs to file in temp directory (optional, no logging by default)
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let log_to_file = args.iter().any(|arg| arg == "--log-to-file");
    if log_to_file {
        let log_path = std::env::temp_dir().join("clementine.log");
        let file_appender =
            tracing_appender::rolling::never(std::env::temp_dir(), "clementine.log");
        tracing_subscriber::fmt()
            .with_writer(file_appender.with_max_level(tracing::Level::DEBUG))
            .with_ansi(false)
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .init();
        println!("Logging to: {}", log_path.display());
    }

    let rom_args: Vec<&String> = args.iter().filter(|arg| *arg != "--log-to-file").collect();

    let cartridge_name = rom_args.first().map_or_else(
        || {
            eprintln!("Usage: clementine <rom_path> [--log-to-file]");
            eprintln!("Error: no cartridge provided");
            std::process::exit(1)
        },
        |name| {
            info!("Loading ROM: {name}");
            (*name).clone()
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
