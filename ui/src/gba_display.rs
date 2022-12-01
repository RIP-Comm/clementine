use egui::{self, Color32, ColorImage, Vec2};
use macros::acquire_lock;

use std::sync::{Arc, Mutex};

use emu::{
    gba::Gba,
    render::{LCD_HEIGHT, LCD_WIDTH},
};

use crate::{
    gba_color::GbaColor,
    ui_traits::{UiTool, View},
};

pub struct GbaDisplay {
    image: egui::ColorImage,
    gba: Arc<Mutex<Gba>>,
    scale: f32,
}

impl GbaDisplay {
    pub(crate) fn new(gba: Arc<Mutex<Gba>>) -> Self {
        #[allow(unused_mut)]
        let mut res = Self {
            image: ColorImage::new([LCD_WIDTH, LCD_HEIGHT], Color32::BLACK),
            gba,
            scale: 1.0,
        };

        #[cfg(feature = "test_mode_3")]
        {
            res.load_test_mode_3();
        }

        #[cfg(feature = "test_mode_4")]
        {
            res.test_mode_4();
        }

        #[cfg(feature = "test_mode_5")]
        {
            res.test_mode_5();
        }

        res
    }

    #[cfg(feature = "test_mode_3")]
    pub fn load_test_mode_3(&mut self) {
        let image_data = include_bytes!("../../img/clementine_logo_test_bitmap.png");
        let color_image: ColorImage =
            egui_extras::image::load_image_bytes(image_data).expect("Failed to load image");

        let size = color_image.size;
        let bitmap_data = color_image
            .clone()
            .pixels
            .into_iter()
            .map(|pixel| {
                let gba_color: GbaColor = pixel.into();
                gba_color.0
            })
            .collect();

        if let Ok(mut gba) = self.gba.lock() {
            gba.ppu.load_centered_bitmap(bitmap_data, size[0], size[1]);
        }
    }

    #[cfg(feature = "test_mode_5")]
    pub fn test_mode_5(&mut self) {
        let image_data = include_bytes!("../../img/clementine_logo_160px.png");
        let color_image: ColorImage =
            egui_extras::image::load_image_bytes(image_data).expect("Failed to load image");

        let size = color_image.size;
        let bitmap_data = color_image
            .clone()
            .pixels
            .into_iter()
            .map(|pixel| {
                let gba_color: GbaColor = pixel.into();
                gba_color.0
            })
            .collect();

        if let Ok(mut gba) = self.gba.lock() {
            gba.ppu.load_gbc_bitmap(bitmap_data, size[0], size[1]);
        }
    }

    #[cfg(feature = "test_mode_4")]
    pub fn test_mode_4(&self) {
        if let Ok(mut gba) = self.gba.lock() {
            gba.ppu.load_default_palette();
        }
    }
}

impl View for GbaDisplay {
    fn ui(&mut self, ui: &mut egui::Ui) {
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

        acquire_lock!(self.gba, gba => {
            for row in 0..LCD_HEIGHT {
                for col in 0..LCD_WIDTH {
                    self.image[(col, row)] =
                        GbaColor(gba.lcd.lock().unwrap()[(col, row)]).into();
                }
            }
        });

        let screen = ui.ctx().load_texture(
            "gba_display",
            self.image.clone(),
            egui::TextureFilter::Linear,
        );

        let size = Vec2::new(
            screen.size_vec2().x * self.scale,
            screen.size_vec2().y * self.scale,
        );
        ui.image(&screen, size);
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
}
