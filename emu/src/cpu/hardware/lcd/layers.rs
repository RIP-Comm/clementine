use super::{memory::Memory, registers::Registers, PixelInfo};

pub mod layer_0;
pub mod layer_1;
pub mod layer_2;
pub mod layer_3;
pub mod layer_obj;

pub trait Layer {
    fn render(
        &self,
        x: usize,
        y: usize,
        memory: &Memory,
        registers: &Registers,
    ) -> Option<PixelInfo>;
}
