use egui::{self, Color32, ColorImage, Vec2};

use std::{
    borrow::BorrowMut,
    sync::{Arc, Mutex},
};

use emu::{
    arm7tdmi::Arm7tdmi,
    gba::Gba,
    render::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
};

use crate::{
    gba_color::GbaColor,
    ui_traits::{UiTool, View},
};

pub struct GbaDisplay {
    image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,
    gba: Arc<Mutex<Gba<Arm7tdmi>>>,
    scale: f32,
}

impl GbaDisplay {
    pub(crate) fn new(gba: Arc<Mutex<Gba<Arm7tdmi>>>) -> Self {
        #[cfg(not(feature = "test_bitmap"))]
        {
            Self {
                image: ColorImage::new([DISPLAY_WIDTH, DISPLAY_HEIGHT], Color32::BLACK),
                texture: None,
                gba,
                scale: 1.0,
            }
        }

        #[cfg(feature = "test_bitmap")]
        {
            let mut res = Self {
                image: ColorImage::new([DISPLAY_WIDTH, DISPLAY_HEIGHT], Color32::BLACK),
                texture: None,
                gba,
                scale: 1.0,
            };
            res.load_test_bitmap();
            return res;
        }
    }

    #[cfg(feature = "test_bitmap")]
    pub fn load_test_bitmap(&mut self) {
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
            gba.cpu
                .borrow_mut()
                .ppu
                .load_bitmap(bitmap_data, size[0], size[1]);
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

        if let Ok(mut gba) = self.gba.lock() {
            let gba_display = gba.cpu.borrow_mut().ppu.render();
            for y in 0..DISPLAY_HEIGHT {
                for x in 0..DISPLAY_WIDTH {
                    self.image[(x, y)] = GbaColor(gba_display[(x, y)]).into();
                }
            }
        }

        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            // Load the texture only once.
            ui.ctx().load_texture(
                "gba_display",
                self.image.clone(),
                egui::TextureFilter::Linear,
            )
        });

        let size = Vec2::new(
            texture.size_vec2().x * self.scale,
            texture.size_vec2().y * self.scale,
        );
        ui.image(texture, size);
    }
}

impl UiTool for GbaDisplay {
    fn name(&self) -> &'static str {
        "Gba Display"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new(self.name())
            .min_width(DISPLAY_WIDTH as f32)
            .min_height(DISPLAY_HEIGHT as f32)
            .open(open)
            .default_width(DISPLAY_WIDTH as f32)
            .default_height(DISPLAY_HEIGHT as f32)
            .resizable(false)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }
}
