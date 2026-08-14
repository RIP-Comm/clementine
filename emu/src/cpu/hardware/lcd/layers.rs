#![allow(clippy::cast_possible_truncation)]

//! Background and sprite layer rendering.
//!
//! The GBA PPU composites multiple layers to produce the final image. This module
//! defines the [`Layer`] trait and provides implementations for each layer type.
//!
//! # Background Rendering Modes
//!
//! The GBA has three different ways to render backgrounds:
//!
//! ## 1. Text Mode (Tiled Backgrounds)
//!
//! The most common mode, used by most 2D games. The background is built from
//! 8×8 pixel **tiles** arranged in a grid (called a **tilemap**). Think of it
//! like a mosaic: you define a set of tile images in VRAM, then reference them
//! by index in the tilemap to form the full background.
//!
//! - **Scrolling**: Simple X/Y pixel offset (no rotation or scaling)
//! - **Tilemap entries**: 16-bit, containing tile index + H/V flip + palette bank
//! - **Color depth**: 4bpp (16 colors, 16 palette banks) or 8bpp (256 colors)
//! - **Map sizes**: 256×256, 512×256, 256×512, or 512×512 pixels
//! - **Per-tile flipping**: Hardware horizontal/vertical flip flags
//!
//! Available on: BG0, BG1 (modes 0-1), BG2, BG3 (mode 0 only)
//!
//! ## 2. Affine Mode (Rotated/Scaled Tiled Backgrounds)
//!
//! Like text mode (still tile-based), but with hardware **rotation and scaling**.
//! The background can be rotated to any angle, zoomed in/out, or skewed using a
//! 2×2 transformation matrix applied per-pixel.
//!
//! - **Transformation**: 2×2 matrix (PA, PB, PC, PD) + reference point (X, Y)
//! - **Tilemap entries**: 8-bit (tile index only - no flip flags or palette)
//! - **Color depth**: Always 8bpp (256 colors)
//! - **Map sizes**: 128×128, 256×256, 512×512, or 1024×1024 (square only)
//! - **Edge behavior**: Configurable wraparound or transparent clipping
//!
//! Available on: BG2 (modes 1-2), BG3 (mode 2 only)
//!
//! ## 3. Bitmap Mode (Direct Pixel Data)
//!
//! Instead of tiles, the background is a raw image stored directly in VRAM.
//! Simpler to program but uses much more memory (no tile reuse).
//!
//! - **Mode 3**: 240×160, 15-bit direct color (32K colors), single buffer
//! - **Mode 4**: 240×160, 8-bit palette indexed, double buffered (page flip)
//! - **Mode 5**: 160×128, 15-bit direct color, double buffered
//!
//! Available on: BG2 only (modes 3, 4, 5)
//!
//! # Sprites (OBJ Layer)
//!
//! Sprites are independent graphical objects that can be positioned anywhere on
//! screen, overlapping backgrounds and each other. The GBA supports 128 hardware
//! sprites, configured via OAM (Object Attribute Memory).
//!
//! - **Sizes**: 8×8 up to 64×64 pixels (various rectangular combinations)
//! - **Affine transform**: Optional per-sprite rotation/scaling (32 matrix slots)
//! - **Color depth**: 4bpp (16 colors) or 8bpp (256 colors) per sprite
//! - **Priority**: 0-3, controls drawing order relative to backgrounds
//! - **Tile storage**: Sprites use the upper half of VRAM character data
//!
//! # Video Modes Summary
//!
//! The DISPCNT register (bits 0-2) selects which video mode to use:
//!
//! | Mode | BG0     | BG1     | BG2     | BG3     | Notes                    |
//! |------|---------|---------|---------|---------|--------------------------|
//! | 0    | Text    | Text    | Text    | Text    | 4 tiled layers           |
//! | 1    | Text    | Text    | Affine  | -       | 2 tiled + 1 rotatable    |
//! | 2    | -       | -       | Affine  | Affine  | 2 rotatable layers       |
//! | 3    | -       | -       | Bitmap  | -       | Full-screen direct color |
//! | 4    | -       | -       | Bitmap  | -       | Indexed, page-flipping   |
//! | 5    | -       | -       | Bitmap  | -       | Smaller, page-flipping   |
//!
//! Sprites (OBJ layer) are available in all modes.
//!
//! # Text vs Affine Comparison
//!
//! | Feature           | Text Mode          | Affine Mode              |
//! |-------------------|--------------------|--------------------------|
//! | Rotation/Scaling  | No                 | Yes (2×2 matrix)         |
//! | Tilemap entry     | 16-bit             | 8-bit                    |
//! | Per-tile flipping | H/V flip supported | Not supported            |
//! | Color depths      | 4bpp or 8bpp       | 8bpp only                |
//! | Map shapes        | Rectangular        | Square only              |
//! | Max map size      | 512×512            | 1024×1024                |
//! | Edge behavior     | Always wraps       | Wrap or clip             |
//!
//! # Rendering Pipeline
//!
//! For each pixel, the LCD controller:
//! 1. Calls [`Layer::render`] on each enabled layer
//! 2. Filters out transparent pixels (`None` results)
//! 3. Sorts by priority (lower = higher priority)
//! 4. Displays the topmost non-transparent pixel
//!
//! # Tile-Based Rendering (Text Mode)
//!
//! Text backgrounds use a simple tile lookup with scrolling:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                  Text Background Rendering                      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Screen Position (x, y)                                         │
//! │         │                                                       │
//! │         ▼                                                       │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
//! │  │ Add Scroll   │───▶│ Tilemap      │───▶│ Tile Data    │      │
//! │  │ Offset       │    │ Lookup       │    │ (Character)  │      │
//! │  └──────────────┘    └──────────────┘    └──────────────┘      │
//! │                             │                    │              │
//! │                             ▼                    ▼              │
//! │                      Tile Number,         Palette Index         │
//! │                      Flip Flags,                │               │
//! │                      Palette Bank               ▼               │
//! │                                          ┌──────────────┐       │
//! │                                          │ Palette RAM  │       │
//! │                                          │ Color Lookup │       │
//! │                                          └──────────────┘       │
//! │                                                 │               │
//! │                                                 ▼               │
//! │                                           Final Color           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Affine Transformation
//!
//! Affine backgrounds apply a 2×2 matrix transformation to map screen coordinates
//! to texture coordinates:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                 Affine Background Rendering                     │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Screen Position (x, y)                                         │
//! │         │                                                       │
//! │         ▼                                                       │
//! │  ┌──────────────────────────────────────────────────────┐      │
//! │  │  texture_x = PA × x + PB × y + REF_X                 │      │
//! │  │  texture_y = PC × x + PD × y + REF_Y                 │      │
//! │  │  (8.8 fixed-point arithmetic)                        │      │
//! │  └──────────────────────────────────────────────────────┘      │
//! │         │                                                       │
//! │         ▼                                                       │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
//! │  │ Wraparound/  │───▶│ Tilemap      │───▶│ Tile Data    │      │
//! │  │ Clip Check   │    │ (8-bit)      │    │ (8bpp only)  │      │
//! │  └──────────────┘    └──────────────┘    └──────────────┘      │
//! │                                                 │               │
//! │                                                 ▼               │
//! │                                          ┌──────────────┐       │
//! │                                          │ Palette RAM  │       │
//! │                                          └──────────────┘       │
//! │                                                 │               │
//! │                                                 ▼               │
//! │                                           Final Color           │
//! └─────────────────────────────────────────────────────────────────┘
//!
//! Matrix examples:
//!   Identity (no transform): PA=1.0, PB=0, PC=0, PD=1.0
//!   90° rotation:            PA=0, PB=-1.0, PC=1.0, PD=0
//!   2× zoom:                 PA=0.5, PB=0, PC=0, PD=0.5
//! ```
//!
//! # Transparency
//!
//! Palette index 0 is always transparent for both backgrounds and sprites.
//! Returning `None` from [`Layer::render`] indicates a transparent pixel.

