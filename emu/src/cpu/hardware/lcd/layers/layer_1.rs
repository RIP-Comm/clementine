//! # GBA Background Layer 1 (BG1)
//!
//! This module implements BG1, one of the four background layers available on the
//! Game Boy Advance. BG1 is a **regular (text) background** layer, identical in
//! functionality to BG0.
//!
//! ## Availability
//!
//! BG1 is available in the following video modes:
//! - **Mode 0**: All four BG layers (0-3) are regular tiled backgrounds
//! - **Mode 1**: BG0 and BG1 are regular, BG2 is affine, BG3 is disabled
//!
//! In modes 2-5 (affine and bitmap modes), BG1 is not available.
//!
//! See [`layer_0`](super::layer_0) for detailed documentation on how regular
//! tiled backgrounds work.

use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;

use super::{Layer, TextBgConfig, render_text_bg};
use serde::{Deserialize, Serialize};

/// BG1
///
/// A regular (non-affine) tiled background layer available in video modes 0 and 1.
/// Supports scrolling, tile flipping, and both 4bpp and 8bpp color modes.
///
/// See [`layer_0::Layer0`](super::layer_0::Layer0) for detailed documentation.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer1;

impl TextBgConfig for Layer1 {
    fn layer_id(&self) -> u8 {
        1
    }

    fn get_scroll(&self, reg: &Registers) -> (u16, u16) {
        (reg.bg1hofs, reg.bg1vofs)
    }

    fn get_screen_size(&self, reg: &Registers) -> (usize, usize) {
        reg.get_bg1_screen_size()
    }

    fn get_screen_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg1_screen_base_block()
    }

    fn get_char_base_block(&self, reg: &Registers) -> u8 {
        reg.get_bg1_character_base_block()
    }

    fn get_color_mode(&self, reg: &Registers) -> bool {
        reg.get_bg1_color_mode()
    }

    fn get_priority(&self, reg: &Registers) -> u8 {
        reg.get_bg1_priority()
    }
}

impl Layer for Layer1 {
    fn layer_id(&self) -> u8 {
        1
    }

    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        render_text_bg(self, x, y, memory, registers)
    }
}
