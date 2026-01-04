//! Background Layer 2 (BG2)
//!
//! BG2 is the most versatile background layer, available in all video modes with
//! different capabilities in each:
//!
//! # Mode Support
//!
//! | Mode | BG2 Type | Description                                       |
//! |------|----------|---------------------------------------------------|
//! | 0    | Text     | Regular tiled background                          |
//! | 1    | Text     | Regular tiled background                          |
//! | 2    | Affine   | Rotation/scaling tiled background                 |
//! | 3    | Bitmap   | 240x160 direct color (15-bit RGB)                 |
//! | 4    | Bitmap   | 240x160 paletted (8-bit) with page flipping       |
//! | 5    | Bitmap   | 160x128 direct color with page flipping           |
//!
//! # Affine Backgrounds (Mode 2)
//!
//! Affine backgrounds support rotation and scaling via a 2×2 transformation matrix
//! and reference point:
//!
//! ```text
//! texture_x = PA × screen_x + PB × screen_y + REF_X
//! texture_y = PC × screen_x + PD × screen_y + REF_Y
//! ```
//!
//! Key differences from text backgrounds:
//! - Tilemap entries are 8-bit (tile index only, no flip/palette bits)
//! - Always uses 8bpp color mode (256 colors)
//! - Supports wraparound or clipping at map edges
//! - Map sizes: 128×128, 256×256, 512×512, or 1024×1024 pixels
//!
//! # Bitmap Modes
//!
//! ## Mode 3: Direct Color Bitmap
//! - 240×160 pixels, full screen
//! - Each pixel is 16-bit (15-bit RGB + unused bit)
//! - No page flipping (single frame)
//! - Uses 75KB of VRAM
//!
//! ## Mode 4: Paletted Bitmap
//! - 240×160 pixels, full screen
//! - Each pixel is 8-bit palette index
//! - Two frames for page flipping (DISPCNT bit 4 selects)
//! - Frame 0: VRAM offset 0x0000, Frame 1: offset 0xA000
//!
//! ## Mode 5: Small Direct Color
//! - 160×128 pixels (smaller than screen)
//! - Each pixel is 16-bit (15-bit RGB)
//! - Two frames for page flipping
//! - Pixels outside 160×128 area are transparent

use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{Color, PixelInfo};

use super::{AffineBgConfig, Layer, TextBgConfig, render_affine_bg, render_text_bg};
use serde::{Deserialize, Serialize};

/// BG2
///
/// The most versatile layer, supporting text mode (modes 0-1), affine mode (mode 2),
/// and bitmap modes (modes 3-5). See [module documentation](self) for details.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer2;

// Text mode configuration (modes 0-1)
impl TextBgConfig for Layer2 {
    fn layer_id(&self) -> u8 {
        2
    }

    fn get_scroll(&self, reg: &Registers) -> (u16, u16) {
        (reg.bg2hofs, reg.bg2vofs)
    }

    fn get_screen_size(&self, reg: &Registers) -> (usize, usize) {
        reg.get_bg2_screen_size()
    }

    fn get_screen_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg2_screen_base_block()
    }

    fn get_char_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg2_character_base_block()
    }

    fn get_color_mode(&self, reg: &Registers) -> bool {
        reg.get_bg2_color_mode()
    }

    fn get_priority(&self, reg: &Registers) -> u8 {
        reg.get_bg2_priority()
    }
}

// Affine mode configuration (mode 2)
impl AffineBgConfig for Layer2 {
    fn layer_id(&self) -> u8 {
        2
    }

    #[allow(clippy::cast_possible_wrap)]
    fn get_affine_params(&self, reg: &Registers) -> (i16, i16, i16, i16) {
        (
            reg.bg2pa as i16,
            reg.bg2pb as i16,
            reg.bg2pc as i16,
            reg.bg2pd as i16,
        )
    }

    #[allow(clippy::cast_possible_wrap)]
    fn get_reference_point(&self, reg: &Registers) -> (i32, i32) {
        (reg.bg2x as i32, reg.bg2y as i32)
    }

    fn get_bg_control(&self, reg: &Registers) -> u16 {
        reg.bg2cnt
    }
}

impl Layer for Layer2 {
    fn layer_id(&self) -> u8 {
        2
    }

    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        match registers.get_bg_mode() {
            0 | 1 => render_text_bg(self, x, y, memory, registers),
            2 => render_affine_bg(self, x, y, memory, registers),
            3 => Self::render_mode3(x, y, memory),
            4 => Self::render_mode4(x, y, memory, registers),
            5 => Self::render_mode5(x, y, memory),
            _ => None,
        }
    }
}

impl Layer2 {
    /// Render BG2 in Mode 3 (240×160, 15-bit direct color bitmap).
    fn render_mode3(x: usize, y: usize, memory: &Memory) -> Option<PixelInfo> {
        // Mode 3: linear bitmap, 2 bytes per pixel
        let offset = (y * 240 + x) * 2;

        if offset + 1 >= memory.video_ram.len() {
            return None;
        }

        let color = u16::from_le_bytes([memory.video_ram[offset], memory.video_ram[offset + 1]]);

        Some(PixelInfo {
            color: Color::from_palette_color(color),
            priority: 0,
            layer: 2,
        })
    }

    /// Render BG2 in Mode 4 (240×160, 8-bit paletted bitmap with page flipping).
    fn render_mode4(
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Frame select: bit 4 of DISPCNT
        let frame_offset = if registers.dispcnt.get_bit(4) {
            0xA000
        } else {
            0
        };
        let offset = frame_offset + y * 240 + x;

        if offset >= memory.video_ram.len() {
            return None;
        }

        let palette_index = memory.video_ram[offset] as usize;

        // Palette index 0 is transparent
        if palette_index == 0 {
            return None;
        }

        let color = u16::from_le_bytes([
            memory.bg_palette_ram[palette_index * 2],
            memory.bg_palette_ram[palette_index * 2 + 1],
        ]);

        Some(PixelInfo {
            color: Color::from_palette_color(color),
            priority: 0,
            layer: 2,
        })
    }

    /// Render BG2 in Mode 5 (160×128, 15-bit direct color bitmap with page flipping).
    fn render_mode5(x: usize, y: usize, memory: &Memory) -> Option<PixelInfo> {
        // Mode 5: smaller resolution, transparent outside
        if x >= 160 || y >= 128 {
            return None;
        }

        let offset = (y * 160 + x) * 2;

        if offset + 1 >= memory.video_ram.len() {
            return None;
        }

        let color = u16::from_le_bytes([memory.video_ram[offset], memory.video_ram[offset + 1]]);

        Some(PixelInfo {
            color: Color::from_palette_color(color),
            priority: 0,
            layer: 2,
        })
    }
}
