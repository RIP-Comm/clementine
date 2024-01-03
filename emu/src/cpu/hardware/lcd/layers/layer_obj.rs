use crate::cpu::hardware::lcd;
use crate::cpu::hardware::lcd::object_attributes;
use crate::cpu::hardware::lcd::point::Point;
use crate::cpu::hardware::lcd::Color;
use crate::cpu::hardware::lcd::{PixelInfo, LCD_WIDTH, WORLD_HEIGHT};

use super::Layer;
use crate::bitwise::Bits;
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

    obj_mapping_kind: lcd::ObjMappingKind,
}

impl Default for LayerObj {
    fn default() -> Self {
        Self {
            obj_attributes_arr: [object_attributes::ObjAttributes::default(); 128],
            rotation_scaling_params: [object_attributes::RotationScaling::default(); 32],
            sprite_pixels_scanline: [None; LCD_WIDTH],
            obj_mapping_kind: lcd::ObjMappingKind::TwoDimensional,
        }
    }
}

impl Layer for LayerObj {
    #[allow(unused_variables)]
    fn render(&self, x: usize, y: usize) -> Option<Color> {
        self.sprite_pixels_scanline[x].map(|info| info.color)
    }
}

impl LayerObj {
    const fn read_color_from_obj_palette(&self, color_idx: usize, obj_palette_ram: &[u8]) -> Color {
        let low_nibble = obj_palette_ram[color_idx] as u16;
        let high_nibble = obj_palette_ram[color_idx + 1] as u16;

        Color::from_palette_color((high_nibble << 8) | low_nibble)
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
        } else {
            // TODO: Implement flip
            pixel_screen_sprite_origin.map(|el| el as f64)
        }
    }

    fn process_sprites_scanline(&mut self, y: u16, video_ram: &[u8], obj_palette_ram: &[u8]) {
        self.sprite_pixels_scanline = [None; LCD_WIDTH];

        for obj in self.obj_attributes_arr.into_iter() {
            if matches!(
                obj.attribute0.obj_mode,
                object_attributes::ObjMode::Disabled
            ) || matches!(
                obj.attribute0.gfx_mode,
                object_attributes::GfxMode::ObjectWindow
            ) {
                continue;
            }

            use object_attributes::ObjShape::*;
            use object_attributes::ObjSize::*;
            let (sprite_width, sprite_height) =
                match (obj.attribute0.obj_shape, obj.attribute1.obj_size) {
                    (Square, Size0) => (8_u8, 8_u8),
                    (Horizontal, Size0) => (16, 8),
                    (Vertical, Size0) => (8, 16),
                    (Square, Size1) => (16, 16),
                    (Horizontal, Size1) => (32, 8),
                    (Vertical, Size1) => (8, 32),
                    (Square, Size2) => (32, 32),
                    (Horizontal, Size2) => (32, 16),
                    (Vertical, Size2) => (16, 32),
                    (Square, Size3) => (64, 64),
                    (Horizontal, Size3) => (64, 32),
                    (Vertical, Size3) => (32, 64),
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

                let color_offset = match obj.attribute0.color_mode {
                    object_attributes::ColorMode::Palette8bpp => {
                        // We multiply *2 because in 8bpp tiles indeces are always even
                        let tile_number = obj.attribute2.tile_number
                            + match self.obj_mapping_kind {
                                lcd::ObjMappingKind::OneDimensional => {
                                    // In this case memory is seen as a single array.
                                    // tile_number is the offset of the first tile in memory.
                                    // then we access [y][x] by doing y*number_cols + x, as if we were to access an array as a matrix
                                    pixel_texture_tile.y * sprite_size_tile.x * 2
                                        + pixel_texture_tile.x * 2
                                }
                                lcd::ObjMappingKind::TwoDimensional => {
                                    // A charblock is 32x32 tiles
                                    pixel_texture_tile.y * 32 + pixel_texture_tile.x * 2
                                }
                            };

                        // A tile is 8x8 mini-bitmap.
                        // A tile is 64bytes long in 8bpp.
                        let palette_offset =
                            tile_number as u32 * 32 + y_tile_idx as u32 * 8 + x_tile_idx as u32;

                        // TODO: Move 0x10000 to a variable. It is the offset where OBJ VRAM starts in vram
                        video_ram[0x10000 + palette_offset as usize]
                    }
                    object_attributes::ColorMode::Palette4bpp => {
                        let tile_number = obj.attribute2.tile_number
                            + match self.obj_mapping_kind {
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
                        let tile_data = tile_number * 32 + y_tile_idx * 4 + x_tile_idx / 2;

                        let palette_offset_low = if tile_data % 2 == 0 {
                            tile_data.get_bits(0..=3)
                        } else {
                            tile_data.get_bits(4..=7)
                        };

                        let palette_offset =
                            (obj.attribute2.palette_number << 4) | (palette_offset_low as u8);
                        video_ram[0x10000 + palette_offset as usize]
                    }
                };

                let x_screen = sprite_position.x + idx;

                if x_screen >= self.sprite_pixels_scanline.len() as u16 {
                    continue;
                }

                let get_pixel_info_closure = || PixelInfo {
                    color: self.read_color_from_obj_palette(color_offset as usize, obj_palette_ram),
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
            }
        }
    }

    pub fn handle_enter_vdraw(
        &mut self,
        y: u16,
        obj_mapping_kind: lcd::ObjMappingKind,
        video_ram: &[u8],
        obj_attributes_ram: &[u8],
        obj_palette_ram: &[u8],
    ) {
        (self.obj_attributes_arr, self.rotation_scaling_params) =
            object_attributes::get_attributes(obj_attributes_ram);
        self.process_sprites_scanline(y, video_ram, obj_palette_ram);

        self.obj_mapping_kind = obj_mapping_kind;
    }
}
