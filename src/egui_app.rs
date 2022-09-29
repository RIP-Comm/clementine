use egui_glium::egui_winit::{
    self,
    egui::{self, Color32, ColorImage},
};

use crate::{
    cpu::Cpu,
    ppu::{DISPLAY_HEIGHT, DISPLAY_WIDTH, Ppu},
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
    pub(crate) fn new(cpu: T, ppu: Ppu) -> Self {
        Self {
            image: ColorImage::new([DISPLAY_WIDTH, DISPLAY_HEIGHT], Color32::BLACK),
            texture: None,
            gba: Gba::new(cpu, ppu),
        }
    }

    pub(crate) fn draw(&mut self, egui_context: &egui::Context) {

        // TODO We need to find of Video Memory boundaries
        let start = 0x06000000 / 8;
        let stop = 0x06017FFF / 8;

        let video_ram = &self.gba.ppu.rom[start..stop];

        for y in 0..DISPLAY_HEIGHT {
            for x in 0..DISPLAY_WIDTH {
                let start = 2 * (y * DISPLAY_HEIGHT + x);
                let end = 2 * (y * DISPLAY_HEIGHT + x) + 1;
                if start < video_ram.len() && end < video_ram.len() {
                    let color = &video_ram[start..=end];
                    let color = ((color[0] as u16) << 8) + color[1] as u16;
                    let blue = (color & 0b0_11111_00000_00000 >> 10) as u8;
                    let green = (color & 0b0_00000_11111_00000 >> 5) as u8;
                    let red = (color & 0b0_00000_00000_11111) as u8;
                    self.image[(x, y)] = Color32::from_rgb(red, green, blue);
                }
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
    pub ppu: Ppu,
}

impl<T> Gba<T>
where
    T: Cpu,
{
    pub(crate) const fn new(cpu: T, ppu: Ppu) -> Self {
        Self { cpu, ppu }
    }
}
