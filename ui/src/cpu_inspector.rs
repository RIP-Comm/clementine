use emu::gba::Gba;

use crate::ui_traits::{UiTool, View};

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct CpuInspector {
    gba: Arc<Mutex<Gba>>,
    play: Arc<AtomicBool>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl CpuInspector {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self {
            gba,
            play: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
        }
    }
}

impl UiTool for CpuInspector {
    fn name(&self) -> &'static str {
        "Cpu Inspector"
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

impl View for CpuInspector {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("MODE: Arm7tdmi");
        ui.add_space(12.0);
        ui.horizontal(|ui| {
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
                if self.thread_handle.is_some() {
                    return;
                }

                let gba_clone = Arc::clone(&self.gba);
                let play_clone = Arc::clone(&self.play);

                self.play.swap(true, std::sync::atomic::Ordering::Relaxed);

                self.thread_handle = Some(thread::spawn(move || {
                    while play_clone.load(std::sync::atomic::Ordering::Relaxed) {
                        gba_clone.lock().unwrap().cpu.step();
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
            if ui.button("⏭").clicked() {
                if let Ok(mut gba) = self.gba.lock() {
                    gba.cpu.step()
                }
            }
        });
        ui.add_space(12.0);
        ui.heading("Registers");
        ui.add_space(8.0);

        // If it's poisoned it means that the thread that executes instructions
        // panicked, we still want access to registers to debug
        let registers = self.gba.lock().map_or_else(
            |poisoned| poisoned.into_inner().cpu.registers.to_vec(),
            |gba| gba.cpu.registers.to_vec(),
        );

        let mut index = 0;

        egui::Grid::new("ARM Registers")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for reg in registers {
                    let mut value = format!("0x{:x}", reg);
                    ui.label(if index == 15 {
                        format!("R{:?} (PC)", index)
                    } else {
                        format!("R{:?}", index)
                    });
                    ui.text_edit_singleline(&mut value);

                    ui.end_row();
                    index += 1;
                }
            });
    }
}
