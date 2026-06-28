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

    /// Advance all timers by `cycles` CPU cycles. Returns which timer IRQs
    /// should be triggered (an overflow happened while that timer's IRQ is
    /// enabled).
    ///
    /// Cascade timers tick once per overflow of the timer below them, so the
    /// overflow counts are threaded down the chain independently of whether
    /// each timer's IRQ is enabled.
    pub fn step(&mut self, cycles: u64) -> TimerOverflowResult {
        // Timer 0 never cascades; the cascade bit is ignored on hardware.
        let tm0 = if Self::is_enabled(self.tm0cnt_h) && !Self::is_cascade(self.tm0cnt_h) {
            Self::advance(
                &mut self.tm0cnt_l,
                self.tm0_reload,
                &mut self.tm0_prescaler_counter,
                Self::get_prescaler(self.tm0cnt_h),
                cycles,
            )
        } else {
            0
        };

        let tm1 = self.step_timer(self.tm1cnt_h, cycles, tm0, 1);
        let tm2 = self.step_timer(self.tm2cnt_h, cycles, tm1, 2);
        let tm3 = self.step_timer(self.tm3cnt_h, cycles, tm2, 3);

        TimerOverflowResult {
            timer0_overflow: tm0 > 0 && Self::is_irq_enabled(self.tm0cnt_h),
            timer1_overflow: tm1 > 0 && Self::is_irq_enabled(self.tm1cnt_h),
            timer2_overflow: tm2 > 0 && Self::is_irq_enabled(self.tm2cnt_h),
            timer3_overflow: tm3 > 0 && Self::is_irq_enabled(self.tm3cnt_h),
        }
    }

    /// Advance one of timers 1..=3 and return how many times it overflowed.
    /// In cascade mode it ticks once per overflow of the timer below it,
    /// otherwise it is driven by its own prescaler.
    fn step_timer(&mut self, control: u16, cycles: u64, prev_overflows: u32, timer: usize) -> u32 {
        if !Self::is_enabled(control) {
            return 0;
        }

        let (counter, reload, prescaler_counter) = match timer {
            1 => (
                &mut self.tm1cnt_l,
                self.tm1_reload,
                &mut self.tm1_prescaler_counter,
            ),
            2 => (
                &mut self.tm2cnt_l,
                self.tm2_reload,
                &mut self.tm2_prescaler_counter,
            ),
            _ => (
                &mut self.tm3cnt_l,
                self.tm3_reload,
                &mut self.tm3_prescaler_counter,
            ),
        };

        if Self::is_cascade(control) {
            Self::apply_ticks(counter, reload, prev_overflows)
        } else {
            Self::advance(
                counter,
                reload,
                prescaler_counter,
                Self::get_prescaler(control),
                cycles,
            )
        }
    }

    /// Advance a prescaler-driven timer by `cycles`, carrying the prescaler
    /// remainder across calls. Returns how many times the counter overflowed.
    // The remainder is below the prescaler (<= 1024) and the tick count fits a
    // u32 for any realistic per-step cycle delta, so the casts cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    fn advance(
        counter: &mut u16,
        reload: u16,
        prescaler_counter: &mut u32,
        prescaler: u32,
        cycles: u64,
    ) -> u32 {
        let total = u64::from(*prescaler_counter) + cycles;
        let ticks = total / u64::from(prescaler);
        *prescaler_counter = (total % u64::from(prescaler)) as u32;
        Self::apply_ticks(counter, reload, ticks as u32)
    }

    /// Add `ticks` to a timer counter, reloading on each overflow. Returns the
    /// number of overflows. After the first overflow the counter runs from
    /// `reload`, so the wrap period is `0x1_0000 - reload`.
    // The two `as u16` results are reduced modulo the wrap period, so they are
    // always below `0x1_0000` and cannot truncate.
    #[allow(clippy::cast_possible_truncation)]
    fn apply_ticks(counter: &mut u16, reload: u16, ticks: u32) -> u32 {
        if ticks == 0 {
            return 0;
        }

        let value = u32::from(*counter) + ticks;
        if value < 0x1_0000 {
            *counter = value as u16;
            return 0;
        }

        let period = 0x1_0000 - u32::from(reload);
        let past_first = value - 0x1_0000;
        let overflows = 1 + past_first / period;
        *counter = (u32::from(reload) + past_first % period) as u16;
        overflows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Control register bits: enable (7), irq (6), cascade (2), prescaler (0-1).
    const ENABLE: u16 = 1 << 7;
    const IRQ: u16 = 1 << 6;
    const CASCADE: u16 = 1 << 2;

    #[test]
    fn overflows_after_exactly_one_full_period() {
        let mut t = Timers::default();
        t.set_reload(0, 0);
        t.set_control(0, ENABLE | IRQ); // prescaler 1, counter reloads to 0

        // One short of a full 16-bit period: no overflow yet.
        let r = t.step(0xFFFF);
        assert!(!r.timer0_overflow);
        assert_eq!(t.tm0cnt_l, 0xFFFF);

        // The cycle that completes the period overflows and reloads to 0.
        let r = t.step(1);
        assert!(r.timer0_overflow);
        assert_eq!(t.tm0cnt_l, 0);
    }

    #[test]
    fn prescaler_divides_and_carries_remainder() {
        let mut t = Timers::default();
        t.set_reload(0, 0xFFFF); // wrap period of one tick
        t.set_control(0, ENABLE | IRQ | 0b01); // prescaler 64

        // 63 cycles: not enough for a single tick.
        assert!(!t.step(63).timer0_overflow);
        assert_eq!(t.tm0cnt_l, 0xFFFF);

        // The 64th cycle produces one tick, which overflows the reloaded counter.
        let r = t.step(1);
        assert!(r.timer0_overflow);
        assert_eq!(t.tm0cnt_l, 0xFFFF);
    }

    #[test]
    fn multiple_overflows_in_a_single_step() {
        let mut t = Timers::default();
        t.set_reload(0, 0xFF00); // period of 0x100 ticks
        t.set_control(0, ENABLE | IRQ);

        // Two full periods plus a bit in one step.
        let r = t.step(0x100 * 2 + 5);
        assert!(r.timer0_overflow);
        assert_eq!(t.tm0cnt_l, 0xFF05);
    }

    #[test]
    fn cascade_ticks_once_per_lower_overflow() {
        let mut t = Timers::default();
        // Timer 0 overflows every cycle (period of one tick).
        t.set_reload(0, 0xFFFF);
        t.set_control(0, ENABLE);

        // Timer 1 cascades, period of two ticks.
        t.set_reload(1, 0xFFFE);
        t.set_control(1, ENABLE | IRQ | CASCADE);

        // Three timer-0 overflows advance the cascade timer three ticks,
        // which is one and a half of its two-tick period: one overflow.
        let r = t.step(3);
        assert!(r.timer1_overflow);
        assert_eq!(t.tm1cnt_l, 0xFFFF);
    }

    #[test]
    fn disabled_timer_does_not_advance() {
        let mut t = Timers::default();
        t.set_reload(0, 0);
        // Not enabled.
        let r = t.step(1_000_000);
        assert!(!r.timer0_overflow);
        assert_eq!(t.tm0cnt_l, 0);
    }
}
