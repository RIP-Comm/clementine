//! ROM Information widget, it displays cartridge metadata.

use std::sync::{Arc, Mutex};

use crate::emu_thread::EmuHandle;
use crate::ui_traits::UiTool;

pub struct RomInfo {
    emu_handle: Arc<Mutex<EmuHandle>>,
}

impl RomInfo {
    pub const fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self { emu_handle }
    }

    /// Get the maker name from the 2-character maker code.
    fn maker_name(code: &str) -> &'static str {
        match code {
            "01" => "Nintendo",
            "08" => "Capcom",
            "13" | "69" => "Electronic Arts",
            "18" => "Hudson Soft",
            "20" => "Destination Software",
            "41" => "Ubisoft",
            "4F" => "Eidos",
            "51" => "Acclaim",
            "52" => "Activision",
            "5D" => "Midway",
            "5G" => "Majesco",
            "64" => "LucasArts",
            "6S" => "TDK Mediactive",
            "78" => "THQ",
            "7D" => "Vivendi",
            "8P" => "Sega",
            "A4" | "EM" => "Konami",
            "AF" => "Namco",
            "B2" => "Bandai",
            "DA" => "Tomy",
            _ => "Unknown",
        }
    }

    /// Display a colored status indicator (green check or red X).
    fn status_label(ui: &mut egui::Ui, valid: bool) {
        if valid {
            ui.colored_label(egui::Color32::GREEN, "Valid");
        } else {
            ui.colored_label(egui::Color32::RED, "Invalid");
        }
    }
}

impl UiTool for RomInfo {
    fn name(&self) -> &'static str {
        "ROM Info"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(280.0)
            .default_pos(egui::pos2(450.0, 10.0))
            .open(open)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        if let Ok(handle) = self.emu_handle.lock() {
            let state = &handle.state;

            egui::Grid::new("rom_info_grid")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Title:");
                    ui.strong(&state.cartridge_title);
                    ui.end_row();

                    ui.label("Game Code:");
                    ui.strong(&state.game_code);
                    ui.end_row();

                    ui.label("Maker:");
                    let maker = Self::maker_name(&state.maker_code);
                    ui.strong(format!("{maker} ({})", state.maker_code));
                    ui.end_row();

                    ui.label("Version:");
                    ui.strong(format!("1.{}", state.software_version));
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.strong("Boot Validation");
            ui.add_space(4.0);

            egui::Grid::new("boot_validation_grid")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Entry Point:");
                    ui.horizontal(|ui| {
                        ui.monospace(format!("{:#010X}", state.entry_point));
                        Self::status_label(ui, state.entry_point_valid);
                    });
                    ui.end_row();

                    ui.label("Nintendo Logo:");
                    Self::status_label(ui, state.logo_valid);
                    ui.end_row();

                    ui.label("Header Checksum:");
                    Self::status_label(ui, state.checksum_valid);
                    ui.end_row();

                    ui.label("Fixed Value (0x96):");
                    Self::status_label(ui, state.fixed_value_valid);
                    ui.end_row();
                });

            ui.add_space(8.0);

            let is_bootable = state.logo_valid && state.checksum_valid && state.fixed_value_valid;
            ui.horizontal(|ui| {
                ui.strong("Bootable:");
                if is_bootable {
                    ui.colored_label(egui::Color32::GREEN, "Yes");
                } else {
                    ui.colored_label(egui::Color32::RED, "No - BIOS would halt");
                }
            });
        } else {
            ui.label("Unable to read ROM info");
        }
    }
}
