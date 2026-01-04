//! LCD Memory - VRAM, Palette RAM, and OAM.
//!
//! This module contains the memory regions used by the GBA's Picture Processing Unit (PPU)
//! for rendering graphics. These regions are separate from main RAM and have specific
//! purposes in the rendering pipeline.
//!
//! # Memory Map
//!
//! | Region          | Address Range           | Size    | Purpose                          |
//! |-----------------|-------------------------|---------|----------------------------------|
//! | BG Palette RAM  | 0x0500_0000-0x0500_01FF | 512 B   | Background color palettes        |
//! | OBJ Palette RAM | 0x0500_0200-0x0500_03FF | 512 B   | Sprite color palettes            |
//! | VRAM            | 0x0600_0000-0x0601_7FFF | 96 KB   | Tile data and tilemaps           |
//! | OAM             | 0x0700_0000-0x0700_03FF | 1 KB    | Sprite attributes                |
//!
//! # Palette RAM Layout
//!
//! Each palette RAM region holds 256 colors (512 bytes, 2 bytes per color).
//! Colors are stored in RGB555 format (5 bits per channel, 1 bit unused).
//!
//! For 4bpp (16-color) mode, palette RAM is divided into 16 banks of 16 colors each:
//! ```text
//! Bank 0: colors 0-15   (offset 0x00-0x1F)
//! Bank 1: colors 16-31  (offset 0x20-0x3F)
//! ...
//! Bank 15: colors 240-255 (offset 0x1E0-0x1FF)
//! ```
//!
//! For 8bpp (256-color) mode, all 256 colors are used as a single palette.
//!
//! **Important**: Color index 0 is always transparent for both backgrounds and sprites.
//!
//! # VRAM Layout
//!
//! VRAM is organized differently depending on the video mode:
//!
//! ## Tile Modes (Modes 0-2)
//! ```text
//! 0x0600_0000-0x0600_FFFF: Character blocks 0-3 (tile pixel data, 64KB)
//!   - Block 0: 0x0600_0000 (16KB)
//!   - Block 1: 0x0600_4000 (16KB)
//!   - Block 2: 0x0600_8000 (16KB)
//!   - Block 3: 0x0600_C000 (16KB)
//!
//! 0x0601_0000-0x0601_7FFF: Screen blocks 0-31 (tilemaps, 32KB)
//!   - Each screen block is 2KB (32x32 tiles, 2 bytes per entry)
//!   - Note: Screen blocks share space with character blocks 2-3
//!
//! 0x0601_0000-0x0601_7FFF: OBJ tile data (sprites use this region)
//! ```
//!
//! ## Bitmap Modes (Modes 3-5)
//! ```text
//! Mode 3: 0x0600_0000-0x0601_2BFF - Single 240x160 frame (16bpp, ~75KB)
//! Mode 4: 0x0600_0000-0x0600_9FFF - Frame 0 (8bpp, 40KB)
//!         0x0600_A000-0x0601_3FFF - Frame 1 (8bpp, 40KB)
//! Mode 5: 0x0600_0000-0x0600_9FFF - Frame 0 (160x128, 16bpp)
//!         0x0600_A000-0x0601_3FFF - Frame 1 (160x128, 16bpp)
//! ```
//!
//! # OAM (Object Attribute Memory)
//!
//! OAM stores attributes for up to 128 sprites. Each sprite uses 8 bytes:
//! - Bytes 0-1: Attribute 0 (Y position, mode, shape)
//! - Bytes 2-3: Attribute 1 (X position, flip/rotation, size)
//! - Bytes 4-5: Attribute 2 (tile index, priority, palette)
//! - Bytes 6-7: Rotation/scaling parameter (shared across 4 sprites)
//!
//! See [`object_attributes`](super::object_attributes) for detailed OAM format.

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// LCD memory regions for graphics rendering.
///
/// Contains VRAM, palette RAM, and OAM - all the memory the PPU needs
/// to render backgrounds and sprites. These are stored as boxed arrays
/// to avoid stack overflow (total ~98KB).
#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct Memory {
    /// Background palette RAM (`0x0500_0000` - `0x0500_01FF`).
    ///
    /// 512 bytes storing 256 colors in RGB555 format.
    /// Used by background layers (BG0-BG3) to look up final pixel colors.
    ///
    /// - 4bpp mode: 16 palettes × 16 colors each
    /// - 8bpp mode: 1 palette × 256 colors
    ///
    /// Color 0 of each palette (or global color 0 in 8bpp) is transparent.
    #[serde_as(as = "Box<[_; 512]>")]
    pub bg_palette_ram: Box<[u8; 0x200]>,

    /// Object (sprite) palette RAM (`0x0500_0200` - `0x0500_03FF`).
    ///
    /// 512 bytes storing 256 colors in RGB555 format.
    /// Used by sprites to look up final pixel colors.
    ///
    /// - 4bpp mode: 16 palettes × 16 colors each (palette selected per-sprite)
    /// - 8bpp mode: 1 palette × 256 colors
    ///
    /// Color 0 (or index 0 within each 4bpp palette bank) is transparent.
    #[serde_as(as = "Box<[_; 512]>")]
    pub obj_palette_ram: Box<[u8; 0x200]>,

    /// Video RAM (`0x0600_0000` - `0x0601_7FFF`).
    ///
    /// 96KB of memory storing:
    /// - Tile pixel data (character blocks) for backgrounds and sprites
    /// - Tilemaps (screen blocks) defining which tiles appear where
    /// - Bitmap frame buffers in modes 3-5
    ///
    /// Layout varies by video mode - see module documentation for details.
    #[serde_as(as = "Box<[_; 98304]>")]
    pub video_ram: Box<[u8; 0x18000]>,

    /// Object Attribute Memory (`0x0700_0000` - `0x0700_03FF`).
    ///
    /// 1KB storing attributes for 128 sprites (8 bytes each).
    /// Defines sprite position, size, tile, palette, priority, and transformation.
    ///
    /// Also contains 32 rotation/scaling parameter sets (interleaved with sprite data).
    /// See [`object_attributes`](super::object_attributes) for the detailed format.
    #[serde_as(as = "Box<[_; 1024]>")]
    pub obj_attributes: Box<[u8; 0x400]>,
}

impl Default for Memory {
    #[allow(clippy::large_stack_arrays)]
    fn default() -> Self {
        Self {
            bg_palette_ram: Box::new([0; 0x200]),
            obj_palette_ram: Box::new([8; 0x200]),
            video_ram: Box::new([0; 0x18000]),
            obj_attributes: Box::new([0; 0x400]),
        }
    }
}
