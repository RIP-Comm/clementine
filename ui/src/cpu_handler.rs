use std::ops::{Deref, DerefMut, Range};
use std::sync::{Arc, Mutex};

use egui::text_selection::text_cursor_state::byte_index_from_char_index;
use egui::{TextBuffer, TextEdit};

use crate::emu_thread::{BreakpointKind, EmuCommand, EmuHandle};
use crate::ui_traits::UiTool;

/// Tolerance for floating point comparisons.
const SPEED_EPSILON: f32 = 0.01;

pub struct CpuHandler {
    emu_handle: Arc<Mutex<EmuHandle>>,
    b_address: UpperHexString,
    breakpoint_combo: BreakpointKind,
    cycle_to_skip_custom_value: u32,
    /// Local copy of speed for the slider.
    speed: f32,
    /// Whether uncapped (max) speed is enabled.
    uncapped: bool,
}

impl CpuHandler {
    pub fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self {
            emu_handle,
            b_address: UpperHexString::default(),
            breakpoint_combo: BreakpointKind::Equal,
            cycle_to_skip_custom_value: 5000,
            speed: 1.0,
            uncapped: false,
        }
    }

    /// Send speed update to the emulator thread.
    fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
        self.uncapped = false;
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::SetSpeed(speed));
        }
    }

    /// Enable uncapped (maximum) speed.
    fn set_uncapped(&mut self) {
        self.uncapped = true;
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::SetSpeed(0.0));
        }
    }
}

#[derive(Default)]
struct UpperHexString(String);

impl Deref for UpperHexString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UpperHexString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TextBuffer for UpperHexString {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        &self.0
    }

    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        let mut text_string = text.to_string();
        text_string.retain(|c| c.is_ascii_hexdigit());
        text_string.make_ascii_uppercase();

        let byte_idx = byte_index_from_char_index(self.as_str(), char_index);

        self.insert_str(byte_idx, text_string.as_str());

        text.chars().count()
    }

    fn delete_char_range(&mut self, char_range: Range<usize>) {
        assert!(char_range.start <= char_range.end);

        // Get both byte indices
        let byte_start = byte_index_from_char_index(self.as_str(), char_range.start);
        let byte_end = byte_index_from_char_index(self.as_str(), char_range.end);

        // Then drain all characters within this range
        self.drain(byte_start..byte_end);
    }

    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
}

impl UiTool for CpuHandler {
    fn name(&self) -> &'static str {
        "Cpu Handler"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(150.0)
            .open(open)
            .default_pos(egui::pos2(200.0, 10.0))
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    #[allow(clippy::too_many_lines)]
    fn ui(&mut self, ui: &mut egui::Ui) {
        let (cartridge_name, is_running, current_cycle, breakpoints, current_speed) =
            self.emu_handle.lock().map_or_else(
                |_| (String::new(), false, 0, Vec::new(), 1.0),
                |handle| {
                    (
                        handle.state.cartridge_title.clone(),
                        handle.state.is_running,
                        handle.state.cycle,
                        handle.breakpoints.clone(),
                        handle.speed,
                    )
                },
            );

        // Sync local speed with handle speed
        if current_speed == 0.0 {
            self.uncapped = true;
        } else {
            self.uncapped = false;
            self.speed = current_speed;
        }

        let mut name = cartridge_name;
        ui.add(TextEdit::singleline(&mut name).desired_width(140.0));

        ui.horizontal(|ui| {
            if ui
                .add_enabled(!is_running, egui::Button::new("▶ Run"))
                .clicked()
                && let Ok(mut handle) = self.emu_handle.lock()
            {
                handle.send(EmuCommand::Run);
            }

            if ui
                .add_enabled(is_running, egui::Button::new("⏸ Pause"))
                .clicked()
                && let Ok(mut handle) = self.emu_handle.lock()
            {
                handle.send(EmuCommand::Pause);
            }
        });

        // Speed control with preset buttons
        ui.horizontal(|ui| {
            ui.label("Speed:");

            // Speed preset buttons
            let speeds = [(1.0, "1x"), (2.0, "2x"), (4.0, "4x"), (8.0, "8x")];
            for (speed, label) in speeds {
                let is_selected = !self.uncapped && (self.speed - speed).abs() < SPEED_EPSILON;
                if ui.selectable_label(is_selected, label).clicked() {
                    self.set_speed(speed);
                }
            }

            // Turbo/Max button - runs as fast as possible
            if ui.selectable_label(self.uncapped, "Turbo").clicked() {
                if self.uncapped {
                    self.set_speed(1.0);
                } else {
                    self.set_uncapped();
                }
            }
        });

        ui.collapsing("CPU controls", |ui| {
            ui.label(format!("Cycle: {current_cycle}"));

            ui.label("Step cycles:");
            ui.horizontal(|ui| {
                for steps in [1, 10, 100] {
                    if ui.button(format!("x{steps}")).clicked()
                        && let Ok(mut handle) = self.emu_handle.lock()
                    {
                        handle.send(EmuCommand::Step(steps));
                    }
                }
            });
            ui.horizontal(|ui| {
                for steps in [500, 1000] {
                    if ui.button(format!("x{steps}")).clicked()
                        && let Ok(mut handle) = self.emu_handle.lock()
                    {
                        handle.send(EmuCommand::Step(steps));
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.cycle_to_skip_custom_value).speed(100));
                if ui.button("Step").clicked()
                    && let Ok(mut handle) = self.emu_handle.lock()
                {
                    handle.send(EmuCommand::Step(self.cycle_to_skip_custom_value));
                }
            });
        });

        ui.collapsing("Breakpoints", |ui| {
            egui::ComboBox::from_id_salt("breakpoint-type")
                .width(100.0)
                .selected_text(if self.breakpoint_combo == BreakpointKind::Equal {
                    "Equal"
                } else {
                    "Greater"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.breakpoint_combo, BreakpointKind::Equal, "Equal");
                    ui.selectable_value(
                        &mut self.breakpoint_combo,
                        BreakpointKind::GreaterThan,
                        "Greater",
                    );
                });

            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut self.b_address)
                        .desired_width(80.0)
                        .char_limit(8)
                        .hint_text("HEX"),
                );

                if ui.button("Add").clicked() {
                    if self.b_address.is_empty() {
                        return;
                    }

                    let a = if self.b_address.starts_with("0x") {
                        self.b_address[2..].to_string()
                    } else {
                        self.b_address.clone()
                    };

                    if let Ok(address) = u32::from_str_radix(&a, 16)
                        && let Ok(mut handle) = self.emu_handle.lock()
                    {
                        handle.send(EmuCommand::AddBreakpoint {
                            address,
                            kind: self.breakpoint_combo,
                        });
                    }

                    self.b_address.clear();
                }
            });

            if !breakpoints.is_empty() {
                ui.separator();
                egui::containers::ScrollArea::vertical()
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for (address, _kind) in &breakpoints {
                            ui.horizontal(|ui| {
                                ui.label(format!("{address:08X}"));
                                if ui.small_button("X").clicked()
                                    && let Ok(mut handle) = self.emu_handle.lock()
                                {
                                    handle.send(EmuCommand::RemoveBreakpoint(*address));
                                }
                            });
                        }
                    });
            }
        });
    }
}
