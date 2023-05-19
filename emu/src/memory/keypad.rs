use crate::bitwise::Bits;
use crate::memory::io_device::IoDevice;
use crate::memory::io_registers::{IORegister, IORegisterAccessControl};

pub struct Keypad {
    pub key_input: IORegister,
    pub key_interrupt_control: IORegister,
}

impl Default for Keypad {
    fn default() -> Self {
        Self::new()
    }
}

impl Keypad {
    pub const fn new() -> Self {
        Self {
            key_input: IORegister::with_access_control(IORegisterAccessControl::ReadWrite),
            key_interrupt_control: IORegister::with_access_control(
                IORegisterAccessControl::ReadWrite,
            ),
        }
    }
}

impl IoDevice for Keypad {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        match address {
            0x4000130 => self.key_input.read().get_byte(0),
            0x4000131 => self.key_input.read().get_byte(1),
            0x4000132 => self.key_interrupt_control.read().get_byte(0),
            0x4000133 => self.key_interrupt_control.read().get_byte(1),
            _ => panic!("Not implemented read memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        match address {
            0x4000130 => self.key_input.set_byte(0, value),
            0x4000131 => self.key_input.set_byte(1, value),
            0x4000132 => self.key_interrupt_control.set_byte(0, value),
            0x4000133 => self.key_interrupt_control.set_byte(1, value),
            _ => panic!("Not implemented write memory address: {address:x}"),
        }
    }
}
