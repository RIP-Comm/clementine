use super::Layer;
use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{Color, PixelInfo};
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Serialize, Deserialize)]
pub struct Layer2;

impl Layer for Layer2 {
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        let mode = registers.get_bg_mode();

        // BG2 is available in modes 0, 1, 2
        // In mode 2, it's an affine background
        match mode {
            0 | 1 => {
                // Mode 0, 1: regular tiled background (not yet implemented)
                None
            }
            2 => Self::render_affine(x, y, memory, registers),
            3 => Self::render_mode3(x, y, memory),
            4 => Self::render_mode4(x, y, memory, registers),
            5 => Self::render_mode5(x, y, memory),
            _ => None,
        }
    }
}

impl Layer2 {
    /// Render BG2 as an affine (rotation/scaling) background in Mode 2
    fn render_affine(
        screen_x: usize,
        screen_y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Read affine parameters (8.8 fixed point)
        let pa = registers.bg2pa as i16; // dx
        let pb = registers.bg2pb as i16; // dmx
        let pc = registers.bg2pc as i16; // dy
        let pd = registers.bg2pd as i16; // dmy

        // Read reference point (24.8 fixed point)
        let ref_x = registers.bg2x as i32;
        let ref_y = registers.bg2y as i32;

        // Apply affine transformation: texture = matrix * screen + displacement
        // texture_x = pa * screen_x + pb * screen_y + ref_x
        // texture_y = pc * screen_x + pd * screen_y + ref_y
        let texture_x = (pa as i32 * screen_x as i32) + (pb as i32 * screen_y as i32) + ref_x;
        let texture_y = (pc as i32 * screen_x as i32) + (pd as i32 * screen_y as i32) + ref_y;

        // Convert from 8.8 fixed point to integer (shift right by 8)
        let tex_x = texture_x >> 8;
        let tex_y = texture_y >> 8;

        // Read BG2 control register
        let bg2cnt = registers.bg2cnt;
        let screen_size = bg2cnt.get_bits(14..=15); // Screen size
        let char_base = bg2cnt.get_bits(2..=3) as usize; // Character base block
        let screen_base = bg2cnt.get_bits(8..=12) as usize; // Screen base block
        let wraparound = bg2cnt.get_bit(13); // Display area overflow

        // Get tilemap dimensions based on screen size
        let (map_width, map_height) = match screen_size {
            0 => (128, 128),   // 16x16 tiles
            1 => (256, 256),   // 32x32 tiles
            2 => (512, 512),   // 64x64 tiles
            3 => (1024, 1024), // 128x128 tiles
            _ => unreachable!(),
        };

        // Handle wraparound/clipping
        let final_x = if wraparound {
            tex_x.rem_euclid(map_width)
        } else if tex_x < 0 || tex_x >= map_width {
            return None; // Out of bounds, transparent
        } else {
            tex_x
        };

        let final_y = if wraparound {
            tex_y.rem_euclid(map_height)
        } else if tex_y < 0 || tex_y >= map_height {
            return None; // Out of bounds, transparent
        } else {
            tex_y
        };

        // Calculate tile coordinates and pixel offset within tile
        let tile_x = (final_x / 8) as usize;
        let tile_y = (final_y / 8) as usize;
        let pixel_x = (final_x % 8) as usize;
        let pixel_y = (final_y % 8) as usize;

        // Get tilemap width in tiles
        let tiles_per_row = (map_width / 8) as usize;

        // Affine tilemap: flat linear layout, 1 byte per entry (tile index only)
        let tilemap_offset = screen_base * 2048; // Each screen block is 2KB
        let tile_index_offset = tilemap_offset + tile_y * tiles_per_row + tile_x;

        if tile_index_offset >= memory.video_ram.len() {
            return None;
        }

        let tile_index = memory.video_ram[tile_index_offset] as usize;

        // Affine backgrounds use 256-color tiles (8bpp), 64 bytes per tile
        let char_base_offset = char_base * 0x4000; // 16KB per character base block
        let tile_data_offset = char_base_offset + tile_index * 64;

        // Get pixel within tile (8bpp: 1 byte per pixel)
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

        // Get priority from BG2CNT
        let priority = bg2cnt.get_bits(0..=1) as u8;

        Some(PixelInfo {
            color: Color::from_palette_color((high_byte << 8) | low_byte),
            priority,
        })
    }

    /// Render BG2 in Mode 3 (240x160, 15-bit direct color bitmap)
    fn render_mode3(x: usize, y: usize, memory: &Memory) -> Option<PixelInfo> {
        // Mode 3: 240x160 pixels, 16-bit color (15-bit RGB + unused bit)
        // VRAM layout: linear bitmap starting at 0x06000000
        let offset = (y * 240 + x) * 2;

        if offset + 1 >= memory.video_ram.len() {
            return None;
        }

        let low_byte = memory.video_ram[offset] as u16;
        let high_byte = memory.video_ram[offset + 1] as u16;
        let color = (high_byte << 8) | low_byte;

        Some(PixelInfo {
            color: Color::from_palette_color(color),
            priority: 0,
        })
    }

    /// Render BG2 in Mode 4 (240x160, 8-bit paletted bitmap with page flipping)
    fn render_mode4(
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Mode 4: 240x160 pixels, 8-bit palette indices
        // Two frames: frame 0 at 0x06000000, frame 1 at 0x0600A000
        let frame_select = registers.dispcnt.get_bit(4);
        let base_offset = if frame_select { 0xA000 } else { 0 };

        let offset = base_offset + y * 240 + x;

        if offset >= memory.video_ram.len() {
            return None;
        }

        let palette_index = memory.video_ram[offset] as usize;

        // Palette index 0 is transparent
        if palette_index == 0 {
            return None;
        }

        if palette_index * 2 + 1 >= memory.bg_palette_ram.len() {
            return None;
        }

        let low_byte = memory.bg_palette_ram[palette_index * 2] as u16;
        let high_byte = memory.bg_palette_ram[palette_index * 2 + 1] as u16;

        Some(PixelInfo {
            color: Color::from_palette_color((high_byte << 8) | low_byte),
            priority: 0,
        })
    }

    /// Render BG2 in Mode 5 (160x128, 15-bit direct color bitmap with page flipping)
    fn render_mode5(x: usize, y: usize, memory: &Memory) -> Option<PixelInfo> {
        // Mode 5: 160x128 pixels, 16-bit color
        // Smaller resolution than screen, centered or scaled
        if x >= 160 || y >= 128 {
            return None;
        }

        let offset = (y * 160 + x) * 2;

        if offset + 1 >= memory.video_ram.len() {
            return None;
        }

        let low_byte = memory.video_ram[offset] as u16;
        let high_byte = memory.video_ram[offset + 1] as u16;
        let color = (high_byte << 8) | low_byte;

        Some(PixelInfo {
            color: Color::from_palette_color(color),
            priority: 0,
        })
    }
}
