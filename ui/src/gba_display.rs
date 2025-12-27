use egui::{self, ColorImage, ImageSource, Ui};

use eframe::epaint::textures::TextureOptions;
use egui::load::SizedTexture;
use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuHandle, LCD_HEIGHT, LCD_WIDTH};
use crate::ui_traits::UiTool;

pub struct GbaDisplay {
    emu_handle: Arc<Mutex<EmuHandle>>,
}

impl GbaDisplay {
    pub const fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self { emu_handle }
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    fn ui(&mut self, ui: &mut Ui) {
        // Get the latest frame from the cached state
        let rgb_data = self.emu_handle.lock().map_or_else(
            |_| vec![0u8; LCD_WIDTH * LCD_HEIGHT * 3],
            |handle| {
                handle
                    .frame
                    .as_ref()
                    .map_or_else(|| vec![0u8; LCD_WIDTH * LCD_HEIGHT * 3], |f| f.to_vec())
            },
        );

        let image = ColorImage::from_rgb([LCD_WIDTH, LCD_HEIGHT], &rgb_data);

        let texture = ui
            .ctx()
            .load_texture("gba_display", image, TextureOptions::NEAREST);

        ui.image(ImageSource::Texture(SizedTexture {
            id: texture.id(),
            size: ui.available_size(),
        }));
    }
}

impl UiTool for GbaDisplay {
    fn name(&self) -> &'static str {
        "Gba Display"
    }

    #[allow(clippy::cast_precision_loss)]
    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        let scale = 3.0;
        egui::Window::new(self.name())
            .open(open)
            .default_pos(egui::pos2(10.0, 300.0))
            .default_width(LCD_WIDTH as f32 * scale)
            .default_height(LCD_HEIGHT as f32 * scale)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, _ui: &mut Ui) {
        todo!()
    }
}
