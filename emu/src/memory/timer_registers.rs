use std::collections::HashMap;

use logger::log;

use crate::{
    bitwise::Bits,
    memory::io_registers::{IORegister, IORegisterAccessControl},
};

use super::io_device::IoDevice;

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

    unused_region: HashMap<usize, u8>,
}

impl Default for TimerRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl TimerRegisters {
    pub fn new() -> Self {
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
            unused_region: HashMap::default(),
        }
    }
}

impl IoDevice for TimerRegisters {
    type Address = usize;
    type Value = u8;

    // There is no need to read the second byte because bits `8-15` are not used
    fn read_at(&self, address: usize) -> u8 {
        match address {
            0x04000100 => self.tm0cnt_l.read().get_byte(0),
            0x04000101 => self.tm0cnt_l.read().get_byte(1),
            0x04000102 => self.tm0cnt_h.read().get_byte(0),
            0x04000103 => self.tm0cnt_h.read().get_byte(1),
            0x04000104 => self.tm1cnt_l.read().get_byte(0),
            0x04000105 => self.tm1cnt_l.read().get_byte(1),
            0x04000106 => self.tm1cnt_h.read().get_byte(0),
            0x04000107 => self.tm1cnt_h.read().get_byte(1),
            0x04000108 => self.tm2cnt_l.read().get_byte(0),
            0x04000109 => self.tm2cnt_l.read().get_byte(1),
            0x0400010A => self.tm2cnt_h.read().get_byte(0),
            0x0400010B => self.tm2cnt_h.read().get_byte(1),
            0x0400010C => self.tm3cnt_l.read().get_byte(0),
            0x0400010D => self.tm3cnt_l.read().get_byte(1),
            0x0400010E => self.tm3cnt_h.read().get_byte(0),
            0x0400010F => self.tm3cnt_h.read().get_byte(1),
            0x04000110..=0x0400012F => self.unused_region.get(&address).map_or(0, |v| *v),
            _ => panic!("Reading an write-only memory address: {address:x}"),
        }
    }

    // There is no need to set the second byte because bits `8-15` are not used
    fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x04000100 => self.tm0cnt_l.set_byte(0, value),
            0x04000101 => self.tm0cnt_l.set_byte(1, value),
            0x04000102 => self.tm0cnt_h.set_byte(0, value),
            0x04000103 => self.tm0cnt_h.set_byte(1, value),
            0x04000104 => self.tm1cnt_l.set_byte(0, value),
            0x04000105 => self.tm1cnt_l.set_byte(1, value),
            0x04000106 => self.tm1cnt_h.set_byte(0, value),
            0x04000107 => self.tm1cnt_h.set_byte(1, value),
            0x04000108 => self.tm2cnt_l.set_byte(0, value),
            0x04000109 => self.tm2cnt_l.set_byte(1, value),
            0x0400010A => self.tm2cnt_h.set_byte(0, value),
            0x0400010B => self.tm2cnt_h.set_byte(1, value),
            0x0400010C => self.tm3cnt_l.set_byte(0, value),
            0x0400010D => self.tm3cnt_l.set_byte(1, value),
            0x0400010E => self.tm3cnt_h.set_byte(0, value),
            0x0400010F => self.tm3cnt_h.set_byte(1, value),
            0x04000110..=0x0400012F => {
                log(format!("write on unused memory {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => panic!("Reading an write-only memory address: {address:x}"),
        }
    }
}
