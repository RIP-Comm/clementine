//! Background Layer 1 (BG1) - Regular tiled background.
//!
//! BG1 is a regular (non-affine) tiled background, identical in functionality to BG0.
//! See [`layer_0`](super::layer_0) for detailed documentation on how regular
//! backgrounds work.
//!
//! # Availability
//!
//! BG1 is available in video modes 0 and 1 only.
//!
//! # Status
//!
//! **Not yet implemented** - currently returns `None` for all pixels.

use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;

use super::Layer;
use serde::Deserialize;
use serde::Serialize;

/// BG1 - Background Layer 1 (not yet implemented).
///
/// See [`layer_0::Layer0`](super::layer_0::Layer0) for how regular backgrounds work.
#[derive(Default, Serialize, Deserialize)]
pub struct Layer1;

impl Layer for Layer1 {
    #[allow(unused_variables)]
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo> {
        // TODO: To implement
        None
    }
}
