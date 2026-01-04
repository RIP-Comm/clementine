use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

/// Timer overflow result indicating which IRQs to request
#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct TimerOverflowResult {
    pub timer0_overflow: bool,
    pub timer1_overflow: bool,
    pub timer2_overflow: bool,
    pub timer3_overflow: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Timers {
    /// Timer 0 Counter (the actual running value read by games)
    pub tm0cnt_l: u16,
    /// Timer 0 Control
    pub tm0cnt_h: u16,
    /// Timer 0 Reload value (written to `tm0cnt_l`, loaded on overflow)
    pub(crate) tm0_reload: u16,
    /// Timer 0 prescaler counter
    tm0_prescaler_counter: u32,

    /// Timer 1 Counter
    pub tm1cnt_l: u16,
    /// Timer 1 Control
    pub tm1cnt_h: u16,
    /// Timer 1 Reload value
    pub(crate) tm1_reload: u16,
    /// Timer 1 prescaler counter
    tm1_prescaler_counter: u32,

    /// Timer 2 Counter
    pub tm2cnt_l: u16,
    /// Timer 2 Control
    pub tm2cnt_h: u16,
    /// Timer 2 Reload value
    pub(crate) tm2_reload: u16,
    /// Timer 2 prescaler counter
    tm2_prescaler_counter: u32,

    /// Timer 3 Counter
    pub tm3cnt_l: u16,
    /// Timer 3 Control
    pub tm3cnt_h: u16,
    /// Timer 3 Reload value
    pub(crate) tm3_reload: u16,
    /// Timer 3 prescaler counter
    tm3_prescaler_counter: u32,
}

impl Timers {
    /// Get prescaler divider from control register bits 0-1
    fn get_prescaler(control: u16) -> u32 {
        match control & 0b11 {
            0 => 1,    // F/1
            1 => 64,   // F/64
            2 => 256,  // F/256
            3 => 1024, // F/1024
            _ => unreachable!(),
        }
    }

    /// Check if timer is enabled (bit 7)
    fn is_enabled(control: u16) -> bool {
        control.get_bit(7)
    }

    /// Check if timer uses count-up timing / cascade mode (bit 2)
    fn is_cascade(control: u16) -> bool {
        control.get_bit(2)
    }

    /// Check if timer IRQ is enabled (bit 6)
    fn is_irq_enabled(control: u16) -> bool {
        control.get_bit(6)
    }

    /// Called when writing to `TMxCNT_L`, sets the reload value
    pub const fn set_reload(&mut self, timer: usize, value: u16) {
        match timer {
            0 => self.tm0_reload = value,
            1 => self.tm1_reload = value,
            2 => self.tm2_reload = value,
            3 => self.tm3_reload = value,
            _ => {}
        }
    }

    /// Called when writing to `TMxCNT_H`, may start/restart timer
    pub fn set_control(&mut self, timer: usize, value: u16) {
        let (old_control, counter, reload) = match timer {
            0 => (self.tm0cnt_h, &mut self.tm0cnt_l, self.tm0_reload),
            1 => (self.tm1cnt_h, &mut self.tm1cnt_l, self.tm1_reload),
            2 => (self.tm2cnt_h, &mut self.tm2cnt_l, self.tm2_reload),
            3 => (self.tm3cnt_h, &mut self.tm3cnt_l, self.tm3_reload),
            _ => return,
        };

        // If timer is being enabled (bit 7: 0->1), reload the counter
        let was_enabled = Self::is_enabled(old_control);
        let now_enabled = Self::is_enabled(value);

        if !was_enabled && now_enabled {
            *counter = reload;
            // Reset prescaler counter
            match timer {
                0 => self.tm0_prescaler_counter = 0,
                1 => self.tm1_prescaler_counter = 0,
                2 => self.tm2_prescaler_counter = 0,
                3 => self.tm3_prescaler_counter = 0,
                _ => {}
            }
        }

        // Store the new control value
        match timer {
            0 => self.tm0cnt_h = value,
            1 => self.tm1cnt_h = value,
            2 => self.tm2cnt_h = value,
            3 => self.tm3cnt_h = value,
            _ => {}
        }
    }

