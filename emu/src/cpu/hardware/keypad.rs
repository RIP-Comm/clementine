use serde::{Deserialize, Serialize};

/// GBA button bit positions in KEYINPUT register (when pressed are set to 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbaButton {
    A = 1 << 0,
    B = 1 << 1,
    Select = 1 << 2,
    Start = 1 << 3,
    Right = 1 << 4,
    Left = 1 << 5,
    Up = 1 << 6,
    Down = 1 << 7,
    R = 1 << 8,
    L = 1 << 9,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Keypad {
    pub key_input: u16,
    pub key_interrupt_control: u16,
}

impl Keypad {
    /// Create a new Keypad with all buttons released (all bits set to 1).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            key_input: 0x03FF, // All 10 buttons released (bits 0-9 = 1)
            key_interrupt_control: 0,
        }
    }

    /// Set button state: pressed = true, released = false.
    /// GBA uses active-low logic: bit 0 = pressed, bit 1 = released.
    pub const fn set_button(&mut self, button: GbaButton, pressed: bool) {
        if pressed {
            // Press: clear the bit (set to 0)
            self.key_input &= !(button as u16);
        } else {
            // Release: set the bit (set to 1)
            self.key_input |= button as u16;
        }
    }
}
