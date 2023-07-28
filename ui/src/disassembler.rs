use std::sync::{Arc, Mutex};

use egui::{ScrollArea, TextEdit, TextStyle};
use emu::gba::Gba;

use crate::ui_traits::UiTool;

pub struct Disassembler {
    gba: Arc<Mutex<Gba>>,
}

impl Disassembler {
    pub(crate) fn new(arc_gba: Arc<Mutex<Gba>>) -> Self {
        Self { gba: arc_gba }
    }
}

impl UiTool for Disassembler {
    fn name(&self) -> &'static str {
        "Disassembler"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .resizable(true)
            .open(open)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let mut s = self
            .gba
            .lock()
            .unwrap()
            .cpu
            .lock()
            .unwrap()
            .disassembler_buffer
            .join("\n");

        ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            ui.add(
                TextEdit::multiline(&mut s)
                    .interactive(false)
                    .font(TextStyle::Monospace)
                    .layouter(&mut |ui, val, _| {
                        ui.fonts(|fonts| {
                            fonts.layout_no_wrap(
                                val.to_string(),
                                TextStyle::Monospace.resolve(ui.style()),
                                ui.visuals().widgets.inactive.text_color(),
                            )
                        })
                    }),
            );
        });
    }
}
