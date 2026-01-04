#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]

//! Object Attribute Memory (OAM) parsing and sprite attributes.
//!
//! This module handles the parsing of OAM data into sprite attributes that the
//! PPU uses for rendering. The GBA supports 128 hardware sprites (called "objects")
//! with various sizes, transformations, and rendering modes.
//!
//! # OAM Structure
//!
//! OAM is 1KB (1024 bytes) containing 128 sprite entries. Each entry is 8 bytes:
//!
//! ```text
//! Bytes 0-1: Attribute 0
//!   Bits 0-7:   Y coordinate (0-255, wraps at 256)
//!   Bits 8-9:   Object mode (Normal/Affine/Disabled/AffineDouble)
//!   Bits 10-11: Graphics mode (Normal/AlphaBlend/ObjectWindow)
//!   Bit 12:     Mosaic enable
//!   Bit 13:     Color mode (0=4bpp/16 colors, 1=8bpp/256 colors)
//!   Bits 14-15: Object shape (Square/Horizontal/Vertical)
//!
//! Bytes 2-3: Attribute 1
//!   Bits 0-8:   X coordinate (0-511, wraps at 512)
//!   Bits 9-13:  Affine parameter index (if affine) OR bits 12-13 are H/V flip (if normal)
//!   Bits 14-15: Object size (Size0-Size3, meaning depends on shape)
//!
//! Bytes 4-5: Attribute 2
//!   Bits 0-9:   Tile number (base tile in VRAM)
//!   Bits 10-11: Priority (0=highest, 3=lowest)
//!   Bits 12-15: Palette bank (4bpp mode only)
//!
//! Bytes 6-7: Rotation/Scaling parameter (shared, see below)
//! ```
//!
//! # Sprite Sizes
//!
//! Sprite dimensions are determined by combining Shape and Size:
//!
//! | Shape      | Size0  | Size1  | Size2  | Size3  |
//! |------------|--------|--------|--------|--------|
//! | Square     | 8×8    | 16×16  | 32×32  | 64×64  |
//! | Horizontal | 16×8   | 32×8   | 32×16  | 64×32  |
//! | Vertical   | 8×16   | 8×32   | 16×32  | 32×64  |
//!
//! # Rotation/Scaling Parameters
//!
//! OAM contains 32 rotation/scaling parameter sets, but they're interleaved with
//! sprite data (stored in the unused bytes 6-7 of each sprite entry). Each set
//! consists of 4 parameters (PA, PB, PC, PD) spread across 4 consecutive sprites:
//!
//! ```text
//! Sprite 0, bytes 6-7: PA for rotation group 0
//! Sprite 1, bytes 6-7: PB for rotation group 0
//! Sprite 2, bytes 6-7: PC for rotation group 0
//! Sprite 3, bytes 6-7: PD for rotation group 0
//! Sprite 4, bytes 6-7: PA for rotation group 1
//! ... and so on
//! ```
//!
//! The parameters form a 2×2 transformation matrix applied to affine sprites:
//! ```text
//! | PA  PB |   Stored as 8.8 fixed-point signed values
//! | PC  PD |   (divide by 256 to get the float value)
//! ```
//!
//! # Coordinate System
//!
//! - Y coordinates use 8 bits (0-255), with values ≥160 appearing off-screen top
//! - X coordinates use 9 bits (0-511), with values ≥240 appearing off-screen left
//! - Sprites wrap around: a sprite at Y=250 with height 16 will show at top of screen
//!
//! # Priority and Ordering
//!
//! When sprites overlap:
//! 1. Lower priority number wins (0 beats 1, 1 beats 2, etc.)
//! 2. At equal priority, lower OAM index wins (sprite 0 beats sprite 1)
//!
//! # References
//!
//! - TONC: <https://www.coranac.com/tonc/text/regobj.htm>
//! - GBATEK: <https://problemkaputt.de/gbatek.htm#lcdobjoamattributes>

use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

