use egui::{self, ColorImage, ImageSource, TextureHandle, Ui};

use eframe::epaint::textures::TextureOptions;
use egui::load::SizedTexture;
use std::sync::{Arc, Mutex};

use crate::emu_thread::{EmuHandle, LCD_HEIGHT, LCD_WIDTH};
use crate::ui_traits::UiTool;

pub struct GbaDisplay {
    emu_handle: Arc<Mutex<EmuHandle>>,
    /// Cached texture, reused across frames to avoid recreation overhead.
    texture: Option<TextureHandle>,
    /// Frame sequence of the texture currently uploaded, to skip re-uploading
    /// an unchanged frame on repaints that carry no new emulator output.
    uploaded_seq: Option<u64>,
}

impl GbaDisplay {
    pub const fn new(emu_handle: Arc<Mutex<EmuHandle>>) -> Self {
        Self {
            emu_handle,
            texture: None,
            uploaded_seq: None,
        }
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
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut Ui) {
        // Grab the latest frame plus its sequence number. Only rebuild and
        // re-upload the texture when a new frame has actually arrived.
        let frame = self.emu_handle.lock().ok().and_then(|handle| {
            (Some(handle.frame_seq) != self.uploaded_seq || self.texture.is_none())
                .then(|| (handle.frame_seq, handle.frame.as_ref().map(|f| f.to_vec())))
        });

        if let Some((seq, rgb)) = frame {
            let rgb_data = rgb.unwrap_or_else(|| vec![0u8; LCD_WIDTH * LCD_HEIGHT * 3]);
            let image = ColorImage::from_rgb([LCD_WIDTH, LCD_HEIGHT], &rgb_data);

            match &mut self.texture {
                Some(tex) => tex.set(image, TextureOptions::NEAREST),
                None => {
                    self.texture = Some(ui.ctx().load_texture(
                        "gba_display",
                        image,
                        TextureOptions::NEAREST,
                    ));
                }
            }
            self.uploaded_seq = Some(seq);
        }

        if let Some(tex) = &self.texture {
            ui.image(ImageSource::Texture(SizedTexture {
                id: tex.id(),
                size: ui.available_size(),
            }));
        }
    }
}