    /// Step all timers by one CPU cycle. Returns which timer IRQs should be triggered.
    pub fn step(&mut self) -> TimerOverflowResult {
        let mut result = TimerOverflowResult::default();

        // Timer 0 (never cascades)
        if Self::is_enabled(self.tm0cnt_h) && !Self::is_cascade(self.tm0cnt_h) {
            let prescaler = Self::get_prescaler(self.tm0cnt_h);
            self.tm0_prescaler_counter += 1;
            if self.tm0_prescaler_counter >= prescaler {
                self.tm0_prescaler_counter = 0;
                let (new_val, overflow) = self.tm0cnt_l.overflowing_add(1);
                if overflow {
                    self.tm0cnt_l = self.tm0_reload;
                    if Self::is_irq_enabled(self.tm0cnt_h) {
                        result.timer0_overflow = true;
                    }
                } else {
                    self.tm0cnt_l = new_val;
                }
            }
        }

        // Timer 1
        let tm0_overflow = result.timer0_overflow;
        if Self::is_enabled(self.tm1cnt_h) {
            let should_tick = if Self::is_cascade(self.tm1cnt_h) {
                tm0_overflow
            } else {
                let prescaler = Self::get_prescaler(self.tm1cnt_h);
                self.tm1_prescaler_counter += 1;
                if self.tm1_prescaler_counter >= prescaler {
                    self.tm1_prescaler_counter = 0;
                    true
                } else {
                    false
                }
            };

            if should_tick {
                let (new_val, overflow) = self.tm1cnt_l.overflowing_add(1);
                if overflow {
                    self.tm1cnt_l = self.tm1_reload;
                    if Self::is_irq_enabled(self.tm1cnt_h) {
                        result.timer1_overflow = true;
                    }
                } else {
                    self.tm1cnt_l = new_val;
                }
            }
        }

        // Timer 2
        let tm1_overflow = result.timer1_overflow;
        if Self::is_enabled(self.tm2cnt_h) {
            let should_tick = if Self::is_cascade(self.tm2cnt_h) {
                tm1_overflow
            } else {
                let prescaler = Self::get_prescaler(self.tm2cnt_h);
                self.tm2_prescaler_counter += 1;
                if self.tm2_prescaler_counter >= prescaler {
                    self.tm2_prescaler_counter = 0;
                    true
                } else {
                    false
                }
            };

            if should_tick {
                let (new_val, overflow) = self.tm2cnt_l.overflowing_add(1);
                if overflow {
                    self.tm2cnt_l = self.tm2_reload;
                    if Self::is_irq_enabled(self.tm2cnt_h) {
                        result.timer2_overflow = true;
                    }
                } else {
                    self.tm2cnt_l = new_val;
                }
            }
        }

        // Timer 3
        let tm2_overflow = result.timer2_overflow;
        if Self::is_enabled(self.tm3cnt_h) {
            let should_tick = if Self::is_cascade(self.tm3cnt_h) {
                tm2_overflow
            } else {
                let prescaler = Self::get_prescaler(self.tm3cnt_h);
                self.tm3_prescaler_counter += 1;
                if self.tm3_prescaler_counter >= prescaler {
                    self.tm3_prescaler_counter = 0;
                    true
                } else {
                    false
                }
            };

            if should_tick {
                let (new_val, overflow) = self.tm3cnt_l.overflowing_add(1);
                if overflow {
                    self.tm3cnt_l = self.tm3_reload;
                    if Self::is_irq_enabled(self.tm3cnt_h) {
                        result.timer3_overflow = true;
                    }
                } else {
                    self.tm3cnt_l = new_val;
                }
            }
        }

        result
    }
}
