//! Background Layer 3 (BG3)
//!
//! BG3's capabilities depend on the video mode:
//!
//! | Mode | BG3 Type | Description                    |
//! |------|----------|--------------------------------|
//! | 0    | Text     | Regular tiled background       |
//! | 2    | Affine   | Rotation/scaling background    |
//!
//! BG3 is not available in modes 1, 3, 4, or 5.
//!
//! # Affine Mode (Mode 2)
//!
//! In mode 2, BG3 functions as an affine background with rotation and scaling.
//! See [`layer_2`](super::layer_2) for detailed affine transformation documentation.
//!
//! BG3 uses its own set of affine registers:
//! - `BG3PA`, `BG3PB`, `BG3PC`, `BG3PD`: Transformation matrix
//! - `BG3X`, `BG3Y`: Reference point

use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;
use crate::cpu::hardware::lcd::PixelInfo;

use super::{render_affine_bg, render_text_bg, AffineBgConfig, Layer, TextBgConfig};
use serde::{Deserialize, Serialize};

/// BG3
///
/// Supports regular tiled mode (mode 0) and affine mode (mode 2).
/// See [module documentation](self) for details.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer3;

// Text mode configuration (mode 0)
impl TextBgConfig for Layer3 {
    fn layer_id(&self) -> u8 {
        3
    }

    fn get_scroll(&self, reg: &Registers) -> (u16, u16) {
        (reg.bg3hofs, reg.bg3vofs)
    }

    fn get_screen_size(&self, reg: &Registers) -> (usize, usize) {
        reg.get_bg3_screen_size()
    }

    fn get_screen_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg3_screen_base_block()
    }

    fn get_char_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg3_character_base_block()
    }

    fn get_color_mode(&self, reg: &Registers) -> bool {
        reg.get_bg3_color_mode()
    }

    fn get_priority(&self, reg: &Registers) -> u8 {
        reg.get_bg3_priority()
    }
}

// Affine mode configuration (mode 2)
impl AffineBgConfig for Layer3 {
    fn layer_id(&self) -> u8 {
        3
    }

    #[allow(clippy::cast_possible_wrap)]
    fn get_affine_params(&self, reg: &Registers) -> (i16, i16, i16, i16) {
        (
            reg.bg3pa as i16,
            reg.bg3pb as i16,
            reg.bg3pc as i16,
            reg.bg3pd as i16,
        )
    }

    #[allow(clippy::cast_possible_wrap)]
    fn get_reference_point(&self, reg: &Registers) -> (i32, i32) {
        (reg.bg3x as i32, reg.bg3y as i32)
    }

    fn get_bg_control(&self, reg: &Registers) -> u16 {
        reg.bg3cnt
    }
}

impl Layer for Layer3 {
    fn layer_id(&self) -> u8 {
        3
    }

    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        match registers.get_bg_mode() {
            0 => render_text_bg(self, x, y, memory, registers),
            2 => render_affine_bg(self, x, y, memory, registers),
            _ => None,
        }
    }
}
