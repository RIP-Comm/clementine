use serde::{Deserialize, Serialize};
use serde_with::serde_as;

// Using Box here to avoid stack overflow
#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct Memory {
    /// From 0x05000000 to  0x050001FF (512 bytes, 256 colors).
    #[serde_as(as = "Box<[_; 512]>")]
    pub bg_palette_ram: Box<[u8; 0x200]>,
    /// From 0x05000200 to 0x050003FF (512 bytes, 256 colors).
    #[serde_as(as = "Box<[_; 512]>")]
    pub obj_palette_ram: Box<[u8; 0x200]>,
    /// From 0x06000000 to 0x06017FFF (96 kb).
    #[serde_as(as = "Box<[_; 98304]>")]
    pub video_ram: Box<[u8; 0x18000]>,
    /// From 0x07000000 to 0x070003FF (1kbyte)
    #[serde_as(as = "Box<[_; 1024]>")]
    pub obj_attributes: Box<[u8; 0x400]>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            bg_palette_ram: Box::new([0; 0x200]),
            obj_palette_ram: Box::new([8; 0x200]),
            video_ram: Box::new([0; 0x18000]),
            obj_attributes: Box::new([0; 0x400]),
        }
    }
}
