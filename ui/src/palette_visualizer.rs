use crate::{
    gba_color::GbaColor,
    ui_traits::{UiTool, View},
};
use egui::Color32;

use egui_extras::{Size, TableBuilder};

use emu::{
    render::color::{Color, PaletteType},
    render::{BG_PALETTE_ADDRESS, MAX_COLORS_SINGLE_PALETTE, OBJ_PALETTE_ADDRESS},
};
use std::sync::{Arc, Mutex};

use emu::{arm7tdmi::Arm7tdmi, gba::Gba};

pub struct PaletteVisualizer {
    gba: Arc<Mutex<Gba<Arm7tdmi>>>,
    palettes: Vec<Vec<Color>>,
    palette_type: PaletteType,
    start_address: u32,
}

impl PaletteVisualizer {
    pub fn new(gba: Arc<Mutex<Gba<Arm7tdmi>>>) -> Self {
        Self {
            gba,
            palettes: vec![],
            palette_type: PaletteType::BG,
            start_address: 0,
        }
    }
}

impl UiTool for PaletteVisualizer {
    fn name(&self) -> &'static str {
        "Palette Visualizer"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .default_width(320.0)
            .default_height(500.0)
            .open(open)
            .show(ctx, |ui| {
                use View as _;
                self.ui(ui);
            });
    }
}

impl View for PaletteVisualizer {
    fn ui(&mut self, ui: &mut egui::Ui) {
        if let Ok(gba) = self.gba.lock() {
            self.palettes = gba.cpu.ppu.get_palettes(&self.palette_type);
            match self.palette_type {
                PaletteType::BG => {
                    self.start_address = BG_PALETTE_ADDRESS;
                }
                PaletteType::OBJ => {
                    self.start_address = OBJ_PALETTE_ADDRESS;
                }
            }
        }

        ui.label("Memory type:");

        #[cfg(feature = "debug")]
        if ui.button("RANDOM VALUES").clicked() {
            if let Ok(mut gba) = self.gba.lock() {
                #[cfg(feature = "debug")]
                gba.cpu.ppu.load_random_palettes();
            }
        }

        ui.horizontal(|ui| {
            ui.radio_value(&mut self.palette_type, PaletteType::BG, "BG Palette");
            ui.spacing();

            ui.radio_value(&mut self.palette_type, PaletteType::OBJ, "OBJ Palette");
            ui.spacing();
        });

        let mut table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Size::initial(80.0).at_least(80.0));

        for _n_col in 0..self.palettes.len() {
            table = table.column(Size::initial(20.0).at_least(20.0))
        }

        table = table.resizable(true);

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.heading("Address");
                });

                for i in 0..self.palettes[0].len() {
                    header.col(|ui| {
                        ui.heading(format!("{:02X}", i * 16));
                    });
                }
            })
            .body(|mut body| {
                let mut offset = 0;
                for palette in &self.palettes {
                    let current_row_address = self.start_address;

                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(format!("0x{:08X}", current_row_address + offset));
                        });

                        for color in palette {
                            let color_u32: Color32 = GbaColor(*color).into();
                            row.col(|ui| {
                                ui.add(egui::Button::new("    ").fill(color_u32));
                            });
                        }
                    });
                    // 16 colors  (every color is 2 bytes)
                    offset += MAX_COLORS_SINGLE_PALETTE as u32 * 2;
                }
            });
    }
}
