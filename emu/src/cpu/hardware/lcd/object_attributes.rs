// We use nomenclature coming from https://www.coranac.com/tonc/text/regobj.htm#sec-oam

use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub enum ObjMode {
    #[default]
    Normal,
    Affine,
    Disabled,
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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub enum GfxMode {
    #[default]
    Normal,
    AlphaBlending,
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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub enum ColorMode {
    /// 16 colors
    #[default]
    Palette4bpp,
    /// 256 colors
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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub enum ObjShape {
    #[default]
    Square,
    Horizontal,
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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
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

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttribute0 {
    pub y_coordinate: u8,
    pub obj_mode: ObjMode,
    pub gfx_mode: GfxMode,
    obj_mosaic: bool,
    pub color_mode: ColorMode,
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

#[allow(dead_code)]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum TransformationKind {
    RotationScaling {
        rotation_scaling_parameter: u8,
    },
    Flip {
        horizontal_flip: bool,
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

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttribute1 {
    pub x_coordinate: u16,
    pub transformation_kind: TransformationKind,
    pub obj_size: ObjSize,
}

impl ObjAttribute1 {
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

#[allow(dead_code)]
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ObjAttribute2 {
    pub tile_number: u16,
    pub priority: u8,
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

#[allow(dead_code)]
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

#[derive(Default, Copy, Clone, Serialize, Deserialize)]
pub struct RotationScaling {
    pa: u16,
    pb: u16,
    pc: u16,
    pd: u16,
}

impl RotationScaling {
    /// Gives back the result of P*T where
    /// P = [ pa  pb ]
    ///     [ pc  pd ]
    /// and
    /// T = \[ x \]
    ///     \[ y \]
    /// pa, pb, pc, pd are first converted to floating number from the fixed point representation
    #[allow(clippy::many_single_char_names)]
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
        (value as i16) as f64 / 256.0
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

#[allow(dead_code)]
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
