#[cfg(feature = "disassembler")]
use crate::disassembler::Disassembler;
use emu::{cartridge_header::CartridgeHeader, gba::Gba};
use logger::log;
use std::io::Read;

use super::cpu_registers::CpuRegisters;
use crate::{
    about, cpu_handler::CpuHandler, gba_display::GbaDisplay, savegame::SaveGame, ui_traits::UiTool,
};

use std::{
    collections::BTreeSet,
    env, error,
    sync::{Arc, Mutex},
};

pub struct ClementineApp {
    tools: Vec<Box<dyn UiTool>>,
    open: BTreeSet<String>,
}

impl ClementineApp {
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
}

impl eframe::App for ClementineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

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
                    format!("{GITHUB} Clementine"),
                    "https://github.com/RIP-Comm/clementine",
                );

                ui.separator();

                self.checkboxes(ui);
            });

        self.windows(ctx);
    }
}

impl ClementineApp {
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

fn read_file(filepath: &str) -> Result<Vec<u8>, Box<dyn error::Error>> {
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
