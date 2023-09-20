use logger::log;
use object_attributes::ObjAttributes;
use object_attributes::RotationScaling;

use crate::bitwise::Bits;

use self::object_attributes::ColorMode;
use self::object_attributes::ObjMode;
use self::object_attributes::ObjShape;
use self::object_attributes::ObjSize;
use self::object_attributes::TransformationKind;
use self::point::Point;

mod object_attributes;
mod point;

/// GBA display width
const LCD_WIDTH: usize = 240;

/// GBA display height
const LCD_HEIGHT: usize = 160;

// Sprites are positioned inside a 512x256 size (x position is 9 bits and y position is 8 bits)
/// World height
const WORLD_HEIGHT: u16 = 256;

#[derive(Default, Clone, Copy)]
pub struct Color(pub u16);

impl Color {
    pub const fn from_palette_color(value: u16) -> Self {
        Self(value)
    }

    pub fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        let red: u16 = red.into();
        let green: u16 = green.into();
        let blue: u16 = blue.into();

        Self((blue << 10) + (green << 5) + red)
    }

    pub fn red(&self) -> u8 {
        self.0.get_bits(0..=4) as u8
    }

    pub fn green(&self) -> u8 {
        self.0.get_bits(5..=9) as u8
    }

    pub fn blue(&self) -> u8 {
        self.0.get_bits(10..=14) as u8
    }
}

enum ObjMappingKind {
    TwoDimensional,
    OneDimensional,
}

impl From<bool> for ObjMappingKind {
    fn from(value: bool) -> Self {
        match value {
            false => Self::TwoDimensional,
            true => Self::OneDimensional,
        }
    }
}

#[derive(Copy, Clone, Default)]
struct PixelInfo {
    color: Color,
    priority: u8,
}

pub struct Lcd {
    /// LCD Control
    pub dispcnt: u16,
    /// Undocumented - Green Swap
    pub green_swap: u16,
    /// General LCD Status (STAT, LYC)
    pub dispstat: u16,
    /// Vertical Counter (LY)
    pub vcount: u16,
    /// BG0 Control
    pub bg0cnt: u16,
    /// BG1 Control
    pub bg1cnt: u16,
    /// BG2 Control
    pub bg2cnt: u16,
    /// BG3 Control
    pub bg3cnt: u16,
    /// BG0 X-Offset
    pub bg0hofs: u16,
    /// BG0 Y_Offset
    pub bg0vofs: u16,
    /// BG1 X-Offset
    pub bg1hofs: u16,
    /// BG1 Y_Offset
    pub bg1vofs: u16,
    /// BG2 X-Offset
    pub bg2hofs: u16,
    /// BG2 Y_Offset
    pub bg2vofs: u16,
    /// BG3 X-Offset
    pub bg3hofs: u16,
    /// BG3 Y_Offset
    pub bg3vofs: u16,
    /// BG2 Rotation/Scaling Parameter A (dx)
    pub bg2pa: u16,
    /// BG2 Rotation/Scaling Parameter B (dmx)
    pub bg2pb: u16,
    /// BG2 Rotation/Scaling Parameter C (dy)
    pub bg2pc: u16,
    /// BG2 Rotation/Scaling Parameter D (dmy)
    pub bg2pd: u16,
    /// BG2 Reference Point X-Coordinate
    pub bg2x: u32,
    /// BG2 Reference Point Y-Coordinate
    pub bg2y: u32,
    /// BG3 Rotation/Scaling Parameter A (dx)
    pub bg3pa: u16,
    /// BG3 Rotation/Scaling Parameter B (dmx)
    pub bg3pb: u16,
    /// BG3 Rotation/Scaling Parameter C (dy)
    pub bg3pc: u16,
    /// BG3 Rotation/Scaling Parameter D (dmy)
    pub bg3pd: u16,
    /// BG3 Reference Point X-Coordinate
    pub bg3x: u32,
    /// BG3 Reference Point Y-Coordinate
    pub bg3y: u32,
    /// Window 0 Horizontal Dimensions
    pub win0h: u16,
    /// Window 1 Horizontal Dimensions
    pub win1h: u16,
    /// Window 0 Vertical Dimensions
    pub win0v: u16,
    /// Window 1 Vertical Dimensions
    pub win1v: u16,
    /// Inside of Window 0 and 1
    pub winin: u16,
    /// Inside of OBJ Window & Outside of Windows
    pub winout: u16,
    /// Mosaic Size
    pub mosaic: u16,
    /// Color Special Effects Selection
    pub bldcnt: u16,
    /// Alpha Blending Coefficients
    pub bldalpha: u16,
    /// Brightness (Fade-In/Out) Coefficient
    pub bldy: u16,

    /// From 0x05000000 to  0x050001FF (512 bytes, 256 colors).
    pub bg_palette_ram: Vec<u8>,
    /// From 0x05000200 to 0x050003FF (512 bytes, 256 colors).
    pub obj_palette_ram: Vec<u8>,
    /// From 0x06000000 to 0x06017FFF (96 kb).
    pub video_ram: Vec<u8>,
    /// From 0x07000000 to 0x070003FF (1kbyte)
    pub obj_attributes: Vec<u8>,

