use egui::{self, ColorImage, Ui};

use eframe::epaint::textures::TextureOptions;
use std::sync::{Arc, Mutex};

use emu::{
    gba::Gba,
    render::{LCD_HEIGHT, LCD_WIDTH},
};

use crate::ui_traits::UiTool;

pub struct GbaDisplay {
    gba: Arc<Mutex<Gba>>,
    scale: f32,
}

impl GbaDisplay {
    pub(crate) fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self { gba, scale: 1.0 }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("x1").clicked() {
                self.scale = 1.0;
            }
            if ui.button("x2").clicked() {
                self.scale = 2.0;
            }
            if ui.button("x4").clicked() {
                self.scale = 4.0;
            }
        });

        //TODO: Fix this .lock().unwrap() repeated two times
        let rgb_data = self
            .gba
            .lock()
            .unwrap()
            .cpu
            .bus
            .lcd
            .buffer
            .iter()
            .flat_map(|row| {
                row.iter().flat_map(|pixel| {
                    let red = (pixel.red() << 3) | (pixel.red() >> 2);
                    let green = (pixel.green() << 3) | (pixel.green() >> 2);
                    let blue = (pixel.blue() << 3) | (pixel.blue() >> 2);
                    [red, green, blue]
                })
            })
            .collect::<Vec<_>>();

        let image = ColorImage::from_rgb([LCD_WIDTH, LCD_HEIGHT], &rgb_data);

        let texture = ui
            .ctx()
            .load_texture("gba_display", image, TextureOptions::NEAREST);

        ui.image(&texture);
    }
}

impl UiTool for GbaDisplay {
    fn name(&self) -> &'static str {
        "Gba Display"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .min_width(LCD_WIDTH as f32)
            .min_height(LCD_HEIGHT as f32)
            .open(open)
            .default_width(LCD_WIDTH as f32)
            .default_height(LCD_HEIGHT as f32)
            .resizable(false)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, _ui: &mut Ui) {
        todo!()
    }
}