use super::{Color, PixelInfo, memory::Memory, registers::Registers};
use crate::bitwise::Bits;

pub mod layer_0;
pub mod layer_1;
pub mod layer_2;
pub mod layer_3;
pub mod layer_obj;

/// Configuration trait for text-mode (regular) background layers.
///
/// Text backgrounds are the simpler of the two background types. They support:
/// - Integer scrolling (no sub-pixel positioning)
/// - Per-tile horizontal and vertical flipping
/// - 4bpp (16 colors) or 8bpp (256 colors) color modes
/// - Map sizes from 256×256 to 512×512 pixels
///
/// BG0 and BG1 are always text-mode. BG2 and BG3 can be text-mode in video mode 0.
///
/// Each background layer implements this trait to provide access to its specific
/// registers. The shared [`render_text_bg`] function uses this trait to render
/// any text background without code duplication.
pub trait TextBgConfig {
    /// Layer ID (0-3).
    fn layer_id(&self) -> u8;

    /// Scroll offsets (HOFS, VOFS).
    fn get_scroll(&self, reg: &Registers) -> (u16, u16);

    /// Screen size in pixels, e.g., (256, 256) or (512, 512).
    fn get_screen_size(&self, reg: &Registers) -> (usize, usize);

    /// Screen base block index (0-31), each block is 2KB.
    fn get_screen_base_block(&self, reg: &Registers) -> u8;

