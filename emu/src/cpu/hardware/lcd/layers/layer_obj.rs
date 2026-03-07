#![allow(clippy::cast_possible_truncation)]

//! Sprite (OBJ) layer rendering.
//!
//! The GBA supports 128 hardware sprites (objects) that can be positioned anywhere
//! on screen with per-pixel transparency, rotation, scaling, and flipping.
//!
//! # Object Attribute Memory (OAM)
//!
//! Sprites are defined in OAM (`0x0700_0000`, 1KB), with each sprite using 8 bytes
//! split into three 16-bit attributes:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  OAM Entry (8 bytes per sprite, interleaved with rot/scale)    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Attribute 0 (2 bytes):                                        │
//! │    Bits 0-7:   Y coordinate (0-255, wraps)                     │
//! │    Bits 8-9:   Object mode (Normal/Affine/Disabled/AffineDouble)│
//! │    Bits 10-11: GFX mode (Normal/Alpha/ObjWindow/Prohibited)    │
//! │    Bit 12:     Mosaic enable                                   │
//! │    Bit 13:     Color mode (0=4bpp/16 colors, 1=8bpp/256 colors)│
//! │    Bits 14-15: Shape (Square/Horizontal/Vertical)              │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Attribute 1 (2 bytes):                                        │
//! │    Bits 0-8:   X coordinate (0-511, wraps at 512)              │
//! │    Bits 9-13:  Affine parameter index OR H-flip/V-flip         │
//! │    Bits 14-15: Size (combined with shape for dimensions)       │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Attribute 2 (2 bytes):                                        │
//! │    Bits 0-9:   Tile number (character name)                    │
//! │    Bits 10-11: Priority (0=highest, 3=lowest)                  │
//! │    Bits 12-15: Palette bank (4bpp mode only)                   │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Sprite Sizes
//!
//! The combination of shape and size determines sprite dimensions:
//!
//! | Shape      | Size 0 | Size 1 | Size 2 | Size 3 |
//! |------------|--------|--------|--------|--------|
//! | Square     | 8x8    | 16x16  | 32x32  | 64x64  |
//! | Horizontal | 16x8   | 32x8   | 32x16  | 64x32  |
//! | Vertical   | 8x16   | 8x32   | 16x32  | 32x64  |
//!
//! # Tile Data Layout
//!
//! Sprite tiles are stored in VRAM starting at `0x0601_0000` (OBJ character area).
//! Two mapping modes control how multi-tile sprites index their tiles:
//!
//! ## 1D Mapping (DISPCNT bit 6 = 1)
//! Tiles are stored consecutively in memory. For a 16x16 sprite (2x2 tiles):
//! ```text
//! Tile indices: [base+0] [base+1]
//!               [base+2] [base+3]
//! ```
//!
//! ## 2D Mapping (DISPCNT bit 6 = 0)
//! Tiles are arranged in a 32-tile-wide virtual grid:
//! ```text
//! Tile indices: [base+0]  [base+1]
//!               [base+32] [base+33]
//! ```
//!
//! # Affine Sprites
//!
//! Sprites can be rotated and scaled using affine transformation parameters.
//! 32 rotation/scaling parameter sets are stored interleaved in OAM (using the
//! unused bytes between sprite attributes).
//!
//! Each parameter set contains PA, PB, PC, PD (8.8 fixed-point) for the
//! transformation matrix. See [`layer_2`](super::layer_2) for matrix math.
//!
//! `AffineDouble` mode doubles the sprite's screen area to prevent clipping
//! during rotation (a 32x32 sprite becomes 64x64 on screen, but still uses
//! 32x32 tile data).
//!
//! # Rendering Pipeline
//!
//! Unlike backgrounds which render pixel-by-pixel, sprites use scanline rendering:
//!
//! 1. At the start of each scanline, [`handle_enter_vdraw`](LayerObj::handle_enter_vdraw)
//!    parses all 128 OAM entries
//! 2. For each sprite intersecting the current scanline, pixels are rendered to
//!    a scanline buffer
//! 3. During compositing, [`render`](Layer::render) simply returns the pre-computed
//!    pixel from the buffer
//!
//! This approach handles sprite priority correctly (lower OAM index = higher priority
//! when priorities are equal).
use crate::cpu::hardware::lcd;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::object_attributes;
use crate::cpu::hardware::lcd::point::Point;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::Color;
use crate::cpu::hardware::lcd::{PixelInfo, LCD_WIDTH, WORLD_HEIGHT};

