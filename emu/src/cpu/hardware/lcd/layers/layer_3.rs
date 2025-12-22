use super::Layer;
use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{Color, PixelInfo};
use serde::Deserialize;
use serde::Serialize;

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
            self.render_affine(x, y, memory, registers)
        } else if mode == 0 {
            // Mode 0: regular tiled background (not yet implemented)
            None
        } else {
            None
        }
    }
}

impl Layer3 {
    /// Render BG3 as an affine (rotation/scaling) background in Mode 2
    fn render_affine(
        &self,
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
        })
    }
}