    /// Character base block index (0-3), each block is 16KB.
    fn get_char_base_block(&self, reg: &Registers) -> u8;

    /// Color mode: `true` for 8bpp (256 colors), `false` for 4bpp (16 colors).
    fn get_color_mode(&self, reg: &Registers) -> bool;

    /// Layer priority (0-3, lower = higher priority).
    fn get_priority(&self, reg: &Registers) -> u8;
}

/// Renders a text-mode background pixel.
///
/// This is the shared implementation for all text backgrounds (BG0-BG3 in modes 0-1).
/// The layer-specific register values are obtained via the [`TextBgConfig`] trait.
///
/// # Algorithm
///
/// 1. Apply scroll offset and wrap to map size
/// 2. Calculate tile coordinates and pixel position within tile
/// 3. Handle multi-screen-block maps (256×256 to 512×512)
/// 4. Read tilemap entry (tile number, flip flags, palette bank)
/// 5. Apply horizontal/vertical flip
/// 6. Read pixel from tile data (4bpp or 8bpp)
/// 7. Look up color in palette RAM
pub fn render_text_bg<T: TextBgConfig>(
    config: &T,
    x: usize,
    y: usize,
    memory: &Memory,
    registers: &Registers,
) -> Option<PixelInfo> {
    let (map_width, map_height) = config.get_screen_size(registers);
    let (hofs, vofs) = config.get_scroll(registers);

    // Mosaic snaps each pixel to the top-left of its block before scrolling.
    let (x, y) = mosaic_snap_bg(config.layer_id(), x, y, registers);

    // Apply scrolling with map size wrapping
    let scroll_x = (x + hofs as usize) % map_width;
    let scroll_y = (y + vofs as usize) % map_height;

    // Tile coordinates (8×8 tiles)
    let tile_x = scroll_x / 8;
    let tile_y = scroll_y / 8;
    let pixel_in_tile_x = scroll_x % 8;
    let pixel_in_tile_y = scroll_y % 8;

    // Screen base block address (each block is 2KB = 0x800)
    let screen_base = config.get_screen_base_block(registers) as usize * 0x800;

    // For maps > 256 pixels, calculate which screen block and local tile position
    let screen_block_x = tile_x / 32;
    let screen_block_y = tile_y / 32;
    let local_tile_x = tile_x % 32;
    let local_tile_y = tile_y % 32;

    // Screen block layout:
    // 256×256: [0]        512×256: [0][1]
    // 256×512: [0]        512×512: [0][1]
    //          [1]                 [2][3]
    let screen_block_offset = match (map_width > 256, map_height > 256) {
        (true, true) => (screen_block_y * 2 + screen_block_x) * 0x800,
        (true, false) => screen_block_x * 0x800,
        (false, true) => screen_block_y * 0x800,
        (false, false) => 0,
    };

    // Tilemap entry: 2 bytes per tile, 32×32 tiles per screen block
    let tilemap_index = local_tile_y * 32 + local_tile_x;
    let tilemap_entry_addr = screen_base + screen_block_offset + tilemap_index * 2;

    if tilemap_entry_addr + 1 >= memory.video_ram.len() {
        return None;
    }

    let tilemap_entry = u16::from_le_bytes([
        memory.video_ram[tilemap_entry_addr],
        memory.video_ram[tilemap_entry_addr + 1],
    ]);

    // Parse tilemap entry
    let tile_number = tilemap_entry.get_bits(0..=9) as usize;
    let horizontal_flip = tilemap_entry.get_bit(10);
    let vertical_flip = tilemap_entry.get_bit(11);
    let palette_bank = tilemap_entry.get_bits(12..=15) as usize;

    // Apply flip to pixel coordinates
    let final_pixel_x = if horizontal_flip {
        7 - pixel_in_tile_x
    } else {
        pixel_in_tile_x
    };
    let final_pixel_y = if vertical_flip {
        7 - pixel_in_tile_y
    } else {
        pixel_in_tile_y
    };

    // Character base block address (each block is 16KB = 0x4000)
    let char_base = config.get_char_base_block(registers) as usize * 0x4000;
    let is_8bpp = config.get_color_mode(registers);

    // Read palette index from tile data
    let palette_index = if is_8bpp {
        // 8bpp: 64 bytes per tile, 1 byte per pixel
        let offset = char_base + tile_number * 64 + final_pixel_y * 8 + final_pixel_x;
        if offset >= memory.video_ram.len() {
            return None;
        }
        memory.video_ram[offset] as usize
    } else {
        // 4bpp: 32 bytes per tile, 4 bits per pixel (2 pixels per byte)
        let offset = char_base + tile_number * 32 + final_pixel_y * 4 + final_pixel_x / 2;
        if offset >= memory.video_ram.len() {
            return None;
        }
        let byte = memory.video_ram[offset];
        if final_pixel_x % 2 == 0 {
            (byte & 0x0F) as usize
        } else {
            (byte >> 4) as usize
        }
    };

    // Palette index 0 is transparent
    if palette_index == 0 {
        return None;
    }

    // Calculate final palette index
    let final_palette_index = if is_8bpp {
        palette_index
    } else {
        palette_bank * 16 + palette_index
    };

    // Read color from BG palette RAM (2 bytes per color)
    let color = Color::from_palette_color(u16::from_le_bytes([
        memory.bg_palette_ram[final_palette_index * 2],
        memory.bg_palette_ram[final_palette_index * 2 + 1],
    ]));

    Some(PixelInfo {
        color,
        priority: config.get_priority(registers),
        layer: config.layer_id(),
        semi_transparent: false,
    })
}

