use egui_glium::egui_winit::{
    self,
    egui::{self, Color32, ColorImage},
};

use crate::{
    cpu::Cpu,
    ppu::{DISPLAY_HEIGHT, DISPLAY_WIDTH},
};

pub struct EguiApp<T>
where
    T: Cpu,
{
    image: egui_winit::egui::ColorImage,
    texture: Option<egui_winit::egui::TextureHandle>,

    pub gba: Gba<T>,
}

impl<T> EguiApp<T>
where
    T: Cpu,
{
    pub(crate) fn new(cpu: T) -> Self {
        Self {
            image: ColorImage::new([DISPLAY_WIDTH, DISPLAY_HEIGHT], Color32::BLACK),
            texture: None,
            gba: Gba::new(cpu),
        }
    }

    pub(crate) fn draw(&mut self, egui_context: &egui::Context) {
        for y in 0..DISPLAY_HEIGHT {
            for x in 0..DISPLAY_WIDTH {
                self.image[(x, y)] = Color32::from_rgb(0, 0, 0);
            }
        }

        match &mut self.texture {
            Some(t) => t.set(self.image.clone(), egui::TextureFilter::Nearest),
            None => {
                self.texture = Some(egui_context.load_texture(
                    "screen",
                    self.image.clone(),
                    egui::TextureFilter::Nearest,
                ))
            }
        };

        egui::CentralPanel::default().show(egui_context, |ui| {
            let img = egui::Image::new(
                self.texture.as_ref().unwrap(),
                [DISPLAY_WIDTH as f32, DISPLAY_HEIGHT as f32],
            );

            let rect = egui_context.available_rect();
            img.paint_at(ui, rect);
        });
    }
}

pub struct Gba<T>
where
    T: Cpu,
{
    pub cpu: T,
}

impl<T> Gba<T>
where
    T: Cpu,
{
    pub(crate) const fn new(cpu: T) -> Self {
        Self { cpu }
    }
}
