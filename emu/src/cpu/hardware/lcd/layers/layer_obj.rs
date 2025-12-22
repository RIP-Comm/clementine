use crate::cpu::hardware::lcd;
use crate::cpu::hardware::lcd::Color;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::object_attributes;
use crate::cpu::hardware::lcd::point::Point;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::{LCD_WIDTH, PixelInfo, WORLD_HEIGHT};

use super::Layer;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Serialize, Deserialize)]
#[allow(clippy::unsafe_derive_deserialize)]
pub struct LayerObj {
    #[serde_as(as = "[_; 128]")]
    obj_attributes_arr: [object_attributes::ObjAttributes; 128],

    #[serde_as(as = "[_; 32]")]
    rotation_scaling_params: [object_attributes::RotationScaling; 32],

    #[serde_as(as = "[_; 240]")]
    sprite_pixels_scanline: [Option<PixelInfo>; LCD_WIDTH],
}

impl Default for LayerObj {
    fn default() -> Self {
        Self {
            obj_attributes_arr: [object_attributes::ObjAttributes::default(); 128],
            rotation_scaling_params: [object_attributes::RotationScaling::default(); 32],
            sprite_pixels_scanline: [None; LCD_WIDTH],
        }
    }
}

impl Layer for LayerObj {
    #[allow(unused_variables)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
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
            let sprite_size = sprite_size.map(|el| el as f64);

            // This is the pixel coordinate in the screen space using the sprite center as origin of the reference system
            // This is needed because the rotscale is applied taking the center of the sprite as the origin of the rotation
            // If the sprite is in AffineDouble mode then it has double dimensions and the center is at +sprite_width/+sprite_height insted of
            // just half ot that.
            let pixel_screen_sprite_center = pixel_screen_sprite_origin.map(|el| el as f64)
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
            let mut pixel_x = pixel_screen_sprite_origin.x as f64;
            let mut pixel_y = pixel_screen_sprite_origin.y as f64;

            if horizontal_flip {
                pixel_x = sprite_size.x as f64 - 1.0 - pixel_x;
            }

            if vertical_flip {
                pixel_y = sprite_size.y as f64 - 1.0 - pixel_y;
            }

