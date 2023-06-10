use std::sync::{Arc, Mutex};

use crate::memory::{internal_memory::InternalMemory, io_device::IoDevice};

#[derive(Default)]
pub struct Bus {
    internal_memory: Arc<Mutex<InternalMemory>>,
}

impl IoDevice for Bus {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        self.internal_memory.lock().unwrap().read_at(address)
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        self.internal_memory
            .lock()
            .unwrap()
            .write_at(address, value);
    }
}

impl Bus {
    pub fn with_memory(memory: Arc<Mutex<InternalMemory>>) -> Self {
        Self {
            internal_memory: memory,
        }
    }

    pub fn read_word(&self, address: usize) -> u32 {
        self.internal_memory.lock().unwrap().read_word(address)
    }

    pub fn write_word(&mut self, address: usize, value: u32) {
        self.internal_memory
            .lock()
            .unwrap()
            .write_word(address, value);
    }

    pub fn read_half_word(&self, address: usize) -> u16 {
        self.internal_memory.lock().unwrap().read_half_word(address)
    }

    pub fn write_half_word(&mut self, address: usize, value: u16) {
        self.internal_memory
            .lock()
            .unwrap()
            .write_half_word(address, value);
    }
}