/// Sprite rendering mode (Attribute 0, bits 8-9).
///
/// Controls how the sprite is rendered and whether affine transformations apply.
#[derive(Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjMode {
    /// Normal sprite rendering with optional H/V flip.
    #[default]
    Normal,
    /// Affine transformation using rotation/scaling parameters.
    /// Sprite is rendered within its normal bounding box.
    Affine,
    /// Sprite is not rendered.
    Disabled,
    /// Affine transformation with double-size bounding box.
    /// Allows the transformed sprite to extend beyond its normal bounds
    /// without clipping (useful for rotations).
    AffineDouble,
}

impl From<u16> for ObjMode {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::Affine,
            2 => Self::Disabled,
            3 => Self::AffineDouble,
            _ => unreachable!(),
        }
    }
}

/// Sprite graphics mode (Attribute 0, bits 10-11).
///
/// Controls special rendering effects for the sprite.
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub enum GfxMode {
    /// Normal sprite rendering.
    #[default]
    Normal,
    /// Semi-transparent sprite (uses blend registers for alpha).
    AlphaBlending,
    /// Sprite acts as a mask for the object window.
    /// Pixels covered by this sprite use WINOBJ layer enable settings.
    ObjectWindow,
}

impl TryFrom<u16> for GfxMode {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::AlphaBlending),
            2 => Ok(Self::ObjectWindow),
            4 => Err("Forbidden GfxMode"),
            _ => unreachable!(),
        }
    }
}

/// Sprite color depth (Attribute 0, bit 13).
///
/// Determines how many colors the sprite can use and how tile data is interpreted.
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub enum ColorMode {
    /// 4 bits per pixel
    ///
    /// Each tile is 32 bytes (8×8 pixels × 4 bits).
    /// The palette bank (0-15) is selected in Attribute 2.
    /// Index 0 within the bank is transparent.
    #[default]
    Palette4bpp,
    /// 8 bits per pixel
    ///
    /// Each tile is 64 bytes (8×8 pixels × 8 bits).
    /// Uses the entire 256-color OBJ palette.
    /// Index 0 is transparent.
    Palette8bpp,
}

impl From<bool> for ColorMode {
    fn from(value: bool) -> Self {
        if value {
            Self::Palette8bpp
        } else {
            Self::Palette4bpp
        }
    }
}

/// Sprite shape (Attribute 0, bits 14-15).
///
/// Combined with [`ObjSize`] to determine the sprite's pixel dimensions.
/// See module documentation for the full size table.
#[derive(Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjShape {
    /// Square sprite (8×8, 16×16, 32×32, or 64×64).
    #[default]
    Square,
    /// Wide sprite (16×8, 32×8, 32×16, or 64×32).
    Horizontal,
    /// Tall sprite (8×16, 8×32, 16×32, or 32×64).
    Vertical,
}

impl TryFrom<u16> for ObjShape {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Square),
            1 => Ok(Self::Horizontal),
            2 => Ok(Self::Vertical),
            3 => Err("Prohibited ObjShape"),
            _ => unreachable!(),
        }
    }
}

/// Sprite size selector (Attribute 1, bits 14-15).
///
/// Combined with [`ObjShape`] to determine actual pixel dimensions:
///
/// | Shape      | Size0  | Size1  | Size2  | Size3  |
/// |------------|--------|--------|--------|--------|
/// | Square     | 8×8    | 16×16  | 32×32  | 64×64  |
/// | Horizontal | 16×8   | 32×8   | 32×16  | 64×32  |
/// | Vertical   | 8×16   | 8×32   | 16×32  | 32×64  |
#[derive(Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjSize {
    #[default]
    Size0,
    Size1,
    Size2,
    Size3,
}

impl From<u16> for ObjSize {
    fn from(value: u16) -> Self {
        match value {
            0 => Self::Size0,
            1 => Self::Size1,
            2 => Self::Size2,
            3 => Self::Size3,
            _ => unreachable!(),
        }
    }
}

