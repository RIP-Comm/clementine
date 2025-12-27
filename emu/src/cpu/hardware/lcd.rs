//! LCD controller (PPU) - handles display rendering.
//!
//! The GBA LCD is 240x160 pixels, 15-bit color (32,768 colors). The [`Lcd`] struct
//! implements the Picture Processing Unit that renders backgrounds and sprites to
//! a framebuffer.
//!
//! # Display Timing
//!
//! The LCD renders one pixel every 4 CPU cycles. A complete frame consists of:
//!
//! ```text
//!                    240 pixels          68 pixels
//!                   ◄──────────►       ◄──────────►
//!               ┌─────────────────────────────────────┐
//!               │                      │              │
//!    160 lines  │      Visible         │   HBlank    │ VDraw
//!               │      (VDraw)         │             │
//!               ├──────────────────────┼─────────────┤
//!     68 lines  │                VBlank              │ VBlank
//!               └─────────────────────────────────────┘
//!
//! - VDraw: Lines 0-159, pixels 0-239 are visible
//! - HBlank: Pixels 240-307 on each line
//! - VBlank: Lines 160-227
//! - Total: 228 lines × 308 pixels = 280,896 cycles/frame ≈ 59.73 Hz
//! ```
//!
//! # Background Modes
//!
//! The DISPCNT register (bits 0-2) selects the background mode:
//!
//! | Mode | BG0    | BG1    | BG2      | BG3      | Description           |
//! |------|--------|--------|----------|----------|-----------------------|
//! | 0    | Text   | Text   | Text     | Text     | 4 text backgrounds    |
//! | 1    | Text   | Text   | Affine   | -        | 2 text + 1 affine     |
//! | 2    | -      | -      | Affine   | Affine   | 2 affine backgrounds  |
//! | 3    | -      | -      | Bitmap   | -        | 240x160 15-bit bitmap |
//! | 4    | -      | -      | Bitmap   | -        | 240x160 8-bit indexed |
//! | 5    | -      | -      | Bitmap   | -        | 160x128 15-bit bitmap |
//!
//! # Layer Priority
//!
//! Each background and OBJ layer has a priority (0-3, lower = higher priority).
//! The [`Lcd::step`] method renders all enabled layers and composites them by priority.
//!
//! # Memory Regions
//!
//! The LCD controller owns several memory regions (in `Memory`):
//! - **Palette RAM** (`0x0500_0000`): 512 bytes for BG + 512 bytes for OBJ colors
//! - **VRAM** (`0x0600_0000`): 96KB for tile data and bitmaps
//! - **OAM** (`0x0700_0000`): 1KB for 128 sprite attributes
//!
//! # Interrupts
//!
//! The LCD can generate three types of interrupts (via [`LcdStepOutput`]):
//! - **V-Blank**: When entering vertical blank period (line 160)
//! - **H-Blank**: When entering horizontal blank period (pixel 240 of each visible line)
//! - **V-Count**: When the current line matches the V-Count setting in DISPSTAT
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
    #[must_use]
    pub const fn from_palette_color(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        let red: u16 = red.into();
        let green: u16 = green.into();
        let blue: u16 = blue.into();

        Self((blue << 10) + (green << 5) + red)
    }

    #[must_use]
    pub fn red(&self) -> u8 {
        self.0.get_bits(0..=4) as u8
    }

    #[must_use]
    pub fn green(&self) -> u8 {
        self.0.get_bits(5..=9) as u8
    }

    #[must_use]
    pub fn blue(&self) -> u8 {
        self.0.get_bits(10..=14) as u8
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum ObjMappingKind {
    TwoDimensional,
    OneDimensional,
}

impl From<bool> for ObjMappingKind {
    fn from(value: bool) -> Self {
        if value {
            Self::OneDimensional
        } else {
            Self::TwoDimensional
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
    #[allow(clippy::large_stack_arrays)]
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

#[allow(clippy::module_name_repetitions)]
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

            // Check forced blank (bit 7 of DISPCNT)
            // When set, display white screen regardless of layer settings
            if self.registers.dispcnt.get_bit(7) {
                self.buffer[pixel_y as usize][pixel_x as usize] = Color::from_rgb(31, 31, 31);
            } else {
                // We get the enabled layers (depending on BG mode and registers), we call render on them
                // we filter out the `None` and we sort by priority.
                let mut layers_with_pixel = self
                    .get_enabled_layers()
                    .into_iter()
                    .filter_map(|layer| {
                        layer.render(
                            pixel_x as usize,
                            pixel_y as usize,
                            &self.memory,
                            &self.registers,
                        )
                    })
                    .collect::<Vec<PixelInfo>>();

                layers_with_pixel.sort_unstable_by_key(|pixel| pixel.priority);

                let first_pixel = layers_with_pixel.first();

                // If no layer renders a pixel, use the backdrop color (palette index 0 of BG palette)
                let backdrop_color = Color::from_palette_color(u16::from_le_bytes([
                    self.memory.bg_palette_ram[0],
                    self.memory.bg_palette_ram[1],
                ]));

                self.buffer[pixel_y as usize][pixel_x as usize] =
                    first_pixel.map_or(backdrop_color, |info| info.color);
            }
        }

        // Disabled verbose per-pixel logging
        // log(format!(
        //     "mode: {:?}, BG0: {:?}, BG1: {:?}, BG2: {:?}, BG3: {:?}, OBJ: {:?}, WIN0: {:?}, WIN1: {:?}, WINOJB: {:?}",
        //     self.registers.get_bg_mode(),
        //     self.registers.get_bg0_enabled(),
        //     self.registers.get_bg1_enabled(),
        //     self.registers.get_bg2_enabled(),
        //     self.registers.get_bg3_enabled(),
        //     self.registers.get_obj_enabled(),
        //     self.registers.get_win0_enabled(),
        //     self.registers.get_win1_enabled(),
        //     self.registers.get_winobj_enabled(),
        // ));

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

    fn get_enabled_layers(&self) -> Vec<&dyn Layer> {
        let mut result: Vec<&dyn Layer> = Vec::new();

        let current_mode = self.registers.get_bg_mode();

        if matches!(current_mode, 0 | 1) && self.registers.get_bg0_enabled() {
            result.push(&self.layer_0);
        }

        if matches!(current_mode, 0 | 1) && self.registers.get_bg1_enabled() {
            result.push(&self.layer_1);
        }

        // BG2 is available in every mode
        if self.registers.get_bg2_enabled() {
            result.push(&self.layer_2);
        }

        if matches!(current_mode, 0 | 2) && self.registers.get_bg3_enabled() {
            result.push(&self.layer_3);
        }

        if self.registers.get_obj_enabled() {
            result.push(&self.layer_obj);
        }

        result
    }
}
