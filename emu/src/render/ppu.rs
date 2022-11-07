use std::sync::{Arc, Mutex};

#[cfg(feature = "debug")]
use rand::Rng;

use crate::{
    memory::internal_memory::InternalMemory,
    render::{
        color::{Color, PaletteType},
        gba_lcd::GbaLcd,
        DISPLAY_HEIGHT, DISPLAY_WIDTH, MAX_COLORS_FULL_PALETTE, MAX_COLORS_SINGLE_PALETTE,
        MAX_PALETTES_BY_TYPE,
    },
};

#[derive(Default)]
pub struct PixelProcessUnit {
    internal_memory: Arc<Mutex<InternalMemory>>,
    gba_lcd: Arc<Mutex<Box<GbaLcd>>>,
}

impl PixelProcessUnit {
    pub fn new(
        gba_lcd: Arc<Mutex<Box<GbaLcd>>>,
        internal_memory: Arc<Mutex<InternalMemory>>,
    ) -> Self {
        Self {
            gba_lcd,
            internal_memory,
        }
    }

    pub fn render(&self) {
        // BG_MODE is forced to 3 to avoid crash on start
        let bg_mode = 3;

        //let bg_mode = self.internal_memory.borrow().lcd_registers.get_bg_mode();
        match bg_mode {
            0 => {
                todo!("BG_MODE 0 not implemented yet")
            }
            1 => {
                todo!("BG_MODE 1 not implemented yet")
            }
            2 => {
                todo!("BG_MODE 2 not implemented yet")
            }
            3 => {
                // Bitmap mode
                let memory = self.internal_memory.lock().unwrap();
                let mut gba_lcd = self.gba_lcd.lock().unwrap();

                for y in 0..DISPLAY_HEIGHT {
                    for x in 0..DISPLAY_WIDTH {
                        let color: Color = [
                            memory.video_ram[(y * DISPLAY_WIDTH + x) * 2],
                            memory.video_ram[(y * DISPLAY_WIDTH + x) * 2 + 1],
                        ]
                        .into();

                        gba_lcd.set_pixel(x, y, color);
                    }
                }
            }
            4 => {
                todo!("BG_MODE 4 not implemented yet")
            }
            5 => {
                todo!("BG_MODE 5 not implemented yet")
            }
            _ => panic!("BG MODE doesn't exist."),
        }
    }

    fn get_array_color(&self, index: usize, palette_type: &PaletteType) -> [u8; 2] {
        debug_assert!(index % 2 == 0);
        let memory = self.internal_memory.lock().unwrap();
        match palette_type {
            PaletteType::BG => [
                memory.bg_palette_ram[index],
                memory.bg_palette_ram[index + 1],
            ],
            PaletteType::OBJ => [
                memory.obj_palette_ram[index],
                memory.obj_palette_ram[index + 1],
            ],
        }
    }

    fn get_color_from_full_palette(&self, color_index: usize, palette_type: &PaletteType) -> Color {
        debug_assert!(color_index < MAX_COLORS_FULL_PALETTE);
        self.get_array_color(color_index * 2, palette_type).into()
    }

    fn get_color_from_single_palette(
        &self,
        palette_index: usize,
        color_index: usize,
        palette_type: &PaletteType,
    ) -> Color {
        debug_assert!(palette_index < MAX_PALETTES_BY_TYPE);
        debug_assert!(color_index < MAX_COLORS_FULL_PALETTE);

        self.get_color_from_full_palette(
            (palette_index * MAX_COLORS_SINGLE_PALETTE) + color_index,
            palette_type,
        )
    }

    pub fn get_palettes(&self, palette_type: &PaletteType) -> Vec<Vec<Color>> {
        let mut palettes = vec![];

        for palette_index in 0..MAX_PALETTES_BY_TYPE {
            palettes.push(vec![]);
            for color_index in 0..MAX_COLORS_SINGLE_PALETTE {
                palettes[palette_index as usize].push(self.get_color_from_single_palette(
                    palette_index,
                    color_index,
                    palette_type,
                ));
            }
        }
        palettes
    }

    #[cfg(feature = "debug")]
    pub fn load_random_palettes(&mut self) {
        for i in 0..200 {
            let mut rng = rand::thread_rng();
            let mut value: u8 = rng.gen();
            self.internal_memory.borrow_mut().bg_palette_ram[i] = value;
            value = rng.gen();
            self.internal_memory.borrow_mut().obj_palette_ram[i] = value;
        }
    }

    #[cfg(feature = "debug")]
    pub fn load_bitmap(&mut self, data: Vec<Color>, width: usize, height: usize) {
        let centered_width = (DISPLAY_WIDTH - width) / 2;
        let mut row = (DISPLAY_HEIGHT - height) / 2;
        for i in 0..data.len() {
            let array_color: [u8; 2] = data[i].into();
            if i % width == 0 {
                row += 1;
            }

            let color_index = ((i % width + centered_width) + (row * DISPLAY_WIDTH)) * 2;
            self.internal_memory.borrow_mut().video_ram[color_index] = array_color[0];
            self.internal_memory.borrow_mut().video_ram[color_index + 1] = array_color[1];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PixelProcessUnit;
    use crate::render::color::PaletteType;

    #[test]
    fn test_get_palettes_bg() {
        let ppu = PixelProcessUnit::default();

        let bg_palettes = ppu.get_palettes(&PaletteType::BG);
        assert_eq!(bg_palettes.len(), 16);
        assert_eq!(bg_palettes[0].len(), 16);
    }

    #[test]
    fn test_get_palettes_obj() {
        let ppu = PixelProcessUnit::default();

        let obj_palettes = ppu.get_palettes(&PaletteType::OBJ);
        assert_eq!(obj_palettes.len(), 16);
        assert_eq!(obj_palettes[0].len(), 16);
    }
}