/// Configuration trait for affine-mode (rotation/scaling) background layers.
///
/// Affine backgrounds support hardware transformation via a 2×2 matrix, enabling:
/// - Rotation (any angle)
/// - Scaling (zoom in/out, can be non-uniform)
/// - Shearing/skewing
/// - Any combination of the above
///
/// Trade-offs compared to text backgrounds:
/// - No per-tile flipping (must be done via the matrix)
/// - Always 8bpp (no 4bpp option)
/// - Simpler tilemap format (8-bit entries vs 16-bit)
/// - Square maps only (but up to 1024×1024)
///
/// Only BG2 and BG3 support affine mode (in video modes 1 and 2).
///
/// This trait provides access to the affine-specific registers (transformation
/// matrix and reference point).
pub trait AffineBgConfig {
    /// Layer ID (2 or 3).
    fn layer_id(&self) -> u8;

    /// Affine matrix parameters (PA, PB, PC, PD) as 8.8 fixed-point.
    fn get_affine_params(&self, reg: &Registers) -> (i16, i16, i16, i16);

    /// Reference point (X, Y) as 20.8 fixed-point.
    fn get_reference_point(&self, reg: &Registers) -> (i32, i32);

    /// Background control register value.
    fn get_bg_control(&self, reg: &Registers) -> u16;
}

