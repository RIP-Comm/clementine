use egui::{self, Color32, ColorImage};

use std::sync::{Arc, Mutex};

use emu::{
    cpu::Cpu,
    gba::Gba,
    render::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
};

use crate::ui_traits::{UiTool, View};

pub struct GbaDisplay<T: Cpu> {
    image: egui::ColorImage,
    texture: Option<egui::TextureHandle>,

    pub gba: Arc<Mutex<Gba<T>>>,
}

impl<T: Cpu> GbaDisplay<T> {
    pub(crate) fn new(gba: Arc<Mutex<Gba<T>>>) -> Self {
        Self {
            image: ColorImage::new([DISPLAY_WIDTH, DISPLAY_HEIGHT], Color32::BLACK),
            texture: None,
            gba,
        }
    }
}

impl<T: Cpu> View for GbaDisplay<T> {
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.set_width(DISPLAY_WIDTH as f32);
        ui.set_height(DISPLAY_HEIGHT as f32);

        for y in 0..DISPLAY_HEIGHT {
            for x in 0..DISPLAY_WIDTH {
                self.image[(x, y)] = Color32::from_rgb(15, 56, 15);
            }
        }

        match &mut self.texture {
            Some(t) => t.set(self.image.clone(), egui::TextureFilter::Nearest),
            None => {
                self.texture = Some(ui.ctx().load_texture(
                    "screen",
                    self.image.clone(),
                    egui::TextureFilter::Nearest,
                ))
            }
        };

        let img = egui::Image::new(
            self.texture.as_ref().unwrap(),
            [DISPLAY_WIDTH as f32, DISPLAY_HEIGHT as f32],
        );

        let rect = ui.ctx().used_rect();
        img.paint_at(ui, rect);
    }
}

impl<T: Cpu> UiTool for GbaDisplay<T> {
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
                use View as _;
                self.ui(ui);
            });
    }
}
