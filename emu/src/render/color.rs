use crate::bitwise::Bits;

#[derive(PartialEq, Eq, Clone)]
pub enum PaletteType {
    BG,
    OBJ,
}

#[derive(Default, Clone, Copy)]
pub struct Color(pub u16);

impl Color {
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

#[cfg(feature = "debug")]
impl From<Color> for [u8; 2] {
    fn from(color: Color) -> Self {
        [(color.0 >> 8) as u8, color.0 as u8]
    }
}

impl From<[u8; 2]> for Color {
    fn from(color: [u8; 2]) -> Self {
        let mut upper: u16 = color[0].into();
        upper <<= 8;
        let lower: u16 = color[1].into();

        Self(upper | lower)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn check() {
        let c = Color(0b0000100010001000);
        assert_eq!(c.red(), 0b01000);
        assert_eq!(c.green(), 0b00100);
        assert_eq!(c.blue(), 0b00010);

        assert_eq!(Color::from_rgb(1, 1, 1).0, 1057);

        let u: [u8; 2] = [1, 1];
        assert_eq!(Color::from(u).0, 257);
    }
}
