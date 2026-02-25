#![allow(clippy::cast_possible_truncation)]

//! LCD controller (PPU) - handles display rendering.
//!
//! The GBA LCD is 240x160 pixels, 15-bit color (32,768 colors). The [`Lcd`] struct
//! implements the Picture Processing Unit (PPU) that renders backgrounds and sprites to
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
    /// Layer index for tie-breaking: 0-3 for BG0-BG3, 4 for OBJ.
    /// Lower values are drawn on top when priorities are equal.
    /// OBJ (4) is treated specially, it wins ties with BGs of same priority.
    layer: u8,
}

#[serde_as]
#[allow(clippy::large_stack_frames)]
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

#[derive(Default)]
pub struct LcdStepOutput {
    pub request_vblank_irq: bool,
    pub request_hblank_irq: bool,
    pub request_vcount_irq: bool,
}

impl Lcd {
    pub fn step(&mut self) -> LcdStepOutput {
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
                // Determine which layers are enabled for this pixel based on window state
                let window_enables = self.get_window_layer_enables(pixel_x as u8, pixel_y as u8);

                // We get the enabled layers (depending on BG mode and registers), we call render on them
                // we filter out the `None` and we sort by priority.
                let mut layers_with_pixel = self
                    .get_enabled_layers()
                    .into_iter()
                    .filter_map(|layer| {
                        // Check if this layer is enabled by window
                        let is_visible = match layer.layer_id() {
                            0 => window_enables.0, // BG0
                            1 => window_enables.1, // BG1
                            2 => window_enables.2, // BG2
                            3 => window_enables.3, // BG3
                            4 => window_enables.4, // OBJ
                            _ => true,
                        };

                        if !is_visible {
                            return None;
                        }

                        layer.render(
                            pixel_x as usize,
                            pixel_y as usize,
                            &self.memory,
                            &self.registers,
                        )
                    })
                    .collect::<Vec<PixelInfo>>();

                // Sort by: (1) priority ascending, (2) OBJ before BGs at same priority,
                // (3) lower BG number first for BGs with equal priority.
                // OBJ layer is 4, BG layers are 0-3. For tie-breaking at same priority:
                // - OBJ (layer 4) should appear BEFORE backgrounds
                // - Lower BG numbers should appear before higher BG numbers
                // We achieve this by mapping: OBJ(4)->0, BG0(0)->1, BG1(1)->2, etc.
                layers_with_pixel.sort_unstable_by_key(|pixel| {
                    let layer_order = if pixel.layer == 4 { 0 } else { pixel.layer + 1 };
                    (pixel.priority, layer_order)
                });

                // If no layer renders a pixel, use the backdrop color (palette index 0 of BG palette)
                let backdrop_color = Color::from_palette_color(u16::from_le_bytes([
                    self.memory.bg_palette_ram[0],
                    self.memory.bg_palette_ram[1],
                ]));

                // Get the top pixel (or backdrop if none)
                let (top_color, top_layer) = layers_with_pixel
                    .first()
                    .map_or((backdrop_color, 5_u8), |info| (info.color, info.layer)); // 5 = backdrop

                // Apply blending effects if enabled for this window region
                let effects_enabled = window_enables.5;
                let final_color = if effects_enabled {
                    self.apply_blend_effect(
                        top_color,
                        top_layer,
                        &layers_with_pixel,
                        backdrop_color,
                    )
                } else {
                    top_color
                };

                self.buffer[pixel_y as usize][pixel_x as usize] = final_color;
            }
        }

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

    /// Apply color blending effects based on BLDCNT settings.
    fn apply_blend_effect(
        &self,
        top_color: Color,
        top_layer: u8,
        layers: &[PixelInfo],
        backdrop_color: Color,
    ) -> Color {
        let blend_mode = self.registers.get_blend_mode();

        // Mode 0 = no blending
        if blend_mode == 0 {
            return top_color;
        }

        let target1 = self.registers.get_blend_target1();

        // Check if top layer is a first target
        let is_target1 = match top_layer {
            0 => target1.0, // BG0
            1 => target1.1, // BG1
            2 => target1.2, // BG2
            3 => target1.3, // BG3
            4 => target1.4, // OBJ
            5 => target1.5, // Backdrop
            _ => false,
        };

        if !is_target1 {
            return top_color;
        }

        match blend_mode {
            1 => {
                // Alpha blending - blend top with second target below it
                let target2 = self.registers.get_blend_target2();

                // Find the second layer that is a target2
                let second_layer = layers.iter().skip(1).find(|p| match p.layer {
                    0 => target2.0,
                    1 => target2.1,
                    2 => target2.2,
                    3 => target2.3,
                    4 => target2.4,
                    _ => false,
                });

                let second_color = if let Some(layer) = second_layer {
                    layer.color
                } else if target2.5 {
                    // Backdrop is target2
                    backdrop_color
                } else {
                    // No valid second target found
                    return top_color;
                };

                let (eva, evb) = self.registers.get_blend_alpha();
                Self::alpha_blend(top_color, second_color, eva, evb)
            }
            2 => {
                // Brightness increase (fade to white)
                let evy = self.registers.get_blend_brightness();
                Self::brightness_increase(top_color, evy)
            }
            3 => {
                // Brightness decrease (fade to black)
                let evy = self.registers.get_blend_brightness();
                Self::brightness_decrease(top_color, evy)
            }
            _ => top_color,
        }
    }

    /// Alpha blend two colors: result = (color1 * eva + color2 * evb) / 16
    fn alpha_blend(color1: Color, color2: Color, eva: u8, evb: u8) -> Color {
        let blend_component = |c1: u8, c2: u8| -> u8 {
            let result = (u16::from(c1) * u16::from(eva) + u16::from(c2) * u16::from(evb)) / 16;
            result.min(31) as u8
        };

        Color::from_rgb(
            blend_component(color1.red(), color2.red()),
            blend_component(color1.green(), color2.green()),
            blend_component(color1.blue(), color2.blue()),
        )
    }

    /// Brightness increase (fade to white): result = color + (31 - color) * evy / 16
    fn brightness_increase(color: Color, evy: u8) -> Color {
        let brighten = |c: u8| -> u8 {
            let result = u16::from(c) + (u16::from(31 - c) * u16::from(evy)) / 16;
            result.min(31) as u8
        };

        Color::from_rgb(
            brighten(color.red()),
            brighten(color.green()),
            brighten(color.blue()),
        )
    }

    /// Brightness decrease (fade to black): result = color - color * evy / 16
    fn brightness_decrease(color: Color, evy: u8) -> Color {
        let darken = |c: u8| -> u8 {
            let result = u16::from(c) - (u16::from(c) * u16::from(evy)) / 16;
            result as u8
        };

        Color::from_rgb(
            darken(color.red()),
            darken(color.green()),
            darken(color.blue()),
        )
    }

    /// Determine which layers are enabled at this pixel based on window settings.
    /// Returns (bg0, bg1, bg2, bg3, obj, effects) enable flags.
    fn get_window_layer_enables(&self, x: u8, y: u8) -> (bool, bool, bool, bool, bool, bool) {
        let win0_enabled = self.registers.get_win0_enabled();
        let win1_enabled = self.registers.get_win1_enabled();
        let winobj_enabled = self.registers.get_winobj_enabled();

        // If no windows are enabled, all layers are visible everywhere
        if !win0_enabled && !win1_enabled && !winobj_enabled {
            return (true, true, true, true, true, true);
        }

        // Check which window the pixel is in (priority: WIN0 > WIN1 > WINOBJ > WINOUT)
        if win0_enabled && self.registers.is_in_win0(x, y) {
            return self.registers.get_win0_enables();
        }

        if win1_enabled && self.registers.is_in_win1(x, y) {
            return self.registers.get_win1_enables();
        }

        // TODO: Check WINOBJ (requires checking if pixel is covered by a window-type sprite)
        // For now, skip WINOBJ check

        self.registers.get_winout_enables()
    }
}
