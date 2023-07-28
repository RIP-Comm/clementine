use std::collections::HashMap;

use logger::log;
use vecfixed::VecFixed;

use crate::{
    bitwise::Bits,
    memory::io_registers::{IORegister, IORegisterAccessControl},
};

use super::io_device::IoDevice;

pub struct Interrupts {
    /// 0x04000200  2    R/W  IE        Interrupt Enable Register
    pub interrupt_enable: IORegister,

    /// 0x04000202  2    R/W  IF        Interrupt Request Flags / IRQ Acknowledge
    // It is a ring buffer since when we write to this register, the value will reach the CPU
    // after 4 cycles (source 3.10 Interrupt Latencies in ARM datasheet).
    // When we write a value to this address we write it in the back of the ring buffer.
    // When we read the value from this address we read it from the front of the ring buffer.
    // Every bus step this ring should be "advanced": peeking the back and pushing a copy of it.
    pub interrupt_request: VecFixed<5, u16>,

    /// 0x04000204  2    R/W  WAITCNT   Game Pak Waitstate Control
    wait_state: IORegister,

    /// Interrupt Master Enable Register
    pub ime: IORegister,

    //   400020Ah       -    -         Not used
    /// Post boot flag.
    post_flag: IORegister,
    //   4000301h  1    W    HALTCNT   Undocumented - Power Down Control
    power_down_control: IORegister,
    //   4000302h       -    -         Not used
    //   4000410h  ?    ?    ?         Undocumented - Purpose Unknown / Bug ??? 0FFh
    purpose_unknown: IORegister,
    //   4000411h       -    -         Not used
    //   4000800h  4    R/W  ?         Undocumented - Internal Memory Control (R/W)
    //   4000804h       -    -         Not used
    //   4xx0800h  4    R/W  ?         Mirrors of 4000800h (repeated each 64K
    unused_region: std::collections::HashMap<usize, u8>,
}

impl Default for Interrupts {
    fn default() -> Self {
        Self::new()
    }
}

impl Interrupts {
    pub fn new() -> Self {
        use IORegisterAccessControl::*;

        Self {
            interrupt_enable: IORegister::with_access_control(ReadWrite),
            interrupt_request: VecFixed::initialize(0),
            wait_state: IORegister::with_access_control(ReadWrite),
            post_flag: IORegister::with_access_control(ReadWrite),
            power_down_control: IORegister::with_access_control(ReadWrite),
            ime: IORegister::with_access_control(ReadWrite),
            purpose_unknown: IORegister::with_access_control(ReadWrite),
            unused_region: HashMap::new(),
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
            0x04000202 => self.interrupt_request.front().unwrap_or(&0).get_byte(0),
            0x04000203 => self.interrupt_request.front().unwrap_or(&0).get_byte(1),
            0x04000204 => self.wait_state.read().get_byte(0),
            0x04000205 => self.wait_state.read().get_byte(1),
            0x04000206 | 0x04000207 | 0x400020A..=0x40002FF => {
                log("read on unused memory");
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            0x04000300 => self.post_flag.read().get_byte(0),
            0x04000208 => self.ime.read().get_byte(0),
            0x04000209 => self.ime.read().get_byte(1),
            0x04000410 => self.purpose_unknown.read().get_byte(0),
            _ => panic!("Reading an write-only memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x04000200 => self.interrupt_enable.set_byte(0, value),
            0x04000201 => self.interrupt_enable.set_byte(1, value),
            // When we write interrupt requests register we're acknowledging that we got the
            // irq request. For this reason for example if we write `1` in the lsb in reality what we want to do
            // is to clear the lsb in the interrupt requests register.
            // Source GBATEK: https://rust-console.github.io/gbatek-gbaonly/#4000202h---if---interrupt-request-flags--irq-acknowledge-rw-see-below
            // cite: Interrupts must be manually acknowledged by writing a “1” to one of the IRQ bits, the IRQ bit will then be cleared.
            // it's not 100% clear but this looks like what we have to do...
            0x04000202 => {
                let current_val = self.interrupt_request.back_mut().unwrap();

                *current_val &= !(value as u16);
            }
            0x04000203 => {
                let current_val = self.interrupt_request.back_mut().unwrap();

                *current_val &= !((value as u16) << 8);
            }
            0x04000204 => self.wait_state.set_byte(0, value),
            0x04000205 => self.wait_state.set_byte(1, value),
            0x04000206 | 0x04000207 | 0x400020A..=0x40002FF => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            0x04000208 => self.ime.set_byte(0, value),
            0x04000209 => self.ime.set_byte(1, value),
            0x04000300 => self.post_flag.set_byte(0, value),
            0x04000301 => self.power_down_control.set_byte(0, value),
            0x04000410 => self.purpose_unknown.set_byte(0, value),
            _ => panic!("Writing an read-only memory address: {address:x}"),
        }
    }
}
