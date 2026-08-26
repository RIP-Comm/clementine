//! Save/Load state functionality.
//!
//! Allows saving and loading the emulator state to/from files.
//! Save states are stored in the current directory using the game code and a
//! sanitized game title as the filename.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuCommand, EmuHandle};
use crate::ui_traits::UiTool;

fn sanitize_filename_component(value: &str) -> String {
    let mut component = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            component.push(character);
        } else if character.is_ascii_whitespace() && !component.ends_with('_') {
            component.push('_');
        }
    }

    let component = component.trim_matches('_');
    if component.is_empty() {
        "unknown".to_string()
    } else {
        component.to_string()
    }
}

fn save_filename(title: &str, game_code: &str) -> String {
    let title = sanitize_filename_component(title);
    let game_code = sanitize_filename_component(game_code);
    if game_code == "unknown" {
        format!("{title}.sav")
    } else {
        format!("{game_code}_{title}.sav")
    }
}

fn legacy_save_filename(title: &str) -> Option<String> {
    let title = title.trim();
    let stem = if title.is_empty() {
        "savestate".to_string()
    } else {
        title.replace(' ', "_")
    };

    (sanitize_filename_component(&stem) == stem).then(|| format!("{stem}.sav"))
}

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
        let (title, game_code) = self.emu_handle.lock().map_or_else(
            |_| (String::new(), String::new()),
            |h| (h.state.cartridge_title.clone(), h.state.game_code.clone()),
        );
        let filename = save_filename(&title, &game_code);

        // Get absolute path in current working directory
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(filename)
    }
    fn legacy_save_path(&self) -> Option<PathBuf> {
        let title = self.emu_handle.lock().ok()?.state.cartridge_title.clone();
        let filename = legacy_save_filename(&title)?;
        Some(
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(filename),
        )
    }
    fn save_state(&mut self) {
        if let Ok(mut handle) = self.emu_handle.lock()
            && handle.send(EmuCommand::RequestSaveState)
        {
            self.pending_save = true;
            self.status = Some("Saving...".to_string());
        } else {
            self.status = Some("Emulator busy, try again.".to_string());
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

            match Self::write_atomic(&path, &data) {
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

    /// Write the save to a sibling temp file and rename it over the target, so
    /// an interrupted write cannot corrupt or truncate the existing save.
    fn write_atomic(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        let tmp = path.with_extension("sav.tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, path)
    }

    fn load_state(&mut self) {
        let primary_path = self.get_save_path();
        let path = if primary_path.exists() {
            primary_path
        } else {
            self.legacy_save_path()
                .filter(|legacy_path| legacy_path.exists())
                .unwrap_or(primary_path)
        };

        if !path.exists() {
            self.status = Some(format!("No save file: {}", path.display()));
            return;
        }

        match std::fs::read(&path) {
            Ok(data) => {
                let size_kb = data.len() / 1024;
                if let Ok(mut handle) = self.emu_handle.lock() {
                    if handle.send(EmuCommand::LoadState(data)) {
                        self.status = Some(format!("Loading {} ({size_kb} KB)...", path.display()));
                    } else {
                        self.status = Some("Emulator busy, try again.".to_string());
                    }
                }
            }
            Err(e) => {
                self.status = Some(format!("Error: {e}"));
            }
        }
    }

    fn check_load_result(&mut self) {
        let Ok(mut handle) = self.emu_handle.lock() else {
            return;
        };

        if let Some(error_msg) = handle.load_state_error.take() {
            self.status = Some(format!("Error: {error_msg}"));
        }

        if handle.load_state_success {
            handle.load_state_success = false;
            self.status = Some("State loaded successfully.".to_string());
        }
    }
}

impl UiTool for SaveGame {
    fn name(&self) -> &'static str {
        "Save Game"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.check_pending_save();
        self.check_load_result();

        egui::Window::new(self.name())
            .default_width(150.0)
            .open(open)
            .default_pos(egui::pos2(10.0, 10.0))
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Save stays enabled even after a failed load. An incompatible .sav
            // is exactly the case where the user needs to write a fresh save
            // over it, and the atomic write never corrupts the old file.
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

#[cfg(test)]
mod tests {
    use super::{SaveGame, legacy_save_filename, sanitize_filename_component, save_filename};

    #[test]
    fn write_atomic_replaces_file_and_leaves_no_temp() {
        let path = std::env::temp_dir().join("clementine_write_atomic_test.sav");
        let tmp = path.with_extension("sav.tmp");
        let _ = std::fs::remove_file(&path);

        SaveGame::write_atomic(&path, &[1, 2, 3]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [1, 2, 3]);

        // A second write overwrites the first with no leftover temp file.
        SaveGame::write_atomic(&path, &[4, 5]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [4, 5]);
        assert!(!tmp.exists(), "temp file must be renamed away");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_filename_uses_safe_components() {
        assert_eq!(sanitize_filename_component("../escape/.."), "escape");
        assert_eq!(sanitize_filename_component(" / "), "unknown");
        assert_eq!(save_filename("../escape/..", "BPEE"), "BPEE_escape.sav");
    }

    #[test]
    fn legacy_save_names_are_supported_only_when_safe() {
        assert_eq!(
            legacy_save_filename("Pokemon FireRed"),
            Some("Pokemon_FireRed.sav".to_string())
        );
        assert_eq!(legacy_save_filename("../escape"), None);
    }
}
