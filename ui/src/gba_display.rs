use egui::{self, ColorImage, ImageSource, Ui};

use eframe::epaint::textures::TextureOptions;
use egui::load::SizedTexture;
use std::sync::{Arc, Mutex};

use emu::{
    gba::Gba,
    render::{LCD_HEIGHT, LCD_WIDTH},
};

use crate::ui_traits::UiTool;

pub struct GbaDisplay {
    gba: Arc<Mutex<Gba>>,
}

impl GbaDisplay {
    pub(crate) const fn new(gba: Arc<Mutex<Gba>>) -> Self {
        Self { gba }
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    fn ui(&mut self, ui: &mut Ui) {
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
        egui::Window::new(self.name())
            .open(open)
            .default_width(LCD_WIDTH as f32)
            .default_height(LCD_HEIGHT as f32)
            .collapsible(false)
            .show(ctx, |ui| {
                self.ui(ui);
            });
    }

    fn ui(&mut self, _ui: &mut Ui) {
        todo!()
    }
}
