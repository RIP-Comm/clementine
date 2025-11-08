use super::Layer;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{Color, LCD_WIDTH, PixelInfo};
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct Layer2 {
    #[serde_as(as = "[_; 240]")]
    bg_pixels_scanline: [Option<PixelInfo>; LCD_WIDTH],
}

impl Default for Layer2 {
    fn default() -> Self {
        Self {
            bg_pixels_scanline: [None; LCD_WIDTH],
        }
    }
}

impl Layer for Layer2 {
    #[allow(unused_variables)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        let idx: usize = y * LCD_WIDTH + x;

        let color_idx = memory.video_ram[idx] as usize;

        // Palette index 0 is transparent
        if color_idx == 0 {
            return None;
        }

        let low_byte = memory.bg_palette_ram[color_idx * 2] as u16;
        let high_byte = memory.bg_palette_ram[color_idx * 2 + 1] as u16;

        Some(PixelInfo {
            color: Color::from_palette_color((high_byte << 8) | low_byte),
            priority: 0,
        })
    }
}
