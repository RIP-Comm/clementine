use super::{color::Color, GBC_LCD_HEIGHT, GBC_LCD_WIDTH, LCD_HEIGHT, LCD_WIDTH};

pub struct GbaLcd {
    pixels: [[Color; LCD_WIDTH]; LCD_HEIGHT],
}

impl Default for GbaLcd {
    fn default() -> Self {
        Self {
            pixels: [[Color::default(); LCD_WIDTH]; LCD_HEIGHT],
        }
    }
}

impl GbaLcd {
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        self.pixels[x][y] = color;
    }

    pub fn set_gbc_pixel(&mut self, x: usize, y: usize, color: Color) {
        // GBC is rendered at the center of the screen
        let x_offset = (LCD_WIDTH - GBC_LCD_WIDTH) / 2;
        let y_offset = (LCD_HEIGHT - GBC_LCD_HEIGHT) / 2;
        self.set_pixel(x + x_offset, y + y_offset, color);
    }
}

impl std::ops::Index<(usize, usize)> for GbaLcd {
    type Output = Color;

    fn index(&self, (x, y): (usize, usize)) -> &Color {
        assert!(x < LCD_WIDTH && y < LCD_HEIGHT);
        &self.pixels[x][y]
    }
}

impl std::ops::IndexMut<(usize, usize)> for GbaLcd {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        assert!(x < LCD_WIDTH && y < LCD_HEIGHT);
        &mut self.pixels[x][y]
    }
}
