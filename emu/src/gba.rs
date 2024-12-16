use std::sync::{Arc, Mutex};

use crate::{
    bus::Bus,
    cartridge_header::CartridgeHeader,
    cpu::{arm7tdmi::Arm7tdmi, hardware::internal_memory::InternalMemory},
    render::gba_lcd::GbaLcd,
};

pub struct Gba {
    pub cpu: Arm7tdmi,

    pub cartridge_header: CartridgeHeader,
    pub lcd: Arc<Mutex<Box<GbaLcd>>>,
}

impl Gba {
    #[must_use]
    pub fn new(
        cartridge_header: CartridgeHeader,
        bios: [u8; 0x0000_4000],
        cartridge: Vec<u8>,
    ) -> Self {
        let lcd = Arc::new(Mutex::new(Box::default()));
        let memory = InternalMemory::new(bios, cartridge);
        let bus = Bus::with_memory(memory);
        let arm = Arm7tdmi::new(bus);

        Self {
            cpu: arm,
            cartridge_header,
            lcd,
        }
    }

    pub fn step(&mut self) {
        self.cpu.step();
    }
}
