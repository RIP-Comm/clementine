use crate::memory::io_registers::{IORegister, IORegisterAccessControl};

pub struct TimerRegisters {
    /// Timer 0 Counter/Reload
    pub tm0cnt_l: IORegister,
    /// Timer 0 Control
    pub tm0cnt_h: IORegister,
    /// Timer 1 Counter/Reload
    pub tm1cnt_l: IORegister,
    /// Timer 1 Control
    pub tm1cnt_h: IORegister,
    /// Timer 2 Counter/Reload
    pub tm2cnt_l: IORegister,
    /// Timer 2 Control
    pub tm2cnt_h: IORegister,
    /// Timer 3 Counter/Reload
    pub tm3cnt_l: IORegister,
    /// Timer 3 Control
    pub tm3cnt_h: IORegister,
}

impl Default for TimerRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerRegisters {
    pub const fn new() -> Self {
        use IORegisterAccessControl::*;

        Self {
            tm0cnt_l: IORegister::with_access_control(ReadWrite),
            tm0cnt_h: IORegister::with_access_control(ReadWrite),
            tm1cnt_l: IORegister::with_access_control(ReadWrite),
            tm1cnt_h: IORegister::with_access_control(ReadWrite),
            tm2cnt_l: IORegister::with_access_control(ReadWrite),
            tm2cnt_h: IORegister::with_access_control(ReadWrite),
            tm3cnt_l: IORegister::with_access_control(ReadWrite),
            tm3cnt_h: IORegister::with_access_control(ReadWrite),
        }
    }
}
