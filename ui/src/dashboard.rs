use egui::Context;
use emu::{cartridge_header::CartridgeHeader, gba::Gba};
use logger::log;

use super::cpu_inspector::CpuInspector;
use crate::{
    about, gba_display::GbaDisplay, palette_visualizer::PaletteVisualizer, ui_traits::UiTool,
};

use std::{
    collections::BTreeSet,
    env, error, fs,
    io::Read,
    sync::{Arc, Mutex},
};

// ----------------------------------------------------------------------------

pub struct UiTools {
    tools: Vec<Box<dyn UiTool>>,
    open: BTreeSet<String>,
}

impl UiTools {
    pub fn new(cartridge_name: String) -> Self {
        let data = match read_file(&cartridge_name) {
            Ok(d) => d,
            Err(e) => {
                log(format!("{e}"));
                std::process::exit(2);
            }
        };

        let bios_file = env::current_dir().unwrap().join("gba_bios.bin");
        let bios = match std::fs::read(bios_file) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("can't open bios file: {e}");
                std::process::exit(3);
            }
        };

        let cartridge_header =
            CartridgeHeader::new(data.as_slice()).expect("Cartridge must be opened");
        let arc_gba = Arc::new(Mutex::new(Gba::new(
            cartridge_header,
            bios[0..0x00004000].try_into().unwrap(),
            data,
        )));

        Self::from_tools(vec![
            Box::<about::About>::default(),
            Box::new(CpuInspector::new(Arc::clone(&arc_gba))),
            Box::new(GbaDisplay::new(Arc::clone(&arc_gba))),
            Box::new(PaletteVisualizer::new(arc_gba)),
        ])
    }

    pub fn from_tools(tools: Vec<Box<dyn UiTool>>) -> Self {
        let mut open = BTreeSet::new();

        // Here the default opened window
        open.insert(tools[1].name().to_owned());
        open.insert(tools[2].name().to_owned());

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

    pub fn windows(&mut self, ctx: &Context) {
        let Self { tools, open } = self;
        for tool in tools {
            let mut is_open = open.contains(tool.name());
            tool.show(ctx, &mut is_open);
            set_open(open, tool.name(), is_open);
        }
    }
}

// ----------------------------------------------------------------------------

fn set_open(open: &mut BTreeSet<String>, key: &'static str, is_open: bool) {
    if is_open {
        if !open.contains(key) {
            open.insert(key.to_owned());
        }
    } else {
        open.remove(key);
    }
}

// ----------------------------------------------------------------------------

pub struct Dashboard {
    ui_tools: UiTools,
}

impl Dashboard {
    pub fn new(cartridge_name: String) -> Self {
        Self {
            ui_tools: UiTools::new(cartridge_name),
        }
    }

    pub fn ui(&mut self, ctx: &Context) {
        egui::SidePanel::right("Clementine Tools")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| {
                egui::trace!(ui);
                ui.vertical_centered(|ui| {
                    ui.heading("✒ Clementine Tools");
                });

                ui.separator();
                ui.label("Links");
                use egui::special_emojis::GITHUB;
                ui.hyperlink_to(
                    format!("{} Clementine", GITHUB),
                    "https://github.com/RIP-Comm/clementine",
                );

                ui.separator();

                self.ui_tools.checkboxes(ui);
            });

        self.show_windows(ctx);
    }

    fn show_windows(&mut self, ctx: &Context) {
        self.ui_tools.windows(ctx);
    }
}

fn read_file(filepath: &str) -> Result<Vec<u8>, Box<dyn error::Error>> {
    let mut f = fs::File::open(filepath)?;
    let mut buf = vec![];
    f.read_to_end(&mut buf)?;

    Ok(buf)
}
