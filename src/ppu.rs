pub const DISPLAY_WIDTH: usize = 240;
pub const DISPLAY_HEIGHT: usize = 160;

// Pixel Process Unit

// Part with tiles (8x8)

// Part 256k buffer a monitor

// Sprite Attributes 0xFE00 -> 0xFE9f

// Video RAM 0x0800 - 0x9FFF => A schermo

// rgb byte, byte, byte

pub struct Ppu {
    pub rom: Vec<u8>,
}

impl Ppu {
    pub(crate) fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
        }
    }
}

