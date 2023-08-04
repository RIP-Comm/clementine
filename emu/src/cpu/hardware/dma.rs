#[derive(Default)]
pub struct DmaRegisters {
    pub source_address: u32,
    pub destination_address: u32,
    pub word_count: u16,
    pub control: u16,
}

#[derive(Default)]
pub struct Dma {
    pub channels: [DmaRegisters; 4],
}
