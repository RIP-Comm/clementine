use std::fmt::Display;

use crate::bitwise::Bits;

#[derive(PartialEq, Eq, Clone)]
pub enum PaletteType {
    BG,
    OBJ,
}

#[derive(Clone, Copy)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{},{})", self.red, self.green, self.blue)
    }
}

impl From<u16> for Color {
    fn from(color: u16) -> Self {
        // Color     Values     Bytes
        //-------------------------------
        // red ---> [0 - 31]    0-4 bit
        // green -> [0 - 31]    5-9 bit
        // blue --> [0 - 31]    10-14 bit
        // useless -------->    15 bit
        // bits are taken from upperbyte + lowerbyte u16

        let red = color.get_bits(0..=4);
        let green = color.get_bits(5..=9);
        let blue = color.get_bits(10..=14);

        Self {
            red: red as u8,
            green: green as u8,
            blue: blue as u8,
        }
    }
}

impl From<[u8; 2]> for Color {
    fn from(color: [u8; 2]) -> Self {
        let mut upper: u16 = color[0].into();
        upper <<= 8;
        let lower: u16 = color[1].into();

        let color_u16: u16 = upper | lower;
        Self::from(color_u16)
    }
}

impl From<Color> for [u8; 2] {
    fn from(color: Color) -> Self {
        let color_u16: u16 = color.into();
        [(color_u16 >> 8) as u8, color_u16 as u8]
    }
}

impl From<Color> for u16 {
    fn from(color: Color) -> Self {
        let red: Self = color.red.into();
        let green: Self = color.green.into();
        let blue: Self = color.blue.into();

        (blue << 10) + (green << 5) + red
    }
}

pub mod colors {

    use crate::render::color::Color;

    pub const BLACK: Color = Color::from_rgb(0, 0, 0);
    pub const RED: Color = Color::from_rgb(255, 0, 0);
    pub const GREEN: Color = Color::from_rgb(0, 255, 0);
    pub const BLUE: Color = Color::from_rgb(0, 0, 255);
    pub const WHITE: Color = Color::from_rgb(255, 255, 255);
}

#[cfg(test)]
mod test {

    use crate::render::color::Color;

    #[test]
    fn color_into_array_u8() {
        let color = Color {
            red: 8,   // 0b01000
            green: 4, // 0b00100
            blue: 2,  // 0b00010
        };
        let color_array: [u8; 2] = color.into();

        // this color is equal to 0b0000100010001000
        assert_eq!(color_array[0], 0b00001000);
        assert_eq!(color_array[1], 0b10001000);
    }

    #[test]
    fn color_into_u16() {
        let color = Color {
            red: 8,   // 0b01000
            green: 4, // 0b00100
            blue: 2,  // 0b00010
        };

        let color_u16: u16 = color.into();
        assert_eq!(color_u16, 0b0000100010001000);
    }

    #[test]
    fn color_from_u16() {
        // red: 8,     // 0b01000
        // green: 4,   // 0b00100
        // blue: 2     // 0b00010
        let color: u16 = 0b0000100010001000;

        let palette_color: Color = color.into();
        assert_eq!(palette_color.red, 8);
        assert_eq!(palette_color.green, 4);
        assert_eq!(palette_color.blue, 2);
    }

    #[test]
    fn color_from_array_u8() {
        let color_array: [u8; 2] = [0b00001000, 0b10001000];

        let palette_color = Color::from(color_array);
        assert_eq!(palette_color.red, 8);
        assert_eq!(palette_color.green, 4);
        assert_eq!(palette_color.blue, 2);
    }
}