            Point::new(pixel_x, pixel_y)
        } else {
            // No transformation
            pixel_screen_sprite_origin.map(|el| el as f64)
        }
    }

    #[allow(clippy::too_many_lines)]
    fn process_sprites_scanline(&mut self, registers: &Registers, memory: &Memory) {
        self.sprite_pixels_scanline = [None; LCD_WIDTH];
        let y = registers.vcount;

        let mut sprites_on_scanline = 0;
        let mut pixels_rendered = 0;

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
            let sprite_size = Point::new(sprite_width as u16, sprite_height as u16);

            // Sprite size using tiles as dimensions
            let sprite_size_tile = sprite_size / 8;

            let sprite_position = Point::new(
                obj.attribute1.x_coordinate,
                obj.attribute0.y_coordinate as u16,
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

            // Debug: Log first sprite check on scanline 30
            #[allow(clippy::items_after_statements)]
            static mut DEBUG_LOGGED: bool = false;
            unsafe {
                if y == 30 && !DEBUG_LOGGED {
                    logger::log(format!(
                        "Scanline {}: Checking sprite @ Y={}, size={}, Y_end={}, vcount type check: y={} (type: u16)",
                        y, sprite_y_start, sprite_screen_size.y, sprite_y_end, y
                    ));
                    DEBUG_LOGGED = true;
                }
            }

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

            sprites_on_scanline += 1;

            // Disabled verbose sprite logging
            // if y < 10 {
            //     let color_mode_str = match obj.attribute0.color_mode {
            //         object_attributes::ColorMode::Palette4bpp => "4bpp",
            //         object_attributes::ColorMode::Palette8bpp => "8bpp",
            //     };
            //     logger::log(format!(
            //         "Sprite @ scanline {}: pos=({},{}), size={}x{}, tile={}, palette={}, mode={}",
            //         y,
            //         sprite_position.x,
            //         sprite_position.y,
            //         sprite_width,
            //         sprite_height,
            //         obj.attribute2.tile_number,
            //         obj.attribute2.palette_number,
            //         color_mode_str
            //     ));
            // }

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
                    || pixel_texture_sprite_origin.x >= sprite_size.x as f64
                    || pixel_texture_sprite_origin.y >= sprite_size.y as f64
                {
                    continue;
                }

                let pixel_texture_sprite_origin = pixel_texture_sprite_origin.map(|el| el as u16);

                // Pixel in texture space using tiles as dimensions
                let pixel_texture_tile = pixel_texture_sprite_origin / 8;

                // Offset of the pixel inside the tile
                let y_tile_idx = pixel_texture_sprite_origin.y % 8;
                let x_tile_idx = pixel_texture_sprite_origin.x % 8;

                let obj_character_vram_mapping = registers.get_obj_character_vram_mapping();

                // Calculate x_screen early for logging purposes
                let x_screen = sprite_position.x + idx;

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

                        // Disabled detailed 8bpp tile logging
                        // if y >= 105 && y <= 110 && x_screen >= 55 && x_screen <= 70 {
                        //     logger::log(format!(
                        //         "8bpp tile @ ({},{}) base_tile={} tile_offset={} final_tile={} offset={} pixel_tex=({},{}) tile_idx=({},{}) mapping={:?}",
                        //         x_screen, y, base_tile, tile_offset, tile_number, tile_data_offset,
                        //         pixel_texture_sprite_origin.x, pixel_texture_sprite_origin.y,
                        //         x_tile_idx, y_tile_idx, obj_character_vram_mapping
                        //     ));
                        // }

                        // TODO: Move 0x10000 to a variable. It is the offset where OBJ VRAM starts in vram
                        // let color_idx = memory.video_ram[0x10000 + tile_data_offset as usize];

                        // Disabled detailed color logging
                        // if y >= 105 && y <= 110 && x_screen >= 55 && x_screen <= 70 {
                        //     logger::log(format!(
                        //         "8bpp color @ ({},{}) vram_addr=0x{:X} color_idx={}",
                        //         x_screen, y, 0x10000 + tile_data_offset as usize, color_idx
                        //     ));
                        // }

                        // color_idx
                        memory.video_ram[0x10000 + tile_data_offset as usize]
                    }
                    object_attributes::ColorMode::Palette4bpp => {
                        let tile_number = obj.attribute2.tile_number
                            + match obj_character_vram_mapping {
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

                        // Combine with palette bank number to get final palette index
                        (obj.attribute2.palette_number << 4) | palette_offset_low
                    }
                };

                if x_screen >= self.sprite_pixels_scanline.len() as u16 {
                    continue;
                }

                // Palette index 0 is transparent for sprites
                if color_offset == 0 {
                    continue;
                }

                // Disabled pixel-level logging
                // if y >= 105 && y <= 110 && x_screen >= 55 && x_screen <= 70 {
                //     logger::log(format!(
                //         "8bpp pixel @ ({},{}) color_offset={} sprite_pos=({},{})",
                //         x_screen, y, color_offset, sprite_position.x, sprite_position.y
                //     ));
                // }

                let get_pixel_info_closure = || PixelInfo {
                    color: Self::read_color_from_obj_palette(
                        color_offset as usize,
                        memory.obj_palette_ram.as_slice(),
                    ),
                    priority: obj.attribute2.priority,
                };

                self.sprite_pixels_scanline[x_screen as usize] =
                    Some(self.sprite_pixels_scanline[x_screen as usize].map_or_else(
                        get_pixel_info_closure,
                        |current_pixel_info| {
                            if current_pixel_info.priority >= obj.attribute2.priority {
                                get_pixel_info_closure()
                            } else {
                                current_pixel_info
                            }
                        },
                    ));
                pixels_rendered += 1;
            }
        }

        // Debug log for multiple scanlines to catch sprites
        if y == 30 || y == 80 {
            logger::log(format!(
                "Scanline {y}: {sprites_on_scanline} sprites intersect, {pixels_rendered} pixels rendered (non-transparent)"
            ));
        }
    }

    pub fn handle_enter_vdraw(&mut self, memory: &Memory, registers: &Registers) {
        (self.obj_attributes_arr, self.rotation_scaling_params) =
            object_attributes::get_attributes(memory.obj_attributes.as_slice());

        // OAM debug logging, check sprites once per second (at vcount 0)
        if registers.vcount == 0 {
            static mut LOG_COUNTER: u32 = 0;

            unsafe {
                // Only log once per second (approximately 60 frames)
                LOG_COUNTER += 1;
                if LOG_COUNTER >= 60 {
                    LOG_COUNTER = 0;

                    let mut enabled_count = 0;
                    let mut unique_positions = std::collections::HashSet::new();

                    for i in 0..128 {
                        let obj = self.obj_attributes_arr[i];
                        if !matches!(
                            obj.attribute0.obj_mode,
                            object_attributes::ObjMode::Disabled
                        ) {
                            enabled_count += 1;
                            unique_positions
                                .insert((obj.attribute1.x_coordinate, obj.attribute0.y_coordinate));

                            // Log first 3 enabled sprites
                            if enabled_count <= 3 {
                                logger::log(format!(
                                    "OAM[{}]: pos=({},{}), tile={}, pal={}, size={}x{} (approx)",
                                    i,
                                    obj.attribute1.x_coordinate,
                                    obj.attribute0.y_coordinate,
                                    obj.attribute2.tile_number,
                                    obj.attribute2.palette_number,
                                    match obj.attribute0.obj_shape {
                                        object_attributes::ObjShape::Square =>
                                            8 << (obj.attribute1.obj_size as u8),
                                        _ => 8,
                                    },
                                    match obj.attribute0.obj_shape {
                                        object_attributes::ObjShape::Square =>
                                            8 << (obj.attribute1.obj_size as u8),
                                        _ => 8,
                                    }
                                ));
                            }
                        }
                    }
                    logger::log(format!(
                        "Total: {} enabled sprites, {} unique positions",
                        enabled_count,
                        unique_positions.len()
                    ));
                }
            }
        }

        self.process_sprites_scanline(registers, memory);
    }
}