/// Sign-extend a 28-bit affine reference point (BGxX/BGxY) held in a u32 to a full i32, where bit 27 is the sign and bits 28-31 are ignored.
pub const fn sign_extend_28(value: u32) -> i32 {
    ((value << 4).cast_signed()) >> 4
}

/// Snap a background pixel to the top-left of its mosaic block when the layer
/// has mosaic enabled, otherwise return the coordinate unchanged.
fn mosaic_snap_bg(layer_id: u8, x: usize, y: usize, registers: &Registers) -> (usize, usize) {
    if !registers.bg_mosaic_enabled(layer_id) {
        return (x, y);
    }
    let (mh, mv) = registers.bg_mosaic_size();
    (x - x % mh, y - y % mv)
}

/// Renders an affine-mode background pixel.
///
/// This is the shared implementation for affine backgrounds (BG2/BG3 in mode 2).
///
/// # Affine Transformation
///
/// ```text
/// texture_x = PA × screen_x + PB × screen_y + REF_X
/// texture_y = PC × screen_x + PD × screen_y + REF_Y
/// ```
///
/// Key differences from text backgrounds:
/// - Tilemap entries are 8-bit (tile index only, no flip/palette)
/// - Always 8bpp color mode
/// - Map sizes: 128×128, 256×256, 512×512, or 1024×1024
/// - Optional wraparound at map edges
pub fn render_affine_bg<T: AffineBgConfig>(
    config: &T,
    screen_x: usize,
    _screen_y: usize,
    memory: &Memory,
    registers: &Registers,
) -> Option<PixelInfo> {
    // pb and pd (dmx, dmy) are folded into the reference point once per scanline
    // by advance_affine_internals, so only pa and pc are applied per pixel here.
    let (pa, _pb, pc, _pd) = config.get_affine_params(registers);
    let (ref_x, ref_y) = config.get_reference_point(registers);

    // Horizontal mosaic snaps the sampled column. Vertical mosaic for affine is
    // not modeled here since it interacts with the per-scanline reference.
    let screen_x = if registers.bg_mosaic_enabled(config.layer_id()) {
        let (mh, _) = registers.bg_mosaic_size();
        screen_x - screen_x % mh
    } else {
        screen_x
    };

    // Apply affine transformation (8.8 fixed-point math)
    // Screen coords (0-239) always fit in i32
    #[allow(clippy::cast_possible_wrap)]
    let sx = screen_x as i32;
    let texture_x = i32::from(pa) * sx + ref_x;
    let texture_y = i32::from(pc) * sx + ref_y;

    // Convert from 8.8 fixed-point to integer
    let tex_x = texture_x >> 8;
    let tex_y = texture_y >> 8;

    // Parse control register
    let bgcnt = config.get_bg_control(registers);
    let screen_size = bgcnt.get_bits(14..=15);
    let char_base = bgcnt.get_bits(2..=3) as usize;
    let screen_base = bgcnt.get_bits(8..=12) as usize;
    let wraparound = bgcnt.get_bit(13);
    let priority = bgcnt.get_bits(0..=1) as u8;

    // Affine map sizes (square only)
    let map_size: i32 = match screen_size {
        0 => 128,
        1 => 256,
        2 => 512,
        3 => 1024,
        _ => unreachable!(),
    };

    // Handle wraparound or clipping
    let final_x = if wraparound {
        tex_x.rem_euclid(map_size)
    } else if tex_x < 0 || tex_x >= map_size {
        return None;
    } else {
        tex_x
    };

    let final_y = if wraparound {
        tex_y.rem_euclid(map_size)
    } else if tex_y < 0 || tex_y >= map_size {
        return None;
    } else {
        tex_y
    };

    // Tile and pixel coordinates (values are positive after wraparound check)
    #[allow(clippy::cast_sign_loss)] // Values guaranteed positive by wraparound logic above
    let tile_x = (final_x / 8) as usize;
    #[allow(clippy::cast_sign_loss)]
    let tile_y = (final_y / 8) as usize;
    #[allow(clippy::cast_sign_loss)]
    let pixel_x = (final_x % 8) as usize;
    #[allow(clippy::cast_sign_loss)]
    let pixel_y = (final_y % 8) as usize;

    #[allow(clippy::cast_sign_loss)] // map_size is always positive
    let tiles_per_row = (map_size / 8) as usize;

    // Affine tilemap: flat layout, 1 byte per entry
    let tilemap_offset = screen_base * 0x800; // 2KB per screen block
    let tile_index_addr = tilemap_offset + tile_y * tiles_per_row + tile_x;

    if tile_index_addr >= memory.video_ram.len() {
        return None;
    }

    let tile_index = memory.video_ram[tile_index_addr] as usize;

    // Affine tiles are always 8bpp (64 bytes per tile)
    let char_base_offset = char_base * 0x4000;
    let pixel_offset = char_base_offset + tile_index * 64 + pixel_y * 8 + pixel_x;

    if pixel_offset >= memory.video_ram.len() {
        return None;
    }

    let palette_index = memory.video_ram[pixel_offset] as usize;

    if palette_index == 0 {
        return None;
    }

    // Read color from palette RAM
    let palette_addr = palette_index * 2;
    if palette_addr + 1 >= memory.bg_palette_ram.len() {
        return None;
    }

    let color = Color::from_palette_color(u16::from_le_bytes([
        memory.bg_palette_ram[palette_addr],
        memory.bg_palette_ram[palette_addr + 1],
    ]));

    Some(PixelInfo {
        color,
        priority,
        layer: config.layer_id(),
        semi_transparent: false,
    })
}

