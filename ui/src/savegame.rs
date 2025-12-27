use std::sync::{Arc, Mutex};

use crate::emu_thread::EmuHandle;
use crate::ui_traits::UiTool;
use native_dialog::MessageDialog;

pub struct SaveGame {
    emu_handle: Arc<Mutex<EmuHandle>>,
}

impl SaveGame {
    pub const fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self { emu_handle }
    }

    fn save_state(&self) {
        // TODO: Implement save state via EmuCommand::RequestSaveState
        // Currently, Gba doesn't implement Serialize, so this is stubbed out
        let _ = &self.emu_handle; // silence unused warning
        MessageDialog::new()
            .set_title("Clementine")
            .set_text("Save state is not yet implemented for the threaded emulator architecture.")
            .show_alert()
            .ok();
    }

    fn load_state(&self) {
        // TODO: Implement load state via EmuCommand::LoadState
        // Currently, Gba doesn't implement Deserialize, so this is stubbed out
        let _ = &self.emu_handle; // silence unused warning
        MessageDialog::new()
            .set_title("Clementine")
            .set_text("Load state is not yet implemented for the threaded emulator architecture.")
            .show_alert()
            .ok();
    }
}

impl UiTool for SaveGame {
    fn name(&self) -> &'static str {
        "Save Game"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(100.0)
            .open(open)
            .default_pos(egui::pos2(10.0, 10.0))
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        if ui.button("Save").clicked() {
            self.save_state();
        }

        if ui.button("Load").clicked() {
            self.load_state();
        }
    }
}
