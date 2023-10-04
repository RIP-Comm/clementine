use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct DmaRegisters {
    pub source_address: u32,
    pub destination_address: u32,
    pub word_count: u16,
    pub control: u16,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Dma {
    pub channels: [DmaRegisters; 4],
}
