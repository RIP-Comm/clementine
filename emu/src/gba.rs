use std::sync::{Arc, Mutex};

use crate::{
    bus::Bus,
    cartridge_header::CartridgeHeader,
    cpu::arm7tdmi::Arm7tdmi,
    memory::internal_memory::InternalMemory,
    render::{gba_lcd::GbaLcd, ppu::PixelProcessUnit},
};

pub struct Gba {
    pub cpu: Arc<Mutex<Arm7tdmi>>,

    pub cartridge_header: CartridgeHeader,
    pub lcd: Arc<Mutex<Box<GbaLcd>>>,
    pub ppu: PixelProcessUnit,
}

impl Gba {
    pub fn new(
        cartridge_header: CartridgeHeader,
        bios: [u8; 0x00004000],
        cartridge: Vec<u8>,
    ) -> Self {
        let lcd = Arc::new(Mutex::new(Box::default()));
        let memory = InternalMemory::new(bios, cartridge);
        let bus = Bus::with_memory(memory);
        let arm = Arc::new(Mutex::new(Arm7tdmi::new(bus)));

        // TODO: ppu needs to have direct access to memory or is it through bus (so it increments cycles when reading?)
        // to check
        let ppu = PixelProcessUnit::new(Arc::clone(&lcd), Arc::clone(&arm));

        Self {
            cpu: arm,
            cartridge_header,
            lcd,
            ppu,
        }
    }

    pub fn step(&mut self) {
        self.cpu.lock().unwrap().step();
    }
}
