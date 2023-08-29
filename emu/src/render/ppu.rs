use std::sync::{Arc, Mutex};

#[cfg(feature = "debug")]
use crate::render::GBC_LCD_WIDTH;
#[cfg(feature = "debug")]
use rand::Rng;

use crate::{
    cpu::arm7tdmi::Arm7tdmi,
    render::{
        // color::{Color, PaletteType},
        gba_lcd::GbaLcd,
        // LCD_HEIGHT, LCD_WIDTH,
        MAX_COLORS_FULL_PALETTE,
        MAX_COLORS_SINGLE_PALETTE,
        MAX_PALETTES_BY_TYPE,
    },
};

use super::color::{Color, PaletteType};

#[derive(Default)]
pub struct PixelProcessUnit {
    cpu: Arc<Mutex<Arm7tdmi>>,
    #[allow(dead_code)]
    gba_lcd: Arc<Mutex<Box<GbaLcd>>>,
}

impl PixelProcessUnit {
    pub fn new(gba_lcd: Arc<Mutex<Box<GbaLcd>>>, cpu: Arc<Mutex<Arm7tdmi>>) -> Self {
        Self { cpu, gba_lcd }
    }

    fn get_array_color(&self, index: usize, palette_type: &PaletteType) -> [u8; 2] {
        debug_assert!(index % 2 == 0);
        let lcd = &self.cpu.lock().unwrap().bus.lcd;
        match palette_type {
            PaletteType::BG => [lcd.bg_palette_ram[index], lcd.bg_palette_ram[index + 1]],
            PaletteType::OBJ => [lcd.obj_palette_ram[index], lcd.obj_palette_ram[index + 1]],
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
                palettes[palette_index].push(self.get_color_from_single_palette(
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
        let mut memory = self.internal_memory.lock().unwrap();

        for i in 0..200 {
            let mut rng = rand::thread_rng();
            let mut value: u8 = rng.gen();
            memory.bg_palette_ram[i] = value;
            value = rng.gen();
            memory.obj_palette_ram[i] = value;
        }
    }

    #[cfg(feature = "debug")]
    pub fn load_centered_bitmap(&mut self, data: Vec<Color>, width: usize, height: usize) {
        let mut memory = self.internal_memory.lock().unwrap();

        let centered_width = (LCD_WIDTH - width) / 2;
        let mut row = (LCD_HEIGHT - height) / 2;
        for i in 0..data.len() {
            let array_color: [u8; 2] = data[i].into();
            if i % width == 0 {
                row += 1;
            }

            let color_index = ((i % width + centered_width) + (row * LCD_WIDTH)) * 2;
            memory.video_ram[color_index] = array_color[0];
            memory.video_ram[color_index + 1] = array_color[1];
        }
    }

    #[cfg(feature = "debug")]
    pub fn load_gbc_bitmap(&mut self, data: Vec<Color>, width: usize, height: usize) {
        let mut memory = self.internal_memory.lock().unwrap();

        let mut row = 0;
        for i in 0..data.len() {
            let array_color: [u8; 2] = data[i].into();
            if i % width == 0 {
                row += 1;
            }

            let color_index = ((i % width) + (row * GBC_LCD_WIDTH)) * 2;
            memory.video_ram[color_index] = array_color[0];
            memory.video_ram[color_index + 1] = array_color[1];
        }
    }
}
