extern crate logger;
extern crate ui;
use logger::log;

#[cfg(feature = "logger")]
use logger::{init_logger, LogKind};

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
        Box::new(|_cc| Ok(Box::new(ui::app::ClementineApp::new(cartridge_name)))),
    )
    .ok();
}
