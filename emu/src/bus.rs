use logger::log;

use crate::memory::{internal_memory::InternalMemory, io_device::IoDevice};

#[derive(Default)]
pub struct Bus {
    pub internal_memory: InternalMemory,
    cycles_count: u128,
    last_used_address: usize,
}

impl Bus {
    pub fn read_at(&mut self, address: usize) -> u8 {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.internal_memory.read_at(address)
    }

    pub fn write_at(&mut self, address: usize, value: u8) {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.internal_memory.write_at(address, value);
    }

    fn step(&mut self) {
        // Step cycles at beginning or end?
        // It may have an impact when we will introduce timers.
        self.cycles_count += 1;

        // TODO: move this somewhere in the UI
        log(format!("CPU Cycles: {}", self.cycles_count));

        // Step ppu, dma, interrupts, timers, etc...
        let val = *self
            .internal_memory
            .interrupts
            .interrupt_request
            .back()
            .unwrap();
        self.internal_memory.interrupts.interrupt_request.push(val);
    }

    pub fn with_memory(memory: InternalMemory) -> Self {
        Self {
            internal_memory: memory,
            ..Default::default()
        }
    }

    const fn get_wait_cycles(&self, address: usize) -> u128 {
        let _is_sequential =
            address == self.last_used_address || address + 4 == self.last_used_address;

        match address {
            // Bios
            0x0..=0x3FFF => 1,
            _ => 0,
        }
    }

    pub fn read_word(&mut self, address: usize) -> u32 {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        self.internal_memory.read_word(address)
    }

    pub fn write_word(&mut self, address: usize, value: u32) {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        self.internal_memory.write_word(address, value);
    }

    pub fn read_half_word(&mut self, address: usize) -> u16 {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.internal_memory.read_half_word(address)
    }

    pub fn write_half_word(&mut self, address: usize, value: u16) {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.internal_memory.write_half_word(address, value);
    }
}
