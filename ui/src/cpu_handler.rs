use emu::gba::Gba;

use crate::ui_traits::UiTool;

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct CpuHandler {
    gba: Arc<Mutex<Gba>>,
    play: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
    breakpoints: Arc<Mutex<BTreeSet<Breakpoint>>>,
    b_address: String,

    breakpoint_combo: BreakpointType,
}

impl CpuHandler {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self {
            gba,
            play: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            breakpoints: Arc::new(Mutex::new(BTreeSet::new())),
            b_address: String::default(),
            breakpoint_combo: BreakpointType::Equal,
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

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Cartridge name:");
            let mut cartridge_name: String = Default::default();
            if let Ok(gba) = self.gba.lock() {
                cartridge_name = gba.cartridge_header.game_title.clone()
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
                            let pc =
                                gba_clone.lock().unwrap().cpu.registers.program_counter() as u32;
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

            if ui.button("⏭x1").clicked() {
                if let Ok(mut gba) = self.gba.lock() {
                    gba.step()
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
        });

        ui.add_space(20.0);

        ui.heading("Breakpoints");
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            ui.label("Address (hex) : ");
            ui.text_edit_singleline(&mut self.b_address);

            egui::ComboBox::from_label("Select breakpoint type")
                .selected_text(if self.breakpoint_combo == BreakpointType::Equal {
                    "="
                } else {
                    ">"
                })
                .show_ui(ui, |ui| {
                    ui.style_mut().wrap = Some(false);
                    ui.set_min_width(60.0);
                    ui.selectable_value(&mut self.breakpoint_combo, BreakpointType::Equal, "=");
                    ui.selectable_value(&mut self.breakpoint_combo, BreakpointType::Greater, ">");
                });

            if ui.button("Add").clicked() {
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

        ui.add_space(10.0);
        egui::containers::ScrollArea::new([false, true]).show(ui, |ui| {
            let breakpoints = self.breakpoints.lock().unwrap().clone();

            for b in breakpoints.iter() {
                ui.horizontal(|ui| {
                    ui.label(format!("0x{:08X}", b.address));
                    if ui.button("x").clicked() {
                        self.breakpoints.lock().unwrap().remove(b);
                    }
                });
            }
        });
    }
}
