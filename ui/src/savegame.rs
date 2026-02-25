//! Save/Load state functionality.
//!
//! Allows saving and loading the emulator state to/from files.
//! Save states are stored in the current directory with the game title as filename.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuCommand, EmuHandle};
use crate::ui_traits::UiTool;

pub struct SaveGame {
    emu_handle: Arc<Mutex<EmuHandle>>,
    /// Tracks if we're waiting for save state data from the emu thread.
    pending_save: bool,
    /// Status message to display.
    status: Option<String>,
}

impl SaveGame {
    pub const fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self {
            emu_handle,
            pending_save: false,
            status: None,
        }
    }

    fn get_save_path(&self) -> PathBuf {
        let game_title = self.emu_handle.lock().map_or_else(
            |_| "savestate".to_string(),
            |h| h.state.cartridge_title.trim().replace(' ', "_"),
        );

        let filename = if game_title.is_empty() {
            "savestate.sav".to_string()
        } else {
            format!("{game_title}.sav")
        };

        // Get absolute path in current working directory
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(filename)
    }

    fn save_state(&mut self) {
        if let Ok(mut handle) = self.emu_handle.lock() {
            handle.send(EmuCommand::RequestSaveState);
            self.pending_save = true;
            self.status = Some("Saving...".to_string());
        }
    }

    fn check_pending_save(&mut self) {
        if !self.pending_save {
            return;
        }

        let save_data = if let Ok(mut handle) = self.emu_handle.lock() {
            handle.pending_save_state.take()
        } else {
            return;
        };

        if let Some(data) = save_data {
            self.pending_save = false;
            let path = self.get_save_path();

            match std::fs::write(&path, &data) {
                Ok(()) => {
                    let size_kb = data.len() / 1024;
                    self.status = Some(format!("Saved to {} ({size_kb} KB)", path.display()));
                }
                Err(e) => {
                    self.status = Some(format!("Error: {e}"));
                }
            }
        }
    }

    fn load_state(&mut self) {
        let path = self.get_save_path();

        if !path.exists() {
            self.status = Some(format!("No save file: {}", path.display()));
            return;
        }

        match std::fs::read(&path) {
            Ok(data) => {
                let size_kb = data.len() / 1024;
                if let Ok(mut handle) = self.emu_handle.lock() {
                    handle.send(EmuCommand::LoadState(data));
                    self.status = Some(format!("Loading {} ({size_kb} KB)...", path.display()));
                }
            }
            Err(e) => {
                self.status = Some(format!("Error: {e}"));
            }
        }
    }

    fn check_load_error(&mut self) {
        let error = if let Ok(mut handle) = self.emu_handle.lock() {
            handle.load_state_error.take()
        } else {
            return;
        };

        if let Some(error_msg) = error {
            self.status = Some(format!("Error: {error_msg}"));
        }
    }
}

impl UiTool for SaveGame {
    fn name(&self) -> &'static str {
        "Save Game"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.check_pending_save();
        self.check_load_error();

        egui::Window::new(self.name())
            .default_width(150.0)
            .open(open)
            .default_pos(egui::pos2(10.0, 10.0))
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                self.save_state();
            }

            if ui.button("Load").clicked() {
                self.load_state();
            }
        });

        ui.separator();

        // Show save file path
        let path = self.get_save_path();
        ui.label("Save file:");
        ui.add(egui::Label::new(path.display().to_string()).wrap_mode(egui::TextWrapMode::Wrap));

        // Show if file exists
        if path.exists() {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let size_kb = metadata.len() / 1024;
                ui.small(format!("(exists, {size_kb} KB)"));
            }
        } else {
            ui.small("(no save yet)");
        }

        // Show status message
        if let Some(status) = &self.status {
            ui.separator();
            ui.label(status);
        }

        if self.pending_save {
            ui.spinner();
        }
    }
}
