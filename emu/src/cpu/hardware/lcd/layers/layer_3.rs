//! Background Layer 3 (BG3) - Regular or affine tiled background.
//!
//! BG3's capabilities depend on the video mode:
//!
//! | Mode | BG3 Type | Status                |
//! |------|----------|-----------------------|
//! | 0    | Text     | Not yet implemented   |
//! | 2    | Affine   | Implemented           |
//!
//! BG3 is not available in modes 1, 3, 4, or 5.
//!
//! # Affine Mode (Mode 2)
//!
//! In mode 2, BG3 functions as an affine background with rotation and scaling.
//! See [`layer_2`](super::layer_2) for detailed affine transformation documentation.
//!
//! BG3 uses its own set of affine registers:
//! - `BG3PA`, `BG3PB`, `BG3PC`, `BG3PD`: Transformation matrix
//! - `BG3X`, `BG3Y`: Reference point

use super::Layer;
use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{Color, PixelInfo};
use serde::Deserialize;
use serde::Serialize;

/// BG3 - Background Layer 3
///
/// Supports regular tiled mode (mode 0, not implemented) and affine mode (mode 2).
/// See [module documentation](self) for details.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer3;

impl Layer for Layer3 {
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        let mode = registers.get_bg_mode();

        // BG3 is only available in mode 0 and mode 2
        // In mode 2, it's an affine background
        if mode == 2 {
            Self::render_affine(x, y, memory, registers)
        } else if mode == 0 {
            Self::render_text(x, y, memory, registers)
        } else {
            None
        }
    }
}

impl Layer3 {
    /// Render BG3 as a regular tiled (text) background in Mode 0.
    #[allow(clippy::similar_names)]
    fn render_text(
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Step 1: Apply scrolling offset
        let scroll_x = (x + registers.bg3hofs as usize) % 256;
        let scroll_y = (y + registers.bg3vofs as usize) % 256;

        // Calculate which tile this pixel belongs to (tiles are 8x8)
        let tile_x = scroll_x / 8;
        let tile_y = scroll_y / 8;

        // Calculate pixel position within the tile
        let pixel_x_in_tile = scroll_x % 8;
        let pixel_y_in_tile = scroll_y % 8;

        // Get screen base block address (each block is 2KB = 0x800)
        let screen_base = registers.get_bg3_screen_base_block() as usize * 0x800;

        // Each tilemap entry is 2 bytes, arranged in 32x32 grid
        let tilemap_index = tile_y * 32 + tile_x;
        let tilemap_entry_addr = screen_base + tilemap_index * 2;

        // Read tilemap entry (16-bit value)
        let tilemap_entry = u16::from_le_bytes([
            memory.video_ram[tilemap_entry_addr],
            memory.video_ram[tilemap_entry_addr + 1],
        ]);

        // Extract tile number and flags from tilemap entry
        let tile_number = tilemap_entry.get_bits(0..=9) as usize;
        let horizontal_flip = tilemap_entry.get_bit(10);
        let vertical_flip = tilemap_entry.get_bit(11);
        let palette_bank = tilemap_entry.get_bits(12..=15) as usize;

        // Apply flipping to pixel coordinates
        let final_pixel_x = if horizontal_flip {
            7 - pixel_x_in_tile
        } else {
            pixel_x_in_tile
        };
        let final_pixel_y = if vertical_flip {
            7 - pixel_y_in_tile
        } else {
            pixel_y_in_tile
        };

        // Get character base block address (each block is 16KB = 0x4000)
        let char_base = registers.get_bg3_character_base_block() as usize * 0x4000;

        // Get palette index based on color mode
        let palette_index = if registers.get_bg3_color_mode() {
            // 8bpp mode: each pixel is 1 byte
            let tile_data_offset = char_base + tile_number * 64 + final_pixel_y * 8 + final_pixel_x;
            memory.video_ram[tile_data_offset] as usize
        } else {
            // 4bpp mode: each pixel is 4 bits (2 pixels per byte)
            let tile_data_offset =
                char_base + tile_number * 32 + final_pixel_y * 4 + final_pixel_x / 2;
            let byte = memory.video_ram[tile_data_offset];

            if final_pixel_x % 2 == 0 {
                byte.get_bits(0..=3) as usize
            } else {
                byte.get_bits(4..=7) as usize
            }
        };

        // Palette index 0 is transparent
        if palette_index == 0 {
            return None;
        }

        // Calculate final palette address
        let final_palette_index = if registers.get_bg3_color_mode() {
            // 8bpp: use full 256-color palette
            palette_index
        } else {
            // 4bpp: use 16-color palette bank
            palette_bank * 16 + palette_index
        };

        // Read color from BG palette (each color is 2 bytes)
        let color = Color::from_palette_color(u16::from_le_bytes([
            memory.bg_palette_ram[final_palette_index * 2],
            memory.bg_palette_ram[final_palette_index * 2 + 1],
        ]));

        Some(PixelInfo {
            color,
            priority: registers.get_bg3_priority(),
            layer: 3,
        })
    }

