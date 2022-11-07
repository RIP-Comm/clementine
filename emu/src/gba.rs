use std::{cell::RefCell, rc::Rc};

use crate::{
    arm7tdmi::Arm7tdmi,
    cartridge_header::CartridgeHeader,
    memory::internal_memory::InternalMemory,
    render::{gba_lcd::GbaLcd, ppu::PixelProcessUnit},
};

pub struct Gba {
    pub cpu: Arm7tdmi,

    pub cartridge_header: CartridgeHeader,
    pub cartridge: Rc<RefCell<Vec<u8>>>,

    pub memory: Rc<RefCell<InternalMemory>>,

    pub lcd: Rc<RefCell<Box<GbaLcd>>>,
    pub ppu: PixelProcessUnit,
}

impl Gba {
    pub fn new(cartridge_header: CartridgeHeader, cartridge: Rc<RefCell<Vec<u8>>>) -> Self {
        let lcd = Rc::new(RefCell::new(Box::new(GbaLcd::new())));
        let memory = Rc::new(RefCell::new(InternalMemory::new()));
        let ppu = PixelProcessUnit::new(lcd.clone(), memory.clone());
        let arm = Arm7tdmi::new(cartridge.clone(), memory.clone());
        Self {
            cpu: arm,
            cartridge,
            cartridge_header,
            ppu,
            lcd,
            memory,
        }
    }
}
