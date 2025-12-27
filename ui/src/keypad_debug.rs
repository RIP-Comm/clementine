//! Keypad Debug Tool
//!
//! A compact debug window that displays the current state of all GBA buttons
//! and allows toggling them manually for testing purposes.

use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuCommand, EmuHandle, GbaButton};
use crate::ui_traits::UiTool;

/// Debug tool for viewing and toggling GBA button states.
pub struct KeypadDebug {
    emu_handle: Arc<Mutex<EmuHandle>>,
}

impl KeypadDebug {
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self { emu_handle }
    }

    const fn is_pressed(key_input: u16, button: GbaButton) -> bool {
        (key_input & (button as u16)) == 0
    }

    /// Compact button: small square with single char or short label.
    fn btn(&self, ui: &mut egui::Ui, label: &str, button: GbaButton, key_input: u16) {
        let pressed = Self::is_pressed(key_input, button);
        let text = egui::RichText::new(label).small().color(if pressed {
            egui::Color32::WHITE
        } else {
            egui::Color32::GRAY
        });

        let bg = if pressed {
            egui::Color32::from_rgb(0, 120, 215)
        } else {
            egui::Color32::from_rgb(50, 50, 50)
        };

        if ui
            .add(
                egui::Button::new(text)
                    .fill(bg)
                    .min_size(egui::vec2(24.0, 18.0)),
            )
            .clicked()
            && let Ok(mut handle) = self.emu_handle.lock()
        {
            handle.send(EmuCommand::SetKey {
                button,
                pressed: !pressed,
            });
        }
    }
}

impl UiTool for KeypadDebug {
    fn name(&self) -> &'static str {
        "Keypad Debug"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(120.0)
            .open(open)
            .default_pos(egui::pos2(1800.0 - 600.0, 520.0))
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let key_input = self
            .emu_handle
            .lock()
            .map_or(0x03FF, |handle| handle.state.key_input);

        ui.small("Click to toggle");

        // Shoulders: L and R at top
        ui.horizontal(|ui| {
            self.btn(ui, "L", GbaButton::L, key_input);
            ui.add_space(40.0);
            self.btn(ui, "R", GbaButton::R, key_input);
        });

        ui.add_space(2.0);

        // Main layout: D-Pad on left, A/B on right
        ui.horizontal(|ui| {
            // D-Pad
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    self.btn(ui, "^", GbaButton::Up, key_input);
                });
                ui.horizontal(|ui| {
                    self.btn(ui, "<", GbaButton::Left, key_input);
                    ui.add_space(2.0);
                    self.btn(ui, ">", GbaButton::Right, key_input);
                });
                ui.horizontal(|ui| {
                    ui.add_space(26.0);
                    self.btn(ui, "v", GbaButton::Down, key_input);
                });
            });

            ui.add_space(8.0);

            // A/B buttons
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    self.btn(ui, "B", GbaButton::B, key_input);
                    self.btn(ui, "A", GbaButton::A, key_input);
                });
            });
        });

        ui.add_space(2.0);

        // Start/Select in center
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            self.btn(ui, "Sel", GbaButton::Select, key_input);
            self.btn(ui, "Sta", GbaButton::Start, key_input);
        });

        // Collapsible details
        ui.collapsing("Info", |ui| {
            ui.small(format!("REG: 0x{key_input:04X}"));
            ui.separator();
            ui.small("Z/X = A/B");
            ui.small("Enter = Start");
            ui.small("Arrows = D-Pad");
            ui.small("A/S = L/R");
        });
    }
}
