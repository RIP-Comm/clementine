//! Keypad Debug Tool
//!
//! A compact debug window that displays the current state of all GBA buttons
//! and allows toggling them manually for testing purposes.

use std::sync::{Arc, Mutex};

use egui::Key;

use crate::emu_thread::{EmuCommand, EmuHandle, GbaButton};
use crate::ui_traits::UiTool;

/// Debug tool for viewing and toggling GBA button states.
pub struct KeypadDebug {
    emu_handle: Arc<Mutex<EmuHandle>>,
    /// Shared keyboard-to-GBA-button bindings, also read by the input handler.
    key_bindings: Arc<Mutex<Vec<(GbaButton, Key)>>>,
    /// The button currently waiting to capture a new key, if any.
    rebinding: Option<GbaButton>,
}

impl KeypadDebug {
    pub const fn new(
        emu_handle: Arc<Mutex<EmuHandle>>,
        key_bindings: Arc<Mutex<Vec<(GbaButton, Key)>>>,
    ) -> Self {
        Self {
            emu_handle,
            key_bindings,
            rebinding: None,
        }
    }

    /// Rebinding section: a row per GBA button showing its key with a button to
    /// capture a new one.
    fn key_bindings_ui(&mut self, ui: &mut egui::Ui) {
        // Snapshot the current bindings for display.
        let current: Vec<(GbaButton, Key)> = self
            .key_bindings
            .lock()
            .map_or_else(|_| Vec::new(), |bindings| bindings.clone());

        for button in GbaButton::ALL {
            let key = current.iter().find(|(b, _)| *b == button).map(|(_, k)| *k);

            ui.horizontal(|ui| {
                ui.small(button.name());
                let label = if self.rebinding == Some(button) {
                    "press a key...".to_string()
                } else {
                    key.map_or_else(|| "unset".to_string(), |k| k.name().to_string())
                };
                if ui.small_button(label).clicked() {
                    self.rebinding = if self.rebinding == Some(button) {
                        None
                    } else {
                        Some(button)
                    };
                }
            });
        }

        // Capture the next key press for the button being rebound.
        if let Some(button) = self.rebinding
            && let Some(new_key) = ui.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => Some(*key),
                    _ => None,
                })
            })
            && let Ok(mut bindings) = self.key_bindings.lock()
        {
            bindings.retain(|(b, _)| *b != button);
            bindings.push((button, new_key));
            self.rebinding = None;
        }
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

        // Collapsible key rebinding.
        ui.collapsing("Key bindings", |ui| {
            self.key_bindings_ui(ui);
        });

        ui.collapsing("Info", |ui| {
            ui.small(format!("REG: 0x{key_input:04X}"));
        });
    }
}
