use crate::cpu::hardware::lcd::PixelInfo;
use crate::cpu::hardware::lcd::memory::Memory;
use crate::cpu::hardware::lcd::registers::Registers;

use super::Layer;
use serde::Deserialize;
use serde::Serialize;

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
