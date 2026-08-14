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

#[derive(Serialize, Deserialize)]
pub struct Keypad {
    pub key_input: u16,
    pub key_interrupt_control: u16,
}

impl Default for Keypad {
    /// Default keypad state: all buttons released (all bits set to 1).
    fn default() -> Self {
        Self::new()
    }
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

    /// Whether the keypad interrupt condition currently holds. KEYCNT bit 14
    /// enables the interrupt, bits 0-9 select the keys, and bit 15 chooses the
    /// condition: 0 means any selected key is pressed, 1 means all of them are.
    /// `key_input` is active low, so a pressed key reads as 0.
    #[must_use]
    pub const fn irq_condition_met(&self) -> bool {
        if self.key_interrupt_control & (1 << 14) == 0 {
            return false;
        }
        let selected = self.key_interrupt_control & 0x03FF;
        if selected == 0 {
            return false;
        }
        let pressed = !self.key_input & 0x03FF;
        if self.key_interrupt_control & (1 << 15) == 0 {
            pressed & selected != 0
        } else {
            pressed & selected == selected
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

#[cfg(test)]
mod tests {
    use super::{GbaButton, Keypad};

    #[test]
    fn no_irq_when_disabled() {
        let mut kp = Keypad::new();
        // Select A and press it, but leave the enable bit (14) off.
        kp.key_interrupt_control = GbaButton::A as u16;
        kp.set_button(GbaButton::A, true);
        assert!(!kp.irq_condition_met());
    }

    #[test]
    fn or_condition_fires_on_any_selected_key() {
        let mut kp = Keypad::new();
        // Enable IRQ, OR mode, select A and B.
        kp.key_interrupt_control = (1 << 14) | GbaButton::A as u16 | GbaButton::B as u16;
        assert!(!kp.irq_condition_met());
        kp.set_button(GbaButton::B, true);
        assert!(kp.irq_condition_met());
    }

    #[test]
    fn and_condition_needs_all_selected_keys() {
        let mut kp = Keypad::new();
        // Enable IRQ, AND mode (bit 15), select A and B.
        kp.key_interrupt_control =
            (1 << 15) | (1 << 14) | GbaButton::A as u16 | GbaButton::B as u16;
        kp.set_button(GbaButton::A, true);
        assert!(!kp.irq_condition_met(), "only one of two keys held");
        kp.set_button(GbaButton::B, true);
        assert!(kp.irq_condition_met(), "both selected keys held");
    }
}
