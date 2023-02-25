use crate::bitwise::Bits;
use crate::memory::io_device::IoDevice;
use crate::memory::io_registers::{IORegister, IORegisterAccessControl};
use logger::log;
use std::collections::HashMap;

struct DmaRegisters {
    pub source_address: IORegister,
    pub destination_address: IORegister,
    pub word_count: IORegister,
    pub control: IORegister,
}

impl Default for DmaRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl DmaRegisters {
    pub const fn new() -> Self {
        Self {
            source_address: IORegister::with_access_control(IORegisterAccessControl::ReadWrite),
            destination_address: IORegister::with_access_control(
                IORegisterAccessControl::ReadWrite,
            ),
            word_count: IORegister::with_access_control(IORegisterAccessControl::ReadWrite),
            control: IORegister::with_access_control(IORegisterAccessControl::ReadWrite),
        }
    }
}

impl IoDevice for DmaRegisters {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        match address {
            0 => self.source_address.read().get_byte(0),
            1 => self.source_address.read().get_byte(1),
            2 => self.source_address.read().get_byte(2),
            3 => self.source_address.read().get_byte(3),
            4 => self.destination_address.read().get_byte(0),
            5 => self.destination_address.read().get_byte(1),
            6 => self.destination_address.read().get_byte(2),
            7 => self.destination_address.read().get_byte(3),
            8 => self.word_count.read().get_byte(0),
            9 => self.word_count.read().get_byte(1),
            10 => self.control.read().get_byte(0),
            11 => self.control.read().get_byte(1),
            _ => panic!("Not implemented read memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        match address {
            0 => self.source_address.set_byte(0, value),
            1 => self.source_address.set_byte(1, value),
            2 => self.source_address.set_byte(2, value),
            3 => self.source_address.set_byte(3, value),
            4 => self.destination_address.set_byte(0, value),
            5 => self.destination_address.set_byte(1, value),
            6 => self.destination_address.set_byte(2, value),
            7 => self.destination_address.set_byte(3, value),
            8 => self.word_count.set_byte(0, value),
            9 => self.word_count.set_byte(1, value),
            10 => self.control.set_byte(0, value),
            11 => self.control.set_byte(1, value),
            _ => panic!("Not implemented write memory address: {address:x}"),
        }
    }
}

pub struct Dma {
    banks: [DmaRegisters; 4],
    unused: HashMap<usize, u8>,
}

impl Default for Dma {
    fn default() -> Self {
        Self::new()
    }
}

impl Dma {
    pub fn new() -> Self {
        Self {
            banks: [
                DmaRegisters::default(),
                DmaRegisters::default(),
                DmaRegisters::default(),
                DmaRegisters::default(),
            ],
            unused: HashMap::new(),
        }
    }
}

impl IoDevice for Dma {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        match address {
            0x040000B0..=0x040000BB => self.banks[0].read_at(address - 0x040000B0),
            0x040000BC..=0x040000C7 => self.banks[1].read_at(address - 0x040000BC),
            0x040000C8..=0x040000D3 => self.banks[2].read_at(address - 0x040000C8),
            0x040000D4..=0x040000DF => self.banks[3].read_at(address - 0x040000D4),
            0x040000E0..=0x040000FF => {
                log("read on unused memory");
                self.unused.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("Not implemented read memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        match address {
            0x040000B0..=0x040000BB => self.banks[0].write_at(address - 0x040000B0, value),
            0x040000BC..=0x040000C7 => self.banks[1].write_at(address - 0x040000BC, value),
            0x040000C8..=0x040000D3 => self.banks[2].write_at(address - 0x040000C8, value),
            0x040000D4..=0x040000DF => self.banks[3].write_at(address - 0x040000D4, value),
            0x040000E0..=0x040000FF => {
                log("write on unused memory");
                self.unused.insert(address, value);
            }
            _ => panic!("Not implemented write memory address: {address:x}"),
        }
    }
}