/// Trait for renderable display layers.
///
/// Each layer type implements this trait to participate in the PPU's
/// compositing pipeline. The LCD controller calls [`render`](Layer::render)
/// for each enabled layer at each pixel position.
#[allow(dead_code)]
pub trait Layer {
    /// Returns the layer ID (0-3 for BG0-BG3, 4 for OBJ).
    fn layer_id(&self) -> u8;

    /// Renders a single pixel at the given screen coordinates.
    ///
    /// # Arguments
    /// * `x` - Screen X coordinate (0-239)
    /// * `y` - Screen Y coordinate (0-159)
    /// * `memory` - Reference to VRAM and palette RAM
    /// * `registers` - LCD control registers
    ///
    /// # Returns
    /// * `Some(PixelInfo)` - The rendered color and priority
    /// * `None` - Pixel is transparent (palette index 0 or out of bounds)
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo>;
}

#[cfg(test)]
mod tests {
    use super::{Registers, mosaic_snap_bg, sign_extend_28};

    #[test]
    fn mosaic_snap_is_identity_when_disabled() {
        let reg = Registers::default();
        assert_eq!(mosaic_snap_bg(0, 5, 7, &reg), (5, 7));
    }

    #[test]
    fn mosaic_snap_quantizes_to_block_origin() {
        let mut reg = Registers::default();
        // BG mosaic H size 2 (bits 0-3 = 1), V size 3 (bits 4-7 = 2).
        reg.mosaic = 0x0021;
        // Enable mosaic on BG0 (BG0CNT bit 6).
        reg.bg0cnt = 1 << 6;

        assert_eq!(mosaic_snap_bg(0, 5, 7, &reg), (4, 6));
        // A background without the mosaic bit is unaffected.
        assert_eq!(mosaic_snap_bg(1, 5, 7, &reg), (5, 7));
    }

    #[test]
    fn sign_extend_28_handles_sign_and_upper_bits() {
        assert_eq!(sign_extend_28(0), 0);
        assert_eq!(sign_extend_28(1), 1);
        // 28-bit -1 (0x0FFFFFFF) must become full i32 -1.
        assert_eq!(sign_extend_28(0x0FFF_FFFF), -1);
        // Most negative 28-bit value.
        assert_eq!(sign_extend_28(0x0800_0000), -(1 << 27));
        // Largest positive 28-bit value.
        assert_eq!(sign_extend_28(0x07FF_FFFF), (1 << 27) - 1);
        // Bits 28-31 are ignored.
        assert_eq!(sign_extend_28(0xF000_0001), 1);
    }
}
