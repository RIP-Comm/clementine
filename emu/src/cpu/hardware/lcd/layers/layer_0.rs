//! # GBA Background Layer 0 (BG0)
//!
//! This module implements BG0, one of the four background layers available on the
//! Game Boy Advance. BG0 is a **regular (text) background** layer, as opposed to
//! affine (rotation/scaling) backgrounds.
//!
//! ## Availability
//!
//! BG0 is available in the following video modes:
//! - **Mode 0**: All four BG layers (0-3) are regular tiled backgrounds
//! - **Mode 1**: BG0 and BG1 are regular, BG2 is affine, BG3 is disabled
//!
//! In modes 2-5 (affine and bitmap modes), BG0 is not available.
//!
//! ## Tilemap Architecture
//!
//! GBA backgrounds use a **tile-based rendering system**:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    VRAM Layout                          │
//! ├─────────────────────────────────────────────────────────┤
//! │  Character Base Blocks (Tile Graphics)                  │
//! │  ├── Block 0: 0x06000000 - 0x06003FFF (16 KB)          │
//! │  ├── Block 1: 0x06004000 - 0x06007FFF (16 KB)          │
//! │  ├── Block 2: 0x06008000 - 0x0600BFFF (16 KB)          │
//! │  └── Block 3: 0x0600C000 - 0x0600FFFF (16 KB)          │
//! ├─────────────────────────────────────────────────────────┤
//! │  Screen Base Blocks (Tilemaps)                          │
//! │  └── 32 blocks × 2 KB each, within the 64 KB VRAM      │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ### Tiles
//!
//! Each tile is an 8×8 pixel graphic stored in VRAM:
//! - **4bpp mode**: 32 bytes per tile (4 bits per pixel, 16 colors per palette bank)
//! - **8bpp mode**: 64 bytes per tile (8 bits per pixel, 256 colors)
//!
//! ### Tilemap Entries
//!
//! The tilemap is a 32×32 grid of 16-bit entries (2 KB total per screen block):
//!
//! ```text
//! 15 14 13 12 | 11 | 10 |  9  8  7  6  5  4  3  2  1  0
//! ─────────────────────────────────────────────────────
//!  Palette    | VF | HF |        Tile Number
//!   Bank      |    |    |        (0-1023)
//! ```
//!
//! - **Bits 0-9**: Tile number (index into character base block)
//! - **Bit 10**: Horizontal flip
//! - **Bit 11**: Vertical flip
//! - **Bits 12-15**: Palette bank (4bpp mode only)
//!
//! ## Scrolling
//!
//! BG0 supports hardware scrolling via the `BG0HOFS` and `BG0VOFS` registers:
//! - Horizontal offset: 0-511 pixels (9-bit value)
//! - Vertical offset: 0-511 pixels (9-bit value)
//!
//! The visible 240×160 screen is a "window" into a larger virtual background
//! that wraps around at the edges.
//!
//! ## Color Modes
//!
//! ### 4bpp (16 colors)
//! - Each tile uses one of 16 palette banks (16 colors each)
//! - Palette bank is specified in the tilemap entry
//! - More tiles fit in VRAM, but fewer colors per tile
//!
//! ### 8bpp (256 colors)
//! - Each tile can use any of the 256 BG palette colors
//! - Palette bank bits in tilemap are ignored
//! - Fewer tiles fit in VRAM, but full color range per tile
//!
//! ## Transparency
//!
//! Palette index 0 is always transparent, regardless of color mode.
//! When a pixel has palette index 0, it is not drawn, allowing lower-priority
//! layers or the backdrop color to show through.
//!
//! ## Priority
//!
//! BG0's priority (0-3) is set in the `BG0CNT` register. Lower values mean
//! higher priority (drawn on top). When multiple layers have the same priority,
//! the layer with the lower number (BG0 < BG1 < BG2 < BG3) is drawn on top.

use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::Color;
use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;

use super::Layer;
use serde::Deserialize;
use serde::Serialize;

/// BG0 - Background Layer 0
///
/// A regular (non-affine) tiled background layer available in video modes 0 and 1.
/// Supports scrolling, tile flipping, and both 4bpp and 8bpp color modes.
///
/// See the [module-level documentation](self) for details on how GBA background
/// layers work.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer0;

impl Layer for Layer0 {
    /// Renders a single pixel of BG0 at the given screen coordinates.
    ///
    /// # Algorithm
    ///
    /// 1. **Apply scrolling**: Add `BG0HOFS`/`BG0VOFS` to screen coordinates
    /// 2. **Find tile**: Divide by 8 to get tile coordinates in the 32×32 tilemap
    /// 3. **Read tilemap entry**: Get tile number, flip flags, and palette bank
    /// 4. **Apply flipping**: Mirror pixel coordinates if flip flags are set
    /// 5. **Read tile data**: Get palette index from character data (4bpp or 8bpp)
    /// 6. **Check transparency**: Return `None` if palette index is 0
    /// 7. **Look up color**: Read final color from BG palette RAM
    ///
    /// # Arguments
    ///
    /// * `x` - Screen X coordinate (0-239)
    /// * `y` - Screen Y coordinate (0-159)
    /// * `memory` - Reference to VRAM and palette RAM
    /// * `registers` - LCD control registers
    ///
    /// # Returns
    ///
    /// - `Some(PixelInfo)` with the color and priority if the pixel is opaque
    /// - `None` if the pixel is transparent (palette index 0)
    #[allow(clippy::similar_names)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // Step 1: Apply scrolling offset
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
            layer: 0,
        })
    }
}
