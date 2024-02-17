use std::{
    error::Error,
    io::{Read, Write},
    sync::{Arc, Mutex},
};

use emu::gba::Gba;

use crate::ui_traits::UiTool;
use emu::cpu::arm7tdmi::Arm7tdmi;
use native_dialog::{FileDialog, MessageDialog};
use std::fs;

pub struct SaveGame {
    gba: Arc<Mutex<Gba>>,
}

impl SaveGame {
    pub fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self { gba }
    }

    fn save_state(&self) -> Result<(), Box<dyn Error>> {
        let path = FileDialog::new()
            .set_location("~")
            .add_filter("Clementine save file", &["clm"])
            .show_save_single_file()?;

        let path = path.ok_or("No file selected")?;

        let cpu = &self.gba.lock().unwrap().cpu;

        let encoded = bincode::serialize(cpu)?;
        let mut file = fs::OpenOptions::new().write(true).create(true).open(path)?;

        file.write_all(&encoded)?;

        Ok(())
    }

    fn load_state(&mut self) -> Result<(), Box<dyn Error>> {
        let path = FileDialog::new()
            .set_location("~")
            .add_filter("Clementine save file", &["clm"])
            .show_open_single_file()?;

        let path = path.ok_or("No file selected")?;

        let mut file = fs::OpenOptions::new().read(true).open(path)?;
        let mut encoded = Vec::new();
        file.read_to_end(&mut encoded)?;

        let cpu = &mut self.gba.lock().unwrap().cpu;
        let decoded: Arm7tdmi = bincode::deserialize(&encoded)?;
        *cpu = decoded;

        Ok(())
    }
}

impl UiTool for SaveGame {
    fn name(&self) -> &'static str {
        "Save Game"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(50.0)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        if ui.button("Save").clicked() {
            self.save_state().unwrap_or_else(|err| {
                // Looking at the code of `MessageDialog` it seems like `.show_alert()` can never return `Err`
                MessageDialog::new()
                    .set_title("Clementine")
                    .set_text(err.to_string().as_str())
                    .show_alert()
                    .unwrap();
            })
        }

        if ui.button("Load").clicked() {
            self.load_state().unwrap_or_else(|err| {
                MessageDialog::new()
                    .set_title("Clementine")
                    .set_text(err.to_string().as_str())
                    .show_alert()
                    .unwrap();
            })
        }
    }
}
