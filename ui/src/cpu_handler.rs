use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut, Range};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

use egui::text_selection::text_cursor_state::byte_index_from_char_index;
use egui::{TextBuffer, TextEdit};

use emu::gba::Gba;

use crate::ui_traits::UiTool;

pub struct CpuHandler {
    gba: Arc<Mutex<Gba>>,
    play: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
    breakpoints: Arc<Mutex<BTreeSet<Breakpoint>>>,
    b_address: UpperHexString,
    breakpoint_combo: BreakpointType,
    cycle_to_skip_custom_value: u64,
}

impl CpuHandler {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self {
            gba,
            play: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            breakpoints: Arc::new(Mutex::new(BTreeSet::new())),
            b_address: UpperHexString::default(),
            breakpoint_combo: BreakpointType::Equal,
            cycle_to_skip_custom_value: 5000,
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd)]
struct Breakpoint {
    address: u32,
    kind: BreakpointType,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
enum BreakpointType {
    Equal,
    Greater,
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
}

impl UiTool for CpuHandler {
    fn name(&self) -> &'static str {
        "Cpu Handler"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(320.0)
            .open(open)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    #[allow(clippy::too_many_lines)]
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Cartridge name:");
            let mut cartridge_name = String::default();
            if let Ok(gba) = self.gba.lock() {
                cartridge_name.clone_from(&gba.cartridge_header.game_title);
            }
            ui.text_edit_singleline(&mut cartridge_name);

            if ui
                .add_enabled(
                    !self.play.load(std::sync::atomic::Ordering::Relaxed),
                    egui::Button::new("▶"),
                )
                .clicked()
            {
                if self.play.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }

                let gba_clone = Arc::clone(&self.gba);
                let play_clone = Arc::clone(&self.play);
                let breakpoints_clone = Arc::clone(&self.breakpoints);

                self.play.swap(true, std::sync::atomic::Ordering::Relaxed);

                self.thread_handle = Some(thread::spawn(move || {
                    while play_clone.load(std::sync::atomic::Ordering::Relaxed) {
                        breakpoints_clone.lock().unwrap().iter().for_each(|&b| {
                            let pc = u32::try_from(
                                gba_clone.lock().unwrap().cpu.registers.program_counter(),
                            )
                            .expect("Failed to convert u16 to u32");
                            match b.kind {
                                BreakpointType::Equal => {
                                    if pc == b.address {
                                        play_clone
                                            .swap(false, std::sync::atomic::Ordering::Relaxed);
                                    }
                                }
                                BreakpointType::Greater => {
                                    if pc > b.address {
                                        play_clone
                                            .swap(false, std::sync::atomic::Ordering::Relaxed);
                                    }
                                }
                            }
                        });

                        gba_clone.lock().unwrap().step();
                    }
                }));
            }

            if ui
                .add_enabled(
                    self.play.load(std::sync::atomic::Ordering::Relaxed),
                    egui::Button::new("⏸ "),
                )
                .clicked()
            {
                self.play.swap(false, std::sync::atomic::Ordering::Relaxed);
                self.thread_handle = None;
            }
        });

        ui.collapsing("CPU Advanced controls", |ui| {
            ui.label(format!(
                "Current CPU cycle: {}",
                &mut self.gba.lock().unwrap().cpu.current_cycle
            ));

            ui.horizontal(|ui| {
                ui.label("Step CPU cycles:");

                if ui.button("⏭x1").clicked() {
                    if let Ok(mut gba) = self.gba.lock() {
                        gba.step();
                    }
                }

                if ui.button("⏭x10").clicked() {
                    if let Ok(mut gba) = self.gba.lock() {
                        (0..10).for_each(|_| gba.step());
                    }
                }

                if ui.button("⏭x100").clicked() {
                    if let Ok(mut gba) = self.gba.lock() {
                        (0..100).for_each(|_| gba.step());
                    }
                }

                if ui.button("⏭x500").clicked() {
                    if let Ok(mut gba) = self.gba.lock() {
                        (0..500).for_each(|_| gba.step());
                    }
                }

                if ui.button("⏭x1000").clicked() {
                    if let Ok(mut gba) = self.gba.lock() {
                        (0..1000).for_each(|_| gba.step());
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Step (custom) CPU cycles:");
                ui.add(egui::DragValue::new(&mut self.cycle_to_skip_custom_value).speed(100));

                if ui.button("Step").clicked()
                    && let Ok(mut gba) = self.gba.lock()
                {
                    (0..self.cycle_to_skip_custom_value).for_each(|_| gba.step());
                }
            })
        });

        ui.collapsing("Breakpoints", |ui| {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_source("breakpoint-type")
                    .selected_text(if self.breakpoint_combo == BreakpointType::Equal {
                        "Equal to"
                    } else {
                        "Greater than"
                    })
                    .show_ui(ui, |ui| {
                        ui.set_width(40.0);
                        ui.set_max_width(100.0);
                        ui.selectable_value(
                            &mut self.breakpoint_combo,
                            BreakpointType::Equal,
                            "Equal to",
                        );
                        ui.selectable_value(
                            &mut self.breakpoint_combo,
                            BreakpointType::Greater,
                            "Greater than",
                        );
                    });

                ui.label("address (HEX):");

                ui.add(
                    TextEdit::singleline(&mut self.b_address)
                        .desired_width(150.0)
                        .char_limit(16),
                );

                if ui.button("Set").clicked() {
                    if self.b_address.is_empty() {
                        return;
                    }

                    let a = if self.b_address.starts_with("0x") {
                        self.b_address[2..].to_string()
                    } else {
                        self.b_address.clone()
                    };

                    let address = u32::from_str_radix(&a, 16).unwrap();
                    let b = Breakpoint {
                        address,
                        kind: self.breakpoint_combo,
                    };

                    self.breakpoints.lock().unwrap().insert(b);

                    self.b_address.clear();
                }
            });

            egui::containers::ScrollArea::new([false, true]).show(ui, |ui| {
                ui.label("Active breakpoints:");
                let breakpoints = self.breakpoints.lock().unwrap().clone();

                for b in &breakpoints {
                    ui.horizontal(|ui| {
                        ui.label(format!("0x{:08X}", b.address));
                        if ui.button("X").clicked() {
                            self.breakpoints.lock().unwrap().remove(b);
                        }
                    });
                }
            });
        });
    }
}