use super::Layer;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct LayerObj {
    #[serde_as(as = "[_; 128]")]
    obj_attributes_arr: [object_attributes::ObjAttributes; 128],

    #[serde_as(as = "[_; 32]")]
    rotation_scaling_params: [object_attributes::RotationScaling; 32],

    #[serde_as(as = "[_; 240]")]
    sprite_pixels_scanline: [Option<PixelInfo>; LCD_WIDTH],

    /// Mask for WINOBJ (Object Window), true if pixel is covered by a window-type sprite.
    /// Used for the flashlight effect in Pokemon caves for example.
    /// Skipped in serialization - recalculated every scanline anyway.
    #[serde(skip, default = "default_winobj_mask")]
    winobj_mask: [bool; LCD_WIDTH],
}

/// Default value for `winobj_mask` when deserializing.
const fn default_winobj_mask() -> [bool; LCD_WIDTH] {
    [false; LCD_WIDTH]
}

impl Default for LayerObj {
    fn default() -> Self {
        Self {
            obj_attributes_arr: [object_attributes::ObjAttributes::default(); 128],
            rotation_scaling_params: [object_attributes::RotationScaling::default(); 32],
            sprite_pixels_scanline: [None; LCD_WIDTH],
            winobj_mask: [false; LCD_WIDTH],
        }
    }
}

impl Layer for LayerObj {
    fn layer_id(&self) -> u8 {
        4
    }

    fn render(
        &self,
        x: usize,
        _y: usize,
        _memory: &Memory,
        _registers: &Registers,
    ) -> Option<PixelInfo> {
        self.sprite_pixels_scanline[x]
    }
}

impl LayerObj {
    const fn read_color_from_obj_palette(color_idx: usize, obj_palette_ram: &[u8]) -> Color {
        // Each color is 2 bytes, so multiply index by 2 to get byte offset
        let byte_offset = color_idx * 2;
        let low_byte = obj_palette_ram[byte_offset] as u16;
        let high_byte = obj_palette_ram[byte_offset + 1] as u16;

        Color::from_palette_color((high_byte << 8) | low_byte)
    }

    fn get_texture_space_point(
        &self,
        sprite_size: Point<u16>,
        pixel_screen_sprite_origin: Point<u16>,
        transformation_kind: object_attributes::TransformationKind,
        obj_mode: object_attributes::ObjMode,
    ) -> Point<f64> {
        if let object_attributes::TransformationKind::RotationScaling {
            rotation_scaling_parameter,
        } = transformation_kind
        {
            // We have to use f64 for translating/rot/scale because we might have negative values when using the pixel
            // in the carthesian plane having the origin as the center of the sprite.
            // We could use i16 as well but then we would still need to use f64 to apply the transformation.

            // RotScale matrix
            let rotscale_params = self.rotation_scaling_params[rotation_scaling_parameter as usize];
            let sprite_size = sprite_size.map(f64::from);

            // This is the pixel coordinate in the screen space using the sprite center as origin of the reference system
            // This is needed because the rotscale is applied taking the center of the sprite as the origin of the rotation
            // If the sprite is in AffineDouble mode then it has double dimensions and the center is at +sprite_width/+sprite_height insted of
            // just half ot that.
            let pixel_screen_sprite_center = pixel_screen_sprite_origin.map(f64::from)
                - match obj_mode {
                    object_attributes::ObjMode::Affine => sprite_size / 2.0,
                    object_attributes::ObjMode::AffineDouble => sprite_size,
                    _ => unreachable!(),
                };

            // Applying transformation.
            // The result will be a pixel in the texture space which still has the center of the sprite as the origin of the reference system
            let pixel_texture_sprite_center = pixel_screen_sprite_center * rotscale_params;

            // Moving back the reference system to the origin of the sprite (top-left corner).
            pixel_texture_sprite_center + sprite_size / 2.0
        } else if let object_attributes::TransformationKind::Flip {
            horizontal_flip,
            vertical_flip,
        } = transformation_kind
        {
            // Handle horizontal and vertical flipping
            let mut pixel_x = f64::from(pixel_screen_sprite_origin.x);
            let mut pixel_y = f64::from(pixel_screen_sprite_origin.y);

            if horizontal_flip {
                pixel_x = f64::from(sprite_size.x) - 1.0 - pixel_x;
            }

            if vertical_flip {
                pixel_y = f64::from(sprite_size.y) - 1.0 - pixel_y;
            }

            Point::new(pixel_x, pixel_y)
        } else {
            // No transformation
            pixel_screen_sprite_origin.map(f64::from)
        }
    }

