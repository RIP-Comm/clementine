//! # GBA Background Layer 1 (BG1)
//!
//! This module implements BG1, one of the four background layers available on the
//! Game Boy Advance. BG1 is a **regular (text) background** layer, identical in
//! functionality to BG0.
//!
//! ## Availability
//!
//! BG1 is available in the following video modes:
//! - **Mode 0**: All four BG layers (0-3) are regular tiled backgrounds
//! - **Mode 1**: BG0 and BG1 are regular, BG2 is affine, BG3 is disabled
//!
//! In modes 2-5 (affine and bitmap modes), BG1 is not available.
//!
//! See [`layer_0`](super::layer_0) for detailed documentation on how regular
//! tiled backgrounds work.

use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::Color;
use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;

use super::Layer;
use serde::Deserialize;
use serde::Serialize;

/// BG1 - Background Layer 1
///
/// A regular (non-affine) tiled background layer available in video modes 0 and 1.
/// Supports scrolling, tile flipping, and both 4bpp and 8bpp color modes.
///
/// See [`layer_0::Layer0`](super::layer_0::Layer0) for detailed documentation.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer1;

impl Layer for Layer1 {
    /// Renders a single pixel of BG1 at the given screen coordinates.
    ///
    /// See [`Layer0::render`](super::layer_0::Layer0) for algorithm details.
    #[allow(clippy::similar_names)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // apply scrolling offset
        let scroll_x = (x + registers.bg1hofs as usize) % 256;
        let scroll_y = (y + registers.bg1vofs as usize) % 256;

        // calculate which tile this pixel belongs to
        let tile_x = scroll_x / 8;
        let tile_y = scroll_y / 8;

        // calculate pixel position within the tile
        let pixel_x_in_tile = scroll_x % 8;
        let pixel_y_in_tile = scroll_y % 8;

        // screen base block address (each block is 2KB = 0x800)
        let screen_base = registers.get_bg1_screen_base_block() as usize * 0x800;

        // tilemap entry is 2 bytes, arranged in 32x32 grid
        let tilemap_index = tile_y * 32 + tile_x;
        let tilemap_entry_addr = screen_base + tilemap_index * 2;

        // read tilemap entry (16-bit value)
        let tilemap_entry = u16::from_le_bytes([
            memory.video_ram[tilemap_entry_addr],
            memory.video_ram[tilemap_entry_addr + 1],
        ]);

        // take tile number and flags from tilemap entry
        let tile_number = tilemap_entry.get_bits(0..=9) as usize;
        let horizontal_flip = tilemap_entry.get_bit(10);
        let vertical_flip = tilemap_entry.get_bit(11);
        let palette_bank = tilemap_entry.get_bits(12..=15) as usize;

        // flipping to pixel coordinates
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

        // get character base block address (each block is 16KB = 0x4000)
        let char_base = registers.get_bg1_character_base_block() as usize * 0x4000;

        // get palette index based on color mode
        let palette_index = if registers.get_bg1_color_mode() {
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

        // palette index 0 is transparent
        if palette_index == 0 {
            return None;
        }

        // final palette index
        let final_palette_index = if registers.get_bg1_color_mode() {
            // 8bpp: use full 256-color palette
            palette_index
        } else {
            // 4bpp: use 16-color palette bank
            palette_bank * 16 + palette_index
        };

        // read color from BG palette, color is 2 bytes
        let color = Color::from_palette_color(u16::from_le_bytes([
            memory.bg_palette_ram[final_palette_index * 2],
            memory.bg_palette_ram[final_palette_index * 2 + 1],
        ]));

        Some(PixelInfo {
            color,
            priority: registers.get_bg1_priority(),
            layer: 1,
        })
    }
}
