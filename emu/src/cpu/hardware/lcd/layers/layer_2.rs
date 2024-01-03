use super::Layer;
use serde::Deserialize;
use serde::Serialize;

#[derive(Default, Serialize, Deserialize)]
pub struct Layer2;

impl Layer for Layer2 {
    #[allow(unused_variables)]
    fn render(&self, x: usize, y: usize) -> Option<crate::cpu::hardware::lcd::Color> {
        // TODO: To implement
        None
    }
}
