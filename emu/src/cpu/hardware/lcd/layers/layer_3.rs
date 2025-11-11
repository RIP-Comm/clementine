use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{Color, LCD_WIDTH};

use super::Layer;
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Serialize, Deserialize)]
pub struct Layer3;

impl Layer for Layer3 {
    #[allow(unused_variables)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Mode 2: BG3 is affine/rotation background
        // For now, implement basic bitmap rendering similar to mode 4
        // TODO: Implement proper affine tile-based rendering for mode 2

        let mode = registers.get_bg_mode();

        // BG3 is only available in mode 0 and mode 2
        if !matches!(mode, 0 | 2) {
            return None;
        }

        // For mode 2, render as 8bpp bitmap (simplified, not correct for affine)
        // In mode 2, this should actually be tile-based with rotation/scaling
        if mode == 2 {
            let idx: usize = y * LCD_WIDTH + x;

            // Check if index is within VRAM bounds
            if idx >= memory.video_ram.len() {
                return None;
            }

            let color_idx = memory.video_ram[idx] as usize;

            // Palette index 0 is transparent
            if color_idx == 0 {
                return None;
            }

            // Check palette bounds
            if color_idx * 2 + 1 >= memory.bg_palette_ram.len() {
                return None;
            }

            let low_byte = memory.bg_palette_ram[color_idx * 2] as u16;
            let high_byte = memory.bg_palette_ram[color_idx * 2 + 1] as u16;

            Some(PixelInfo {
                color: Color::from_palette_color((high_byte << 8) | low_byte),
                priority: 0,
            })
        } else {
            // Mode 0: tile-based (not implemented yet)
            None
        }
    }
}
