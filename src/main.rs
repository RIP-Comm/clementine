use std::{env, process};
extern crate ui;
use ui::app::ClementineApp;
extern crate logger;
use logger::{init_logger, log, LogKind};

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<String>>();

    if args.len() > 1 {
        let arg = args.remove(0);
        if arg.as_str() == "--log-on-file" {
            init_logger(LogKind::FILE);
        } else {
            eprintln!("arguments not recognized.");
            process::exit(1);
        }
    } else {
        init_logger(LogKind::STDOUT);
    }

    log("clementine v0.1.0");

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
        drag_and_drop_support: true,
        initial_window_size: Some([1200.0, 800.0].into()),
        ..Default::default()
    };

    eframe::run_native(
        "Clementine - A GBA Emulator",
        options,
        Box::new(|_cc| Box::new(ClementineApp::new(cartridge_name))),
    )
    .ok();
}
