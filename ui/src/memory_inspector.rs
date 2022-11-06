use emu::{cpu::Cpu, gba::Gba, memory::io_device::IoDevice};

use crate::ui_traits::{UiTool, View};

use std::{
    borrow::Borrow,
    sync::{Arc, Mutex},
};

pub struct MemoryInspector<T: Cpu> {
    address_string: String,
    value: u8,
    base: Base,
    gba: Arc<Mutex<Gba<T>>>,
}

impl<T: Cpu> MemoryInspector<T> {
    pub fn new(gba: Arc<Mutex<Gba<T>>>) -> Self {
        Self {
            gba,
            address_string: String::from("0"),
            value: 0,
            base: Base::Dec,
        }
    }
}

impl<T: Cpu> UiTool for MemoryInspector<T> {
    fn name(&self) -> &'static str {
        "Memory Inspector"
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

#[derive(PartialEq)]
enum Base {
    Dec,
    Hex,
}

impl<T: Cpu> View for MemoryInspector<T> {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.radio_value(&mut self.base, Base::Dec, "Dec");
        ui.radio_value(&mut self.base, Base::Hex, "Hex");

        ui.horizontal(|ui| {
            ui.label("Memory address:");
            ui.text_edit_singleline(&mut self.address_string);
            if ui.button("Read").clicked() {
                if let Ok(gba) = self.gba.lock() {
                    let radix = match self.base {
                        Base::Dec => 10,
                        Base::Hex => 16,
                    };

                    let address = u32::from_str_radix(&self.address_string, radix).unwrap();
                    self.value = gba.borrow().cpu.get_memory().borrow().read_at(address);
                }
            }
        });

        ui.label(format!("Value: {}", self.value));
    }
}