/// OAM Attribute 0
///
/// First 16-bit word of a sprite's OAM entry.
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttribute0 {
    /// Y coordinate on screen (0-255). Values ≥160 wrap to top.
    pub y_coordinate: u8,
    /// Rendering mode (normal, affine, disabled, or affine double-size).
    pub obj_mode: ObjMode,
    /// Graphics effect mode (normal, alpha blend, or object window).
    pub gfx_mode: GfxMode,
    /// Whether mosaic effect is applied to this sprite.
    obj_mosaic: bool,
    /// Color depth (4bpp = 16 colors, 8bpp = 256 colors).
    pub color_mode: ColorMode,
    /// Sprite shape (square, horizontal, or vertical).
    pub obj_shape: ObjShape,
}

impl TryFrom<u16> for ObjAttribute0 {
    type Error = &'static str;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Ok(Self {
            y_coordinate: value.get_bits(0..=7) as u8,
            obj_mode: value.get_bits(8..=9).into(),
            gfx_mode: value.get_bits(10..=11).try_into().unwrap(),
            obj_mosaic: value.get_bit(12),
            color_mode: value.get_bit(13).into(),
            obj_shape: value.get_bits(14..=15).try_into().unwrap(),
        })
    }
}

/// Sprite transformation type.
///
/// For normal sprites, bits 12-13 of Attribute 1 control flipping.
/// For affine sprites, bits 9-13 select which rotation/scaling parameter set to use.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum TransformationKind {
    /// Affine transformation using one of 32 rotation/scaling parameter sets.
    RotationScaling {
        /// Index (0-31) into the rotation/scaling parameter array.
        rotation_scaling_parameter: u8,
    },
    /// Simple flip transformation (normal sprites only).
    Flip {
        /// Mirror sprite horizontally.
        horizontal_flip: bool,
        /// Mirror sprite vertically.
        vertical_flip: bool,
    },
}

impl Default for TransformationKind {
    fn default() -> Self {
        Self::Flip {
            horizontal_flip: false,
            vertical_flip: false,
        }
    }
}

/// OAM Attribute 1
///
/// Second 16-bit word of a sprite's OAM entry.
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttribute1 {
    /// X coordinate on screen (0-511). Values ≥240 wrap to left side.
    pub x_coordinate: u16,
    /// Transformation to apply (flip or affine rotation/scaling).
    pub transformation_kind: TransformationKind,
    /// Size selector (combined with shape to get pixel dimensions).
    pub obj_size: ObjSize,
}

impl ObjAttribute1 {
    /// Parse Attribute 1 from raw value.
    ///
    /// The interpretation of bits 9-13 depends on the sprite's [`ObjMode`]:
    /// - For affine sprites: rotation/scaling parameter index (0-31)
    /// - For normal sprites: bits 12-13 are horizontal/vertical flip flags
    fn from_value(value: u16, obj_mode: ObjMode) -> Self {
        Self {
            x_coordinate: value.get_bits(0..=8),
            transformation_kind: match obj_mode {
                ObjMode::Affine | ObjMode::AffineDouble => TransformationKind::RotationScaling {
                    rotation_scaling_parameter: value.get_bits(9..=13) as u8,
                },
                ObjMode::Normal | ObjMode::Disabled => TransformationKind::Flip {
                    horizontal_flip: value.get_bit(12),
                    vertical_flip: value.get_bit(13),
                },
            },
            obj_size: value.get_bits(14..=15).into(),
        }
    }
}

/// OAM Attribute 2
///
/// Third 16-bit word of a sprite's OAM entry.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttribute2 {
    /// Base tile number in OBJ VRAM (0-1023).
    ///
    /// Points to the first tile of the sprite in OBJ character data
    /// (VRAM starting at `0x0601_0000`). Additional tiles are determined
    /// by sprite size and the 1D/2D mapping mode in DISPCNT.
    pub tile_number: u16,
    /// Display priority (0-3). Lower = drawn on top.
    ///
    /// Sprites are compared against background layers with the same priority.
    /// At equal priority, lower OAM index wins among sprites.
    pub priority: u8,
    /// Palette bank for 4bpp sprites (0-15).
    ///
    /// Selects which 16-color palette from OBJ palette RAM to use.
    /// Ignored for 8bpp sprites (they use the full 256-color palette).
    pub palette_number: u8,
}

