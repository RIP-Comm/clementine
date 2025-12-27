use std::sync::{Arc, Mutex};

use crate::emu_thread::EmuHandle;
use crate::ui_traits::UiTool;

pub struct CpuRegisters {
    emu_handle: Arc<Mutex<EmuHandle>>,
    base_kind: BaseKind,
}

#[derive(PartialEq)]
enum BaseKind {
    Hex,
    Dec,
}

impl CpuRegisters {
    pub const fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self {
            emu_handle,
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
            .open(open)
            .default_pos(egui::pos2(1800.0 - 100.0, 10.0))
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

        // Read registers from cached state
        let registers = self
            .emu_handle
            .lock()
            .map_or([0u32; 16], |handle| handle.state.registers);

        egui::Grid::new("ARM Registers")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for (index, reg) in registers.iter().enumerate() {
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
                }
            });
    }
}
