use super::{color::colors, color::Color, DISPLAY_HEIGHT, DISPLAY_WIDTH};

pub struct GbaLcd {
    pixels: [Color; DISPLAY_WIDTH * DISPLAY_HEIGHT],
}

impl Default for GbaLcd {
    fn default() -> Self {
        Self {
            pixels: [colors::BLACK; DISPLAY_WIDTH * DISPLAY_HEIGHT],
        }
    }
}

impl GbaLcd {
    pub const fn new() -> Self {
        Self {
            pixels: [colors::BLACK; DISPLAY_WIDTH * DISPLAY_HEIGHT],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        self[(x, y)] = color;
    }
}

impl std::ops::Index<(usize, usize)> for GbaLcd {
    type Output = Color;

    fn index(&self, (x, y): (usize, usize)) -> &Color {
        assert!(x < DISPLAY_WIDTH && y < DISPLAY_HEIGHT);
        &self.pixels[y * DISPLAY_WIDTH + x]
    }
}

impl std::ops::IndexMut<(usize, usize)> for GbaLcd {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        assert!(x < DISPLAY_WIDTH && y < DISPLAY_HEIGHT);
        &mut self.pixels[y * DISPLAY_WIDTH + x]
    }
}