    pub buffer: [[Color; LCD_WIDTH]; LCD_HEIGHT],
    pixel_index: u32,
    should_draw: bool,
    obj_attributes_arr: [ObjAttributes; 128],
    rotation_scaling_params: [RotationScaling; 32],
    sprite_pixels_scanline: [Option<PixelInfo>; LCD_WIDTH],
}

impl Default for Lcd {
    fn default() -> Self {
        Self {
            dispcnt: 0,
            green_swap: 0,
            dispstat: 0,
            vcount: 0,
            bg0cnt: 0,
            bg1cnt: 0,
            bg2cnt: 0,
            bg3cnt: 0,
            bg0hofs: 0,
            bg0vofs: 0,
            bg1hofs: 0,
            bg1vofs: 0,
            bg2hofs: 0,
            bg2vofs: 0,
            bg3hofs: 0,
            bg3vofs: 0,
            bg2pa: 0,
            bg2pb: 0,
            bg2pc: 0,
            bg2pd: 0,
            bg2x: 0,
            bg2y: 0,
            bg3pa: 0,
            bg3pb: 0,
            bg3pc: 0,
            bg3pd: 0,
            bg3x: 0,
            bg3y: 0,
            win0h: 0,
            win1h: 0,
            win0v: 0,
            win1v: 0,
            winin: 0,
            winout: 0,
            mosaic: 0,
            bldcnt: 0,
            bldalpha: 0,
            bldy: 0,
            bg_palette_ram: vec![0; 0x200],
            obj_palette_ram: vec![0; 0x200],
            video_ram: vec![0; 0x00018000],
            obj_attributes: vec![0; 0x400],
            pixel_index: 0,
            buffer: [[Color::default(); LCD_WIDTH]; LCD_HEIGHT],
            should_draw: false,
            obj_attributes_arr: [ObjAttributes::default(); 128],
            rotation_scaling_params: [RotationScaling::default(); 32],
            sprite_pixels_scanline: [None; LCD_WIDTH],
        }
    }
}
#[derive(Default)]
pub struct LcdStepOutput {
    pub request_vblank_irq: bool,
    pub request_hblank_irq: bool,
    pub request_vcount_irq: bool,
}

impl Lcd {
    fn read_color_from_obj_palette(&self, color_idx: usize) -> Color {
        let low_nibble = self.obj_palette_ram[color_idx] as u16;
        let high_nibble = self.obj_palette_ram[color_idx + 1] as u16;

        Color::from_palette_color((high_nibble << 8) | low_nibble)
    }

