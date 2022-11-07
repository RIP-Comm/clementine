use emu::{cpu::Cpu, gba::Gba};

use crate::ui_traits::{UiTool, View};

use std::sync::{Arc, Mutex};

pub struct CpuInspector {
    gba: Arc<Mutex<Gba>>,
    play: bool,
}

impl CpuInspector {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self { gba, play: false }
    }
}

impl UiTool for CpuInspector {
    fn name(&self) -> &'static str {
        "Cpu Inspector"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(320.0)
            .open(open)
            .show(ctx, |ui| {
                use View as _;
                self.ui(ui);
            });
    }
}

impl View for CpuInspector {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("MODE: Arm7tdmi");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let mut cartridge_name: String = Default::default();
            if let Ok(gba) = self.gba.lock() {
                cartridge_name = gba.cartridge_header.game_title.clone()
            }
            ui.text_edit_singleline(&mut cartridge_name);
            if ui.button("▶").clicked() {
                // Start a thread for gameboy execution
                todo!("Start a thread for a background gameboy execution");
            }
            if ui.button("⏸ ").clicked() {
                self.play = false;
            }
            if ui.button("⏭").clicked() {
                if let Ok(mut gba) = self.gba.lock() {
                    gba.cpu.step()
                }
            }
        });
        ui.add_space(12.0);
        ui.heading("Registers");
        ui.add_space(8.0);

        let registers = self
            .gba
            .lock()
            .map_or_else(|_| vec![], |gba| gba.cpu.registers.to_vec());

        let mut index = 0;

        egui::Grid::new("ARM Registers")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for reg in registers {
                    let mut value = reg.to_string();
                    ui.label(if index == 15 {
                        format!("R{:?} (PC)", index)
                    } else {
                        format!("R{:?}", index)
                    });
                    ui.text_edit_singleline(&mut value);

                    ui.end_row();
                    index += 1;
                }
            });
    }
}
