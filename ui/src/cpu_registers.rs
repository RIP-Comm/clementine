use emu::gba::Gba;

use crate::ui_traits::UiTool;

use std::sync::{Arc, Mutex};

pub struct CpuRegisters {
    gba: Arc<Mutex<Gba>>,
}

impl CpuRegisters {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self { gba }
    }
}

impl UiTool for CpuRegisters {
    fn name(&self) -> &'static str {
        "Cpu Registers"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(320.0)
            .open(open)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Registers");
        ui.add_space(8.0);

        // If it's poisoned it means that the thread that executes instructions
        // panicked, we still want access to registers to debug
        let registers = self.gba.lock().map_or_else(
            |poisoned| poisoned.into_inner().cpu.registers.to_vec(),
            |gba| gba.cpu.registers.to_vec(),
        );

        let mut index = 0;

        egui::Grid::new("ARM Registers")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for reg in registers {
                    let mut value = reg.to_string();
                    ui.label(if index == 15 {
                        format!("R{index:?} (PC)")
                    } else {
                        format!("R{index:?}")
                    });
                    ui.text_edit_singleline(&mut value);

                    ui.end_row();
                    index += 1;
                }
            });
    }
}
