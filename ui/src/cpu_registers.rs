use emu::gba::Gba;

use crate::ui_traits::UiTool;

use std::sync::{Arc, Mutex};

pub struct CpuRegisters {
    gba: Arc<Mutex<Gba>>,
    base_kind: BaseKind,
}

#[derive(PartialEq)]
enum BaseKind {
    Hex,
    Dec,
}

impl CpuRegisters {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self {
            gba,
            base_kind: BaseKind::Hex,
        }
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

        ui.radio_value(&mut self.base_kind, BaseKind::Dec, "Decimal");
        ui.radio_value(&mut self.base_kind, BaseKind::Hex, "Hexadecimal");
        ui.add_space(8.0);

        let registers = self
            .gba
            .lock()
            .unwrap()
            .cpu
            .lock()
            .unwrap()
            .registers
            .to_vec();

        let mut index = 0;

        egui::Grid::new("ARM Registers")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for reg in registers {
                    let mut value = match self.base_kind {
                        BaseKind::Dec => reg.to_string(),
                        BaseKind::Hex => format!("0x{reg:x}"),
                    };

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
