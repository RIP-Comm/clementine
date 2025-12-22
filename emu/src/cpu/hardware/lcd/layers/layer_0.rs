use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::Color;
use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;

use super::Layer;
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Serialize, Deserialize)]
pub struct Layer0;

impl Layer for Layer0 {
    #[allow(clippy::similar_names)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Apply scrolling offset
        let scroll_x = (x + registers.bg0hofs as usize) % 256;
        let scroll_y = (y + registers.bg0vofs as usize) % 256;

        // Calculate which tile this pixel belongs to (tiles are 8x8)
        let tile_x = scroll_x / 8;
        let tile_y = scroll_y / 8;

        // Calculate pixel position within the tile
        let pixel_x_in_tile = scroll_x % 8;
        let pixel_y_in_tile = scroll_y % 8;

        // Get screen base block address (each block is 2KB = 0x800)
        let screen_base = registers.get_bg0_screen_base_block() as usize * 0x800;

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
        let char_base = registers.get_bg0_character_base_block() as usize * 0x4000;

        // Get palette index based on color mode
        let palette_index = if registers.get_bg0_color_mode() {
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
        let final_palette_index = if registers.get_bg0_color_mode() {
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
            priority: registers.get_bg0_priority(),
        })
    }
}
