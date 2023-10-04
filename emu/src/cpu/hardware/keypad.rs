use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Keypad {
    pub key_input: u16,
    pub key_interrupt_control: u16,
}
