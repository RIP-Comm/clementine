use emu::gba::Gba;

use crate::ui_traits::UiTool;

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct CpuHandler {
    gba: Arc<Mutex<Gba>>,
    play: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
    breakpoints: Arc<Mutex<HashSet<u32>>>,
    b_address: String,
}

impl CpuHandler {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self {
            gba,
            play: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            breakpoints: Arc::new(Mutex::new(HashSet::new())),
            b_address: String::default(),
        }
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
                        if breakpoints_clone.lock().unwrap().contains(
                            &(gba_clone.lock().unwrap().cpu.registers.program_counter() as u32),
                        ) {
                            play_clone.swap(false, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }

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

            if ui.button("Add").clicked() {
                self.breakpoints
                    .lock()
                    .unwrap()
                    .insert(u32::from_str_radix(&self.b_address, 16).unwrap());
            }
        });

        ui.add_space(10.0);
        egui::containers::ScrollArea::new([false, true]).show(ui, |ui| {
            let breakpoints = self.breakpoints.lock().unwrap().clone();

            for b in breakpoints.iter() {
                ui.horizontal(|ui| {
                    ui.label(format!("0x{b:x}"));
                    if ui.button("x").clicked() {
                        self.breakpoints.lock().unwrap().remove(b);
                    }
                });
            }
        });
    }
}