    /// Render BG3 as an affine (rotation/scaling) background in Mode 2
    fn render_affine(
        screen_x: usize,
        screen_y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Read affine parameters (8.8 fixed point)
        let pa = registers.bg3pa as i16; // dx
        let pb = registers.bg3pb as i16; // dmx
        let pc = registers.bg3pc as i16; // dy
        let pd = registers.bg3pd as i16; // dmy

        // Read reference point (24.8 fixed point)
        let ref_x = registers.bg3x as i32;
        let ref_y = registers.bg3y as i32;

        // Apply affine transformation: texture = matrix * screen + displacement
        let texture_x = (pa as i32 * screen_x as i32) + (pb as i32 * screen_y as i32) + ref_x;
        let texture_y = (pc as i32 * screen_x as i32) + (pd as i32 * screen_y as i32) + ref_y;

        // Convert from 8.8 fixed point to integer (shift right by 8)
        let tex_x = texture_x >> 8;
        let tex_y = texture_y >> 8;

        // Read BG3 control register
        let bg3cnt = registers.bg3cnt;
        let screen_size = bg3cnt.get_bits(14..=15);
        let char_base = bg3cnt.get_bits(2..=3) as usize;
        let screen_base = bg3cnt.get_bits(8..=12) as usize;
        let wraparound = bg3cnt.get_bit(13);

        // Get tilemap dimensions
        let (map_width, map_height) = match screen_size {
            0 => (128, 128),
            1 => (256, 256),
            2 => (512, 512),
            3 => (1024, 1024),
            _ => unreachable!(),
        };

        // Handle wraparound/clipping
        let final_x = if wraparound {
            tex_x.rem_euclid(map_width)
        } else if tex_x < 0 || tex_x >= map_width {
            return None;
        } else {
            tex_x
        };

        let final_y = if wraparound {
            tex_y.rem_euclid(map_height)
        } else if tex_y < 0 || tex_y >= map_height {
            return None;
        } else {
            tex_y
        };

        // Calculate tile and pixel coordinates
        let tile_x = (final_x / 8) as usize;
        let tile_y = (final_y / 8) as usize;
        let pixel_x = (final_x % 8) as usize;
        let pixel_y = (final_y % 8) as usize;

        let tiles_per_row = (map_width / 8) as usize;

        // Affine tilemap: flat linear layout, 1 byte per entry
        let tilemap_offset = screen_base * 2048;
        let tile_index_offset = tilemap_offset + tile_y * tiles_per_row + tile_x;

        if tile_index_offset >= memory.video_ram.len() {
            return None;
        }

        let tile_index = memory.video_ram[tile_index_offset] as usize;

        // Affine backgrounds use 8bpp tiles, 64 bytes per tile
        let char_base_offset = char_base * 0x4000;
        let tile_data_offset = char_base_offset + tile_index * 64;
        let pixel_offset = tile_data_offset + pixel_y * 8 + pixel_x;

        if pixel_offset >= memory.video_ram.len() {
            return None;
        }

        let palette_index = memory.video_ram[pixel_offset] as usize;

        // Palette index 0 is transparent
        if palette_index == 0 {
            return None;
        }

        // Read color from BG palette
        if palette_index * 2 + 1 >= memory.bg_palette_ram.len() {
            return None;
        }

        let low_byte = memory.bg_palette_ram[palette_index * 2] as u16;
        let high_byte = memory.bg_palette_ram[palette_index * 2 + 1] as u16;

        // Get priority from BG3CNT
        let priority = bg3cnt.get_bits(0..=1) as u8;

        Some(PixelInfo {
            color: Color::from_palette_color((high_byte << 8) | low_byte),
            priority,
            layer: 3,
        })
    }
}
