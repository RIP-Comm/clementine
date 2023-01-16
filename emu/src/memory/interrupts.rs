use crate::{
    bitwise::Bits,
    memory::io_registers::{IORegister, IORegisterAccessControl},
};

use super::io_device::IoDevice;

pub struct Interrupts {
    /// 0x04000200  2    R/W  IE        Interrupt Enable Register
    interrupt_enable: IORegister,

    /// 0x04000202  2    R/W  IF        Interrupt Request Flags / IRQ Acknowledge
    interrupt_request: IORegister,

    /// 0x04000204  2    R/W  WAITCNT   Game Pak Waitstate Control
    wait_state: IORegister,

    /// 0x04000206       -    -         Not used
    not_used_06: IORegister,
    /// 0x400020A       -    -         Not used
    not_used_a: IORegister,

    /// 0x400020C       -    -         Not used
    not_used_c: IORegister, // FIXME: Not sure about this

    /// 0x400020E       -    -         Not used
    not_used_e: IORegister, // FIXME: Not sure about this

    /// 0x4000210       -    -         Not used
    not_used_10: IORegister, // FIXME: Not sure about this

    /// 0x4000212       -    -         Not used
    not_used_12: IORegister, // FIXME: Not sure about this

    /// 0x4000214       -    -         Not used
    not_used_14: IORegister, // FIXME: Not sure about this

    /// 0x4000216       -    -         Not used
    not_used_16: IORegister, // FIXME: Not sure about this

    /// 0x4000218       -    -         Not used
    not_used_18: IORegister, // FIXME: Not sure about this

    /// 0x400021A       -    -         Not used
    not_used_1a: IORegister, // FIXME: Not sure about this

    /// Interrupt Master Enable Register
    ime: IORegister,

    //   400020Ah       -    -         Not used
    /// Post boot flag.
    post_flag: IORegister,
    //   4000301h  1    W    HALTCNT   Undocumented - Power Down Control
    //   4000302h       -    -         Not used
    //   4000410h  ?    ?    ?         Undocumented - Purpose Unknown / Bug ??? 0FFh
    //   4000411h       -    -         Not used
    //   4000800h  4    R/W  ?         Undocumented - Internal Memory Control (R/W)
    //   4000804h       -    -         Not used
    //   4xx0800h  4    R/W  ?         Mirrors of 4000800h (repeated each 64K)
}

impl Default for Interrupts {
    fn default() -> Self {
        Self::new()
    }
}

impl Interrupts {
    pub const fn new() -> Self {
        use IORegisterAccessControl::*;

        Self {
            interrupt_enable: IORegister::with_access_control(ReadWrite),
            interrupt_request: IORegister::with_access_control(ReadWrite),
            wait_state: IORegister::with_access_control(ReadWrite),
            post_flag: IORegister::with_access_control(ReadWrite),
            ime: IORegister::with_access_control(ReadWrite),
            not_used_06: IORegister::with_access_control(ReadWrite),
            not_used_a: IORegister::with_access_control(ReadWrite),
            not_used_c: IORegister::with_access_control(ReadWrite),
            not_used_e: IORegister::with_access_control(ReadWrite),
            not_used_10: IORegister::with_access_control(ReadWrite),
            not_used_12: IORegister::with_access_control(ReadWrite),
            not_used_14: IORegister::with_access_control(ReadWrite),
            not_used_16: IORegister::with_access_control(ReadWrite),
            not_used_18: IORegister::with_access_control(ReadWrite),
            not_used_1a: IORegister::with_access_control(ReadWrite),
        }
    }
}

impl IoDevice for Interrupts {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: usize) -> u8 {
        match address {
            0x04000200 => self.interrupt_enable.read().get_byte(0),
            0x04000201 => self.interrupt_enable.read().get_byte(1),
            0x04000202 => self.interrupt_request.read().get_byte(0),
            0x04000203 => self.interrupt_request.read().get_byte(1),
            0x04000204 => self.wait_state.read().get_byte(0),
            0x04000205 => self.wait_state.read().get_byte(1),
            0x04000206 => self.not_used_06.read().get_byte(0),
            0x04000207 => self.not_used_06.read().get_byte(1),
            0x0400020A => self.not_used_a.read().get_byte(0),
            0x0400020B => self.not_used_a.read().get_byte(1),
            0x0400020C => self.not_used_c.read().get_byte(0),
            0x0400020D => self.not_used_c.read().get_byte(1),
            0x0400020E => self.not_used_e.read().get_byte(0),
            0x0400020F => self.not_used_e.read().get_byte(1),
            0x04000210 => self.not_used_10.read().get_byte(0),
            0x04000211 => self.not_used_10.read().get_byte(1),
            0x04000212 => self.not_used_12.read().get_byte(0),
            0x04000213 => self.not_used_12.read().get_byte(1),
            0x04000214 => self.not_used_14.read().get_byte(0),
            0x04000215 => self.not_used_14.read().get_byte(1),
            0x04000216 => self.not_used_16.read().get_byte(0),
            0x04000217 => self.not_used_16.read().get_byte(1),
            0x04000218 => self.not_used_18.read().get_byte(0),
            0x04000219 => self.not_used_18.read().get_byte(1),
            0x0400021A => self.not_used_1a.read().get_byte(0),
            0x0400021B => self.not_used_1a.read().get_byte(1),
            0x04000300 => self.post_flag.read().get_byte(0),
            0x04000208 => self.ime.read().get_byte(0),
            0x04000209 => self.ime.read().get_byte(1),
            _ => panic!("Reading an write-only memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x04000200 => self.interrupt_enable.set_byte(0, value),
            0x04000201 => self.interrupt_enable.set_byte(1, value),
            0x04000202 => self.interrupt_request.set_byte(0, value),
            0x04000203 => self.interrupt_request.set_byte(1, value),
            0x04000204 => self.wait_state.set_byte(0, value),
            0x04000205 => self.wait_state.set_byte(1, value),
            0x04000206 => self.not_used_06.set_byte(0, value),
            0x04000207 => self.not_used_06.set_byte(1, value),
            0x0400020A => self.not_used_a.set_byte(0, value),
            0x0400020B => self.not_used_a.set_byte(1, value),
            0x0400020C => self.not_used_c.set_byte(0, value),
            0x0400020D => self.not_used_c.set_byte(1, value),
            0x0400020E => self.not_used_e.set_byte(0, value),
            0x0400020F => self.not_used_e.set_byte(1, value),
            0x04000210 => self.not_used_10.set_byte(0, value),
            0x04000211 => self.not_used_10.set_byte(1, value),
            0x04000212 => self.not_used_12.set_byte(0, value),
            0x04000213 => self.not_used_12.set_byte(1, value),
            0x04000214 => self.not_used_14.set_byte(0, value),
            0x04000215 => self.not_used_14.set_byte(1, value),
            0x04000216 => self.not_used_16.set_byte(0, value),
            0x04000217 => self.not_used_16.set_byte(1, value),
            0x04000218 => self.not_used_18.set_byte(0, value),
            0x04000219 => self.not_used_18.set_byte(1, value),
            0x0400021A => self.not_used_1a.set_byte(0, value),
            0x0400021B => self.not_used_1a.set_byte(1, value),
            0x04000300 => self.post_flag.set_byte(0, value),
            0x04000208 => self.ime.set_byte(0, value),
            0x04000209 => self.ime.set_byte(1, value),
            _ => panic!("Writing an read-only memory address: {address:x}"),
        }
    }
}
