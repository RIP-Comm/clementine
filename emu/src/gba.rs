use crate::{cartridge_header::CartridgeHeader, cpu::Cpu};

pub struct Gba<T>
where
    T: Cpu,
{
    pub cpu: T,
    pub cartridge_header: CartridgeHeader,
}

impl<T> Gba<T>
where
    T: Cpu,
{
    pub const fn new(cartridge_header: CartridgeHeader, cpu: T) -> Self {
        Self {
            cpu,
            cartridge_header,
        }
    }
}
