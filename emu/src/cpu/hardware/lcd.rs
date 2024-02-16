use logger::log;
use serde::Deserialize;
use serde::Serialize;
use serde_with::serde_as;

use crate::bitwise::Bits;
use crate::cpu::hardware::lcd::layers::Layer;

use self::layers::layer_0::Layer0;
use self::layers::layer_1::Layer1;
use self::layers::layer_2::Layer2;
use self::layers::layer_3::Layer3;
use self::layers::layer_obj::LayerObj;
use self::memory::Memory;
use self::registers::Registers;

mod layers;
mod memory;
mod object_attributes;
mod point;
mod registers;

/// GBA display width
const LCD_WIDTH: usize = 240;

/// GBA display height
const LCD_HEIGHT: usize = 160;

// Sprites are positioned inside a 512x256 size (x position is 9 bits and y position is 8 bits)
/// World height
const WORLD_HEIGHT: u16 = 256;

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct Color(pub u16);

impl Color {
    pub const fn from_palette_color(value: u16) -> Self {
        Self(value)
    }

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

#[derive(Serialize, Deserialize)]
enum ObjMappingKind {
    TwoDimensional,
    OneDimensional,
}

impl From<bool> for ObjMappingKind {
    fn from(value: bool) -> Self {
        match value {
            false => Self::TwoDimensional,
            true => Self::OneDimensional,
        }
    }
}

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
struct PixelInfo {
    color: Color,
    priority: u8,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct Lcd {
    pub(crate) registers: Registers,
    pub(crate) memory: Memory,

    #[serde_as(as = "[[_; 240]; 160]")]
    pub buffer: [[Color; LCD_WIDTH]; LCD_HEIGHT],

    pixel_index: u32,
    should_draw: bool,

    layer_0: Layer0,
    layer_1: Layer1,
    layer_2: Layer2,
    layer_3: Layer3,
    layer_obj: LayerObj,
}

impl Default for Lcd {
    fn default() -> Self {
        Self {
            registers: Registers::default(),
            memory: Memory::default(),
            pixel_index: 0,
            buffer: [[Color::default(); LCD_WIDTH]; LCD_HEIGHT],
            should_draw: false,
            layer_0: Layer0,
            layer_1: Layer1,
            layer_2: Layer2,
            layer_3: Layer3,
            layer_obj: LayerObj::default(),
        }
    }
}
#[derive(Default)]
pub struct LcdStepOutput {
    pub request_vblank_irq: bool,
    pub request_hblank_irq: bool,
    pub request_vcount_irq: bool,
}

impl Lcd {
    pub fn step(&mut self) -> LcdStepOutput {
        // This will be much more complex obviously
        let mut output = LcdStepOutput::default();

        if self.registers.vcount < 160 {
            // We either are in Vdraw or Hblank
            if self.pixel_index == 0 {
                // We're drawing the first pixel of the scanline, we're entering Vdraw

                self.registers.set_hblank_flag(false);
                self.registers.set_vblank_flag(false);

                self.should_draw = true;

                // Cache attributes and scanline
                self.layer_obj
                    .handle_enter_vdraw(&self.memory, &self.registers);
            } else if self.pixel_index == 240 {
                // We're entering Hblank

                self.registers.set_hblank_flag(true);

                if self.registers.get_hblank_irq_enable() {
                    output.request_hblank_irq = true;
                }

                self.should_draw = false;
            }
        } else if self.registers.vcount == 160 && self.pixel_index == 0 {
            // We're drawing the first pixel of the Vblank period

            self.registers.set_vblank_flag(true);

            if self.registers.get_vblank_irq_enable() {
                output.request_vblank_irq = true;
            }

            self.should_draw = false;
        }

        if self.should_draw {
            let pixel_y = self.registers.vcount;
            let pixel_x = self.pixel_index;

            self.buffer[pixel_y as usize][pixel_x as usize] = self
                .layer_obj
                .render(pixel_x as usize, pixel_y as usize)
                .unwrap_or_else(|| Color::from_rgb(31, 31, 31));
        }

        log(format!(
            "mode: {:?}, BG2: {:?} BG3: {:?}, OBJ: {:?}, WIN0: {:?}, WIN1: {:?}, WINOJB: {:?}",
            self.registers.get_bg_mode(),
            self.registers.get_bg2_enabled(),
            self.registers.get_bg3_enabled(),
            self.registers.get_obj_enabled(),
            self.registers.get_win0_enabled(),
            self.registers.get_win1_enabled(),
            self.registers.get_winobj_enabled(),
        ));

        self.pixel_index += 1;

        if self.pixel_index == 308 {
            // We finished to draw the scanline
            self.pixel_index = 0;
            self.registers.vcount += 1;

            // We finished to draw the screen
            if self.registers.vcount == 228 {
                self.registers.vcount = 0;
            }
        }

        self.registers.set_vcounter_flag(false);

        if self.registers.vcount.get_byte(0) == self.registers.get_vcount_setting() {
            self.registers.set_vcounter_flag(true);

            if self.registers.get_vcounter_irq_enable() {
                output.request_vcount_irq = true;
            }
        }

        output
    }
}
