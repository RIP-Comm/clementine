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

use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::PixelInfo;

use super::{render_text_bg, Layer, TextBgConfig};
use serde::{Deserialize, Serialize};

/// BG0
///
/// A regular (non-affine) tiled background layer available in video modes 0 and 1.
/// Supports scrolling, tile flipping, and both 4bpp and 8bpp color modes.
///
/// See the [module-level documentation](self) for details on how GBA background
/// layers work.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer0;

impl TextBgConfig for Layer0 {
    fn layer_id(&self) -> u8 {
        0
    }

    fn get_scroll(&self, reg: &Registers) -> (u16, u16) {
        (reg.bg0hofs, reg.bg0vofs)
    }

    fn get_screen_size(&self, reg: &Registers) -> (usize, usize) {
        reg.get_bg0_screen_size()
    }

    fn get_screen_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg0_screen_base_block()
    }

    fn get_char_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg0_character_base_block()
    }

    fn get_color_mode(&self, reg: &Registers) -> bool {
        reg.get_bg0_color_mode()
    }

    fn get_priority(&self, reg: &Registers) -> u8 {
        reg.get_bg0_priority()
    }
}

impl Layer for Layer0 {
    fn layer_id(&self) -> u8 {
        0
    }

    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        render_text_bg(self, x, y, memory, registers)
    }
}