    #[allow(clippy::too_many_lines)]
    fn process_sprites_scanline(&mut self, registers: &Registers, memory: &Memory) {
        self.sprite_pixels_scanline = [None; LCD_WIDTH];
        self.winobj_mask = [false; LCD_WIDTH];
        let y = registers.vcount;

        for obj in self.obj_attributes_arr {
            if matches!(
                obj.attribute0.obj_mode,
                object_attributes::ObjMode::Disabled
            ) || matches!(
                obj.attribute0.gfx_mode,
                object_attributes::GfxMode::ObjectWindow
            ) {
                continue;
            }

            let (sprite_width, sprite_height) =
                match (obj.attribute0.obj_shape, obj.attribute1.obj_size) {
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size0) => {
                        (8_u8, 8_u8)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size0,
                    ) => (16, 8),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size0) => {
                        (8, 16)
                    }
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size1) => {
                        (16, 16)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size1,
                    ) => (32, 8),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size1) => {
                        (8, 32)
                    }
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size2) => {
                        (32, 32)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size2,
                    ) => (32, 16),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size2) => {
                        (16, 32)
                    }
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size3) => {
                        (64, 64)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size3,
                    ) => (64, 32),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size3) => {
                        (32, 64)
                    }
                };

            // We can represent the size of the sprite using a point.
            let sprite_size = Point::new(u16::from(sprite_width), u16::from(sprite_height));

            // Sprite size using tiles as dimensions
            let sprite_size_tile = sprite_size / 8;

            let sprite_position = Point::new(
                obj.attribute1.x_coordinate,
                u16::from(obj.attribute0.y_coordinate),
            );

            let is_affine_double = matches!(
                obj.attribute0.obj_mode,
                object_attributes::ObjMode::AffineDouble
            );

            // Sprite size in screen space (takes into account double size sprites)
            let sprite_screen_size = sprite_size * if is_affine_double { 2 } else { 1 };

            // Check if current scanline intersects this sprite's Y range
            // Sprites use a 256-pixel coordinate system (WORLD_HEIGHT)
            let sprite_y_start = sprite_position.y;
            let sprite_y_end = (sprite_y_start + sprite_screen_size.y) % WORLD_HEIGHT;

            // Check if scanline y is within sprite's Y range (handling wrapping)
            let scanline_in_sprite = if sprite_y_end > sprite_y_start {
                // Normal case: no wrapping
                y >= sprite_y_start && y < sprite_y_end
            } else {
                // Wrapping case: sprite crosses bottom of screen
                y >= sprite_y_start || y < sprite_y_end
            };

            if !scanline_in_sprite {
                continue;
            }

            for idx in 0..sprite_screen_size.x {
                // This is the pixel coordinate in the screen space using the sprite origin (top-left corner) as origin of the reference system
                let pixel_screen_sprite_origin =
                    Point::new(idx, (y + WORLD_HEIGHT - sprite_position.y) % WORLD_HEIGHT);

                // We check that the coordinates in the screen space are inside the sprite
                // Taking care of the fact that if the sprite in AffineDouble it has double the dimensions
                if pixel_screen_sprite_origin.x > sprite_screen_size.x
                    || pixel_screen_sprite_origin.y > sprite_screen_size.y
                {
                    continue;
                }

                // We apply the transformation.
                // The result is a pixel in the texture space with the origin of the sprite (top-left corner) as the origin of the reference system
                let pixel_texture_sprite_origin = self.get_texture_space_point(
                    sprite_size,
                    pixel_screen_sprite_origin,
                    obj.attribute1.transformation_kind,
                    obj.attribute0.obj_mode,
                );

                // We check that the pixel is inside the sprite
                if pixel_texture_sprite_origin.x < 0.0
                    || pixel_texture_sprite_origin.y < 0.0
                    || pixel_texture_sprite_origin.x >= f64::from(sprite_size.x)
                    || pixel_texture_sprite_origin.y >= f64::from(sprite_size.y)
                {
                    continue;
                }

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                // Bounds checked above
                let pixel_texture_sprite_origin = pixel_texture_sprite_origin.map(|el| el as u16);

                // Pixel in texture space using tiles as dimensions
                let pixel_texture_tile = pixel_texture_sprite_origin / 8;

                // Offset of the pixel inside the tile
                let y_tile_idx = pixel_texture_sprite_origin.y % 8;
                let x_tile_idx = pixel_texture_sprite_origin.x % 8;

                let obj_character_vram_mapping = registers.get_obj_character_vram_mapping();

                // Calculate x_screen - add sprite position to pixel index within sprite
                // Sprites use a 512-pixel wide virtual coordinate space (9-bit X)
                // Wrapping occurs at 512, so sprites near the right edge can wrap to appear on left
                let x_screen = (sprite_position.x.wrapping_add(idx)) % 512;

                let color_offset = match obj.attribute0.color_mode {
                    object_attributes::ColorMode::Palette8bpp => {
                        // For 8bpp sprites, tile numbering uses 32-byte s-tile offsets
                        // Each 8bpp tile is 64 bytes (d-tile) = 2 s-tiles, so we multiply by 2
                        let base_tile = obj.attribute2.tile_number;
                        let tile_offset = match obj_character_vram_mapping {
                            lcd::ObjMappingKind::OneDimensional => {
                                // In 1D mode, tiles are consecutive in memory
                                // Multiply by 2 because each 8bpp tile occupies 2 tile slots
                                pixel_texture_tile.y * sprite_size_tile.x * 2
                                    + pixel_texture_tile.x * 2
                            }
                            lcd::ObjMappingKind::TwoDimensional => {
                                // In 2D mode, tiles are in a 32-tile-wide grid
                                // Multiply by 2 because each 8bpp tile occupies 2 tile slots
                                pixel_texture_tile.y * 32 + pixel_texture_tile.x * 2
                            }
                        };
                        let tile_number = base_tile + tile_offset;

                        // A tile is 8x8 mini-bitmap.
                        // A tile is 64bytes long in 8bpp (8x8 pixels × 1 byte/pixel).
                        // Address calculation: 0x10000 + (tile << 5) + (tile_y << 3) + tile_x
                        // Note: tile << 5 means tile * 32, but tiles are actually stored sequentially
                        // For 8bpp, each tile is 64 bytes, so tiles at even indices use first 32 bytes,
                        // and we need to account for the full 64-byte tile size
                        let tile_data_offset = (tile_number << 5) + (y_tile_idx << 3) + x_tile_idx;

                        // color_idx
                        memory.video_ram[0x10000 + tile_data_offset as usize]
                    }
                    object_attributes::ColorMode::Palette4bpp => {
                        let tile_offset = match obj_character_vram_mapping {
                            lcd::ObjMappingKind::OneDimensional => {
                                // In this case memory is seen as a single array.
                                // tile_number is the offset of the first tile in memory.
                                // then we access [y][x] by doing y*number_cols + x, as if we were to access an array as a matrix
                                pixel_texture_tile.y * sprite_size_tile.x + pixel_texture_tile.x
                            }
                            lcd::ObjMappingKind::TwoDimensional => {
                                // A charblock is 32x32 tiles
                                pixel_texture_tile.y * 32 + pixel_texture_tile.x
                            }
                        };
                        let tile_number = obj.attribute2.tile_number + tile_offset;

                        // A tile is 32bytes long in 4bpp.
                        // Each byte contains 2 pixels (4 bits each)
                        // Address calculation: 0x10000 + (tile << 5) + (tile_y << 2) + (tile_x >> 1)
                        let tile_data_offset =
                            (tile_number << 5) + (y_tile_idx << 2) + (x_tile_idx >> 1);

                        // Read the byte containing the pixel data
                        let pixel_byte = memory.video_ram[0x10000 + tile_data_offset as usize];

                        // Extract the correct nibble based on x position
                        // Odd x positions use high nibble, even use low nibble
                        let palette_offset_low = if (x_tile_idx & 1) != 0 {
                            pixel_byte >> 4 // High nibble for odd x
                        } else {
                            pixel_byte & 0x0F // Low nibble for even x
                        };

                        // For 4bpp sprites, palette index 0 within the bank is transparent
                        // Check this BEFORE combining with palette bank
                        if palette_offset_low == 0 {
                            continue;
                        }

                        // Combine with palette bank number to get final palette index
                        (obj.attribute2.palette_number << 4) | palette_offset_low
                    }
                };

                if x_screen >= self.sprite_pixels_scanline.len() as u16 {
                    continue;
                }

                // Palette index 0 is transparent for 8bpp sprites
                // (4bpp transparency is handled above before combining with palette bank)
                if matches!(
                    obj.attribute0.color_mode,
                    object_attributes::ColorMode::Palette8bpp
                ) && color_offset == 0
                {
                    continue;
                }

                let get_pixel_info_closure = || PixelInfo {
                    color: Self::read_color_from_obj_palette(
                        color_offset as usize,
                        memory.obj_palette_ram.as_slice(),
                    ),
                    priority: obj.attribute2.priority,
                    layer: 4,
                };

                self.sprite_pixels_scanline[x_screen as usize] =
                    Some(self.sprite_pixels_scanline[x_screen as usize].map_or_else(
                        get_pixel_info_closure,
                        |current_pixel_info| {
                            // For OBJ priority: lower priority number = higher priority (drawn on top)
                            // For equal priority, lower OAM index wins (processed first, should NOT be replaced)
                            // So only replace if new sprite has STRICTLY lower priority number
                            if current_pixel_info.priority > obj.attribute2.priority {
                                get_pixel_info_closure()
                            } else {
                                current_pixel_info
                            }
                        },
                    ));
            }
        }
    }

    /// Process sprites with `GfxMode::ObjectWindow` to build the WINOBJ mask.
    ///
    /// Sprites with this mode don't render visually but instead define the Object Window region.
    /// Non-transparent pixels of these sprites mark areas where WINOBJ layer enables apply.
    /// This is used for effects like the flashlight in Pokemon Emerald caves.
    #[allow(clippy::too_many_lines)]
    fn process_winobj_sprites_scanline(&mut self, registers: &Registers, memory: &Memory) {
        let y = registers.vcount;

        for obj in &self.obj_attributes_arr {
            // Only process ObjectWindow sprites that are not disabled
            if matches!(
                obj.attribute0.obj_mode,
                object_attributes::ObjMode::Disabled
            ) || !matches!(
                obj.attribute0.gfx_mode,
                object_attributes::GfxMode::ObjectWindow
            ) {
                continue;
            }

            let (sprite_width, sprite_height) =
                match (obj.attribute0.obj_shape, obj.attribute1.obj_size) {
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size0) => {
                        (8_u8, 8_u8)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size0,
                    ) => (16, 8),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size0) => {
                        (8, 16)
                    }
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size1) => {
                        (16, 16)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size1,
                    ) => (32, 8),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size1) => {
                        (8, 32)
                    }
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size2) => {
                        (32, 32)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size2,
                    ) => (32, 16),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size2) => {
                        (16, 32)
                    }
                    (object_attributes::ObjShape::Square, object_attributes::ObjSize::Size3) => {
                        (64, 64)
                    }
                    (
                        object_attributes::ObjShape::Horizontal,
                        object_attributes::ObjSize::Size3,
                    ) => (64, 32),
                    (object_attributes::ObjShape::Vertical, object_attributes::ObjSize::Size3) => {
                        (32, 64)
                    }
                };

            let sprite_size = Point::new(u16::from(sprite_width), u16::from(sprite_height));
            let sprite_size_tile = sprite_size / 8;

            let sprite_position = Point::new(
                obj.attribute1.x_coordinate,
                u16::from(obj.attribute0.y_coordinate),
            );

            let is_affine_double = matches!(
                obj.attribute0.obj_mode,
                object_attributes::ObjMode::AffineDouble
            );

            let sprite_screen_size = sprite_size * if is_affine_double { 2 } else { 1 };

            let sprite_y_start = sprite_position.y;
            let sprite_y_end = (sprite_y_start + sprite_screen_size.y) % WORLD_HEIGHT;

            let scanline_in_sprite = if sprite_y_end > sprite_y_start {
                y >= sprite_y_start && y < sprite_y_end
            } else {
                y >= sprite_y_start || y < sprite_y_end
            };

            if !scanline_in_sprite {
                continue;
            }

            for idx in 0..sprite_screen_size.x {
                let pixel_screen_sprite_origin =
                    Point::new(idx, (y + WORLD_HEIGHT - sprite_position.y) % WORLD_HEIGHT);

                if pixel_screen_sprite_origin.x > sprite_screen_size.x
                    || pixel_screen_sprite_origin.y > sprite_screen_size.y
                {
                    continue;
                }

                let pixel_texture_sprite_origin = self.get_texture_space_point(
                    sprite_size,
                    pixel_screen_sprite_origin,
                    obj.attribute1.transformation_kind,
                    obj.attribute0.obj_mode,
                );

                if pixel_texture_sprite_origin.x < 0.0
                    || pixel_texture_sprite_origin.y < 0.0
                    || pixel_texture_sprite_origin.x >= f64::from(sprite_size.x)
                    || pixel_texture_sprite_origin.y >= f64::from(sprite_size.y)
                {
                    continue;
                }

                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let pixel_texture_sprite_origin = pixel_texture_sprite_origin.map(|el| el as u16);

                let pixel_texture_tile = pixel_texture_sprite_origin / 8;
                let y_tile_idx = pixel_texture_sprite_origin.y % 8;
                let x_tile_idx = pixel_texture_sprite_origin.x % 8;

                let obj_character_vram_mapping = registers.get_obj_character_vram_mapping();
                let x_screen = (sprite_position.x.wrapping_add(idx)) % 512;

                // Check if pixel is non-transparent (same logic as normal sprites)
                let is_opaque = match obj.attribute0.color_mode {
                    object_attributes::ColorMode::Palette8bpp => {
                        let base_tile = obj.attribute2.tile_number;
                        let tile_offset = match obj_character_vram_mapping {
                            lcd::ObjMappingKind::OneDimensional => {
                                pixel_texture_tile.y * sprite_size_tile.x * 2
                                    + pixel_texture_tile.x * 2
                            }
                            lcd::ObjMappingKind::TwoDimensional => {
                                pixel_texture_tile.y * 32 + pixel_texture_tile.x * 2
                            }
                        };
                        let tile_number = base_tile + tile_offset;
                        let tile_data_offset = (tile_number << 5) + (y_tile_idx << 3) + x_tile_idx;
                        let color_idx = memory.video_ram[0x10000 + tile_data_offset as usize];
                        color_idx != 0
                    }
                    object_attributes::ColorMode::Palette4bpp => {
                        let tile_offset = match obj_character_vram_mapping {
                            lcd::ObjMappingKind::OneDimensional => {
                                pixel_texture_tile.y * sprite_size_tile.x + pixel_texture_tile.x
                            }
                            lcd::ObjMappingKind::TwoDimensional => {
                                pixel_texture_tile.y * 32 + pixel_texture_tile.x
                            }
                        };
                        let tile_number = obj.attribute2.tile_number + tile_offset;
                        let tile_data_offset =
                            (tile_number << 5) + (y_tile_idx << 2) + (x_tile_idx >> 1);
                        let pixel_byte = memory.video_ram[0x10000 + tile_data_offset as usize];
                        let palette_offset_low = if (x_tile_idx & 1) != 0 {
                            pixel_byte >> 4
                        } else {
                            pixel_byte & 0x0F
                        };
                        palette_offset_low != 0
                    }
                };

                if is_opaque && x_screen < LCD_WIDTH as u16 {
                    self.winobj_mask[x_screen as usize] = true;
                }
            }
        }
    }

    /// Check if a pixel is covered by the Object Window.
    pub const fn is_in_winobj(&self, x: u8) -> bool {
        self.winobj_mask[x as usize]
    }

    pub fn handle_enter_vdraw(&mut self, memory: &Memory, registers: &Registers) {
        (self.obj_attributes_arr, self.rotation_scaling_params) =
            object_attributes::get_attributes(memory.obj_attributes.as_slice());

        self.process_sprites_scanline(registers, memory);
        self.process_winobj_sprites_scanline(registers, memory);
    }
}
