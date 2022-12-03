use crate::{
    bitwise::Bits,
    memory::io_registers::{IORegister, IORegisterAccessControl},
};

use super::io_device::IoDevice;

pub struct Interrupts {
    //   4000200h  2    R/W  IE        Interrupt Enable Register
    //   4000202h  2    R/W  IF        Interrupt Request Flags / IRQ Acknowledge
    //   4000204h  2    R/W  WAITCNT   Game Pak Waitstate Control
    //   4000206h       -    -         Not used
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
            post_flag: IORegister::with_access_control(ReadWrite),
            ime: IORegister::with_access_control(ReadWrite),
        }
    }
}

impl IoDevice for Interrupts {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: usize) -> u8 {
        match address {
            0x04000300 => self.post_flag.read().get_byte(0),
            0x04000208 => self.ime.read().get_byte(0),
            0x04000209 => self.ime.read().get_byte(1),
            _ => panic!("Reading an write-only memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x04000300 => self.post_flag.set_byte(0, value),
            0x04000208 => self.ime.set_byte(0, value),
            0x04000209 => self.ime.set_byte(1, value),
            _ => panic!("Writing an read-only memory address: {address:x}"),
        }
    }
}