    fn get_texture_space_point(
        &self,
        sprite_size: Point<u16>,
        pixel_screen_sprite_origin: Point<u16>,
        transformation_kind: TransformationKind,
        obj_mode: ObjMode,
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
                    ObjMode::Affine => sprite_size / 2.0,
                    ObjMode::AffineDouble => sprite_size,
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

    fn process_sprites_scanline(&mut self) {
        self.sprite_pixels_scanline = [None; LCD_WIDTH];

        let y = self.vcount;

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

            let (sprite_width, sprite_height) =
                match (obj.attribute0.obj_shape, obj.attribute1.obj_size) {
                    (ObjShape::Square, ObjSize::Size0) => (8_u8, 8_u8),
                    (ObjShape::Horizontal, ObjSize::Size0) => (16, 8),
                    (ObjShape::Vertical, ObjSize::Size0) => (8, 16),
                    (ObjShape::Square, ObjSize::Size1) => (16, 16),
                    (ObjShape::Horizontal, ObjSize::Size1) => (32, 8),
                    (ObjShape::Vertical, ObjSize::Size1) => (8, 32),
                    (ObjShape::Square, ObjSize::Size2) => (32, 32),
                    (ObjShape::Horizontal, ObjSize::Size2) => (32, 16),
                    (ObjShape::Vertical, ObjSize::Size2) => (16, 32),
                    (ObjShape::Square, ObjSize::Size3) => (64, 64),
                    (ObjShape::Horizontal, ObjSize::Size3) => (64, 32),
                    (ObjShape::Vertical, ObjSize::Size3) => (32, 64),
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
                    ColorMode::Palette8bpp => {
                        // We multiply *2 because in 8bpp tiles indeces are always even
                        let tile_number = obj.attribute2.tile_number
                            + match self.get_obj_character_vram_mapping() {
                                ObjMappingKind::OneDimensional => {
                                    // In this case memory is seen as a single array.
                                    // tile_number is the offset of the first tile in memory.
                                    // then we access [y][x] by doing y*number_cols + x, as if we were to access an array as a matrix
                                    pixel_texture_tile.y * sprite_size_tile.x * 2
                                        + pixel_texture_tile.x * 2
                                }
                                ObjMappingKind::TwoDimensional => {
                                    // A charblock is 32x32 tiles
                                    pixel_texture_tile.y * 32 + pixel_texture_tile.x * 2
                                }
                            };

                        // A tile is 8x8 mini-bitmap.
                        // A tile is 64bytes long in 8bpp.
                        let palette_offset =
                            tile_number as u32 * 32 + y_tile_idx as u32 * 8 + x_tile_idx as u32;

                        // TODO: Move 0x10000 to a variable. It is the offset where OBJ VRAM starts in vram
                        self.video_ram[0x10000 + palette_offset as usize]
                    }
                    ColorMode::Palette4bpp => {
                        let tile_number = obj.attribute2.tile_number
                            + match self.get_obj_character_vram_mapping() {
                                ObjMappingKind::OneDimensional => {
                                    // In this case memory is seen as a single array.
                                    // tile_number is the offset of the first tile in memory.
                                    // then we access [y][x] by doing y*number_cols + x, as if we were to access an array as a matrix
                                    pixel_texture_tile.y * sprite_size_tile.x + pixel_texture_tile.x
                                }
                                ObjMappingKind::TwoDimensional => {
                                    // A charblock is 32x32 tiles
                                    obj.attribute2.tile_number
                                        + pixel_texture_tile.y * 32
                                        + pixel_texture_tile.x
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
                        self.video_ram[0x10000 + palette_offset as usize]
                    }
                };

                let x_screen = sprite_position.x + idx;

                if x_screen >= self.sprite_pixels_scanline.len() as u16 {
                    continue;
                }

                let get_pixel_info_closure = || PixelInfo {
                    color: self.read_color_from_obj_palette(color_offset as usize),
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

    pub fn step(&mut self) -> LcdStepOutput {
        // This will be much more complex obviously
        let mut output = LcdStepOutput::default();

        if self.vcount < 160 {
            // We either are in Vdraw or Hblank
            if self.pixel_index == 0 {
                // We're drawing the first pixel of the scanline, we're entering Vdraw

                self.set_hblank_flag(false);
                self.set_vblank_flag(false);

                self.should_draw = true;

                (self.obj_attributes_arr, self.rotation_scaling_params) =
                    object_attributes::get_attributes(self.obj_attributes.as_slice());
                self.process_sprites_scanline();
            } else if self.pixel_index == 240 {
                // We're entering Hblank

                self.set_hblank_flag(true);

                if self.get_hblank_irq_enable() {
                    output.request_hblank_irq = true;
                }

                self.should_draw = false;
            }
        } else if self.vcount == 160 && self.pixel_index == 0 {
            // We're drawing the first pixel of the Vblank period

            self.set_vblank_flag(true);

            if self.get_vblank_irq_enable() {
                output.request_vblank_irq = true;
            }

            self.should_draw = false;
        }

        if self.should_draw {
            let pixel_y = self.vcount;
            let pixel_x = self.pixel_index;

            self.buffer[pixel_y as usize][pixel_x as usize] = self.sprite_pixels_scanline
                [pixel_x as usize]
                .map_or_else(|| Color::from_rgb(31, 31, 31), |info| info.color);
        }

        log(format!(
            "mode: {:?}, BG2: {:?} BG3: {:?}",
            self.get_bg_mode(),
            self.get_bg2_enabled(),
            self.get_bg3_enabled()
        ));

        self.pixel_index += 1;

        if self.pixel_index == 308 {
            // We finished to draw the scanline
            self.pixel_index = 0;
            self.vcount += 1;

            // We finished to draw the screen
            if self.vcount == 228 {
                self.vcount = 0;
            }
        }

        self.set_vcounter_flag(false);

        if self.vcount.get_byte(0) == self.get_vcount_setting() {
            self.set_vcounter_flag(true);

            if self.get_vcounter_irq_enable() {
                output.request_vcount_irq = true;
            }
        }

        output
    }

    fn get_bg2_enabled(&self) -> bool {
        self.dispcnt.get_bit(10)
    }

    fn get_bg3_enabled(&self) -> bool {
        self.dispcnt.get_bit(11)
    }

    /// Info about vram fields used to render display.
    pub fn get_bg_mode(&self) -> u8 {
        self.dispcnt.get_bits(0..=2).try_into().unwrap()
    }

    fn get_obj_character_vram_mapping(&self) -> ObjMappingKind {
        self.dispcnt.get_bit(6).into()
    }

    fn get_vcount_setting(&self) -> u8 {
        self.dispstat.get_byte(1)
    }

    fn get_vblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(3)
    }

    fn get_hblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(4)
    }

    fn get_vcounter_irq_enable(&self) -> bool {
        self.dispstat.get_bit(5)
    }

    fn set_vblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(0, value);
    }

    fn set_hblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(1, value);
    }

    fn set_vcounter_flag(&mut self, value: bool) {
        self.dispstat.set_bit(2, value);
    }
}
