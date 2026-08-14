use super::{GBC_LCD_HEIGHT, GBC_LCD_WIDTH, LCD_HEIGHT, LCD_WIDTH, color::Color};

pub struct GbaLcd {
    pixels: [[Color; LCD_WIDTH]; LCD_HEIGHT],
}

impl Default for GbaLcd {
    #[allow(clippy::large_stack_arrays)] // LCD framebuffer is inherently large
    fn default() -> Self {
        Self {
            pixels: [[Color::default(); LCD_WIDTH]; LCD_HEIGHT],
        }
    }
}

impl GbaLcd {
    pub const fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        // pixels is [HEIGHT][WIDTH], so the row is y and the column is x.
        self.pixels[y][x] = color;
    }

    pub const fn set_gbc_pixel(&mut self, x: usize, y: usize, color: Color) {
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
        &self.pixels[y][x]
    }
}

impl std::ops::IndexMut<(usize, usize)> for GbaLcd {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        assert!(x < LCD_WIDTH && y < LCD_HEIGHT);
        &mut self.pixels[y][x]
    }
}

#[cfg(test)]
mod tests {
    use super::{Color, GbaLcd};

    #[test]
    fn set_pixel_handles_wide_x_and_round_trips() {
        let mut lcd = GbaLcd::default();
        // x up to 239 previously indexed a 160-row array and panicked.
        lcd.set_pixel(200, 100, Color(0x1234));
        assert_eq!(lcd[(200, 100)].0, 0x1234);
        // A different pixel stays untouched.
        assert_eq!(lcd[(0, 0)].0, 0);
    }
}
