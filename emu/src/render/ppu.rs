use macros::acquire_lock;
use std::sync::{Arc, Mutex};

#[cfg(feature = "debug")]
use rand::Rng;

#[cfg(feature = "debug")]
use crate::render::color::colors;

use crate::{
    memory::internal_memory::InternalMemory,
    render::{
        color::{Color, PaletteType},
        gba_lcd::GbaLcd,
        GBC_LCD_HEIGHT, GBC_LCD_WIDTH, LCD_HEIGHT, LCD_WIDTH, MAX_COLORS_FULL_PALETTE,
        MAX_COLORS_SINGLE_PALETTE, MAX_PALETTES_BY_TYPE,
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
        #[allow(unused_assignments)]
        let mut bg_mode =
            acquire_lock!(self.internal_memory, memory => { memory.lcd_registers.get_bg_mode() });

        // BG_MODE_3 forced for now.
        bg_mode = 3;

        #[cfg(feature = "mode_3")]
        {
            bg_mode = 3;
        }

        #[cfg(feature = "mode_4")]
        {
            bg_mode = 4;
        }

        #[cfg(feature = "mode_5")]
        {
            bg_mode = 5;
        }

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
                acquire_lock!(self.internal_memory, memory => {
                    // Bitmap mode
                    for y in 0..LCD_HEIGHT {
                        for x in 0..LCD_WIDTH {
                            let index_color = (y * LCD_WIDTH + x) * 2;

                            let color_array = [
                                memory.video_ram[index_color],
                                memory.video_ram[index_color + 1],
                            ];

                            let color: Color = [color_array[0], color_array[1]].into();

                            acquire_lock!(self.gba_lcd, gba_lcd => { gba_lcd.set_pixel(x, y, color); })
                        }
                    }
                });
            }
            4 => {
                // 06000000-06009FFF for Frame 0
                // 0600A000-06013FFF for Frame 1
                let selected_frame = acquire_lock!(self.internal_memory, memory => {
                    memory.lcd_registers.get_frame_select()
                });

                for y in 0..LCD_HEIGHT {
                    for x in 0..LCD_WIDTH {
                        let index_mem =
                            (y * LCD_WIDTH + x) + (selected_frame * LCD_HEIGHT * LCD_WIDTH);

                        let index_palette = acquire_lock!(self.internal_memory, memory => {
                            memory.video_ram[index_mem]
                        });

                        let color: Color = self
                            .get_color_from_full_palette(index_palette.into(), &PaletteType::BG);

                        acquire_lock!(self.gba_lcd, gba_lcd => { gba_lcd.set_pixel(x, y, color); })
                    }
                }
            }
            5 => {
                // 06000000-06009FFF for Frame 0
                // 0600A000-06013FFF for Frame 1
                let selected_frame = match self.internal_memory.lock() {
                    Ok(memory) => memory.lcd_registers.get_frame_select(),
                    _ => 0,
                };

                acquire_lock!(self.internal_memory, memory => {
                // Bitmap mode
                    for y in 0..GBC_LCD_HEIGHT {
                        for x in 0..GBC_LCD_WIDTH {
                            let index_color = (y * GBC_LCD_WIDTH + x) * 2
                                + (selected_frame * GBC_LCD_HEIGHT * GBC_LCD_WIDTH);

                            let color_array =
                                [
                                    memory.video_ram[index_color],
                                    memory.video_ram[index_color + 1],
                                ];

                            let color: Color = [color_array[0], color_array[1]].into();

                            acquire_lock!(self.gba_lcd, gba_lcd => { gba_lcd.set_gbc_pixel(x, y, color); })
                        }
                    }
                });
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

    #[cfg(feature = "debug")]
    pub fn load_default_palette(&mut self) {
        acquire_lock!(self.internal_memory, memory => {

            let red_array: [u8; 2] = colors::RED.into();
            let green_array: [u8; 2] = colors::GREEN.into();

            let palette_index_top_half: u8 = 10;
            let palette_index_bottom_half: u8 = 5;

            memory.bg_palette_ram[palette_index_top_half as usize * 2] = red_array[0];
            memory.bg_palette_ram[palette_index_top_half as usize * 2 + 1] = red_array[1];

            memory.bg_palette_ram[palette_index_bottom_half as usize * 2] = green_array[0];
            memory.bg_palette_ram[palette_index_bottom_half as usize * 2 + 1] = green_array[1];

            let mut counter = 0;
            for _i in 0..LCD_WIDTH * LCD_HEIGHT / 2 {
                memory.video_ram[counter] = palette_index_top_half;
                counter += 1;
            }

            for _i in 0..LCD_WIDTH * LCD_HEIGHT / 2 {
                memory.video_ram[counter] = palette_index_bottom_half;
                counter += 1;
            }
        });
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
