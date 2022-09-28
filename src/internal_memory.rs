use crate::io_device::IoDevice;

pub struct InternalMemory {
    /// From 0x03000000 to 0x03007FFF (32kb).
    internal_work_ram: [u8; 0x7FFF],
}

impl InternalMemory {
    pub const fn new() -> Self {
        Self {
            internal_work_ram: [0; 0x7FFF],
        }
    }
}

impl IoDevice for InternalMemory {
    type Address = u32;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        let a: usize = address.try_into().unwrap();
        self.internal_work_ram[a]
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        let a: usize = address.try_into().unwrap();
        self.internal_work_ram[a] = value;
    }
}
