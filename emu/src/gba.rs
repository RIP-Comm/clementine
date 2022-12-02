use std::sync::{Arc, Mutex};

use crate::arm::arm7tdmi::Arm7tdmi;
use crate::{
    cartridge_header::CartridgeHeader,
    memory::internal_memory::InternalMemory,
    render::{gba_lcd::GbaLcd, ppu::PixelProcessUnit},
};

pub struct Gba {
    pub cpu: Arm7tdmi,

    pub cartridge_header: CartridgeHeader,

    pub memory: Arc<Mutex<InternalMemory>>,

    pub lcd: Arc<Mutex<Box<GbaLcd>>>,
    pub ppu: PixelProcessUnit,
}

impl Gba {
    pub fn new(
        cartridge_header: CartridgeHeader,
        bios: [u8; 0x00004000],
        cartridge: Vec<u8>,
    ) -> Self {
        let lcd = Arc::new(Mutex::new(Box::new(GbaLcd::new())));
        let memory = Arc::new(Mutex::new(InternalMemory::new(bios, cartridge)));
        let ppu = PixelProcessUnit::new(Arc::clone(&lcd), Arc::clone(&memory));
        let arm = Arm7tdmi::new(Arc::clone(&memory));
        Self {
            cpu: arm,
            cartridge_header,
            ppu,
            lcd,
            memory,
        }
    }
}