impl Default for ObjAttribute2 {
    fn default() -> Self {
        Self {
            tile_number: 0,
            // Lowest priority
            priority: 3,
            palette_number: 0,
        }
    }
}

impl From<u16> for ObjAttribute2 {
    fn from(value: u16) -> Self {
        Self {
            tile_number: value.get_bits(0..=9),
            priority: value.get_bits(10..=11) as u8,
            palette_number: value.get_bits(12..=15) as u8,
        }
    }
}

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttributes {
    pub attribute0: ObjAttribute0,
    pub attribute1: ObjAttribute1,
    pub attribute2: ObjAttribute2,
}

impl TryFrom<[u16; 3]> for ObjAttributes {
    type Error = &'static str;
    fn try_from(value: [u16; 3]) -> Result<Self, Self::Error> {
        let obj_attribute0 = value[0].try_into().unwrap();

        Ok(Self {
            attribute0: obj_attribute0,
            attribute1: ObjAttribute1::from_value(value[1], obj_attribute0.obj_mode),
            attribute2: value[2].into(),
        })
    }
}

#[derive(Default, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct RotationScaling {
    pub pa: u16,
    pub pb: u16,
    pub pc: u16,
    pub pd: u16,
}

impl RotationScaling {
    /// Gives back the result of P*T where
    /// P = [ pa  pb ]
    ///     [ pc  pd ]
    /// and
    /// T = \[ x \]
    ///     \[ y \]
    /// pa, pb, pc, pd are first converted to floating number from the fixed point representation
    #[allow(clippy::many_single_char_names)] // a,b,c,d,x,y are standard matrix/vector notation
    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        let a = Self::get_float_from_fixed_point(self.pa);
        let b = Self::get_float_from_fixed_point(self.pb);
        let c = Self::get_float_from_fixed_point(self.pc);
        let d = Self::get_float_from_fixed_point(self.pd);

        (x.mul_add(a, y * b), x.mul_add(c, y * d))
    }

    fn get_float_from_fixed_point(value: u16) -> f64 {
        // We interpret the value as signed and we divide by 2^8 since the rotation/scaling parameter
        // is represented as an 8.8 fixed point value.
        #[allow(clippy::cast_possible_wrap)] // Intentional reinterpretation as signed
        let signed = value as i16;
        f64::from(signed) / 256.0
    }
}

impl Index<usize> for RotationScaling {
    type Output = u16;
    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.pa,
            1 => &self.pb,
            2 => &self.pc,
            3 => &self.pd,
            _ => panic!("Index out of bound"),
        }
    }
}
impl IndexMut<usize> for RotationScaling {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.pa,
            1 => &mut self.pb,
            2 => &mut self.pc,
            3 => &mut self.pd,
            _ => panic!("Index out of bound"),
        }
    }
}

pub fn get_attributes(oam_memory: &[u8]) -> ([ObjAttributes; 128], [RotationScaling; 32]) {
    let mut obj_attributes = [ObjAttributes::default(); 128];
    let mut rotation_scalings = [RotationScaling::default(); 32];

    for (idx, [attribute0, attribute1, attribute2, rotation_scaling]) in oam_memory
        .chunks_exact(8)
        .map(|values| {
            let mut result = [u16::default(); 4];
            result[0] = ((values[1] as u16) << 8) | values[0] as u16;
            result[1] = ((values[3] as u16) << 8) | values[2] as u16;
            result[2] = ((values[5] as u16) << 8) | values[4] as u16;
            result[3] = ((values[7] as u16) << 8) | values[6] as u16;

            result
        })
        .enumerate()
    {
        obj_attributes[idx] = [attribute0, attribute1, attribute2].try_into().unwrap();
        rotation_scalings[idx / 4][idx % 4] = rotation_scaling;
    }

    (obj_attributes, rotation_scalings)
}
