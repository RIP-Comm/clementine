use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

use super::ObjMappingKind;

#[derive(Default, Serialize, Deserialize)]
pub struct Registers {
    /// LCD Control
    pub dispcnt: u16,
    /// Undocumented - Green Swap
    pub green_swap: u16,
    /// General LCD Status (STAT, LYC)
    pub dispstat: u16,
    /// Vertical Counter (LY)
    pub vcount: u16,
    /// BG0 Control
    pub bg0cnt: u16,
    /// BG1 Control
    pub bg1cnt: u16,
    /// BG2 Control
    pub bg2cnt: u16,
    /// BG3 Control
    pub bg3cnt: u16,
    /// BG0 X-Offset
    pub bg0hofs: u16,
    /// BG0 Y_Offset
    pub bg0vofs: u16,
    /// BG1 X-Offset
    pub bg1hofs: u16,
    /// BG1 Y_Offset
    pub bg1vofs: u16,
    /// BG2 X-Offset
    pub bg2hofs: u16,
    /// BG2 Y_Offset
    pub bg2vofs: u16,
    /// BG3 X-Offset
    pub bg3hofs: u16,
    /// BG3 Y_Offset
    pub bg3vofs: u16,
    /// BG2 Rotation/Scaling Parameter A (dx)
    pub bg2pa: u16,
    /// BG2 Rotation/Scaling Parameter B (dmx)
    pub bg2pb: u16,
    /// BG2 Rotation/Scaling Parameter C (dy)
    pub bg2pc: u16,
    /// BG2 Rotation/Scaling Parameter D (dmy)
    pub bg2pd: u16,
    /// BG2 Reference Point X-Coordinate
    pub bg2x: u32,
    /// BG2 Reference Point Y-Coordinate
    pub bg2y: u32,
    /// BG3 Rotation/Scaling Parameter A (dx)
    pub bg3pa: u16,
    /// BG3 Rotation/Scaling Parameter B (dmx)
    pub bg3pb: u16,
    /// BG3 Rotation/Scaling Parameter C (dy)
    pub bg3pc: u16,
    /// BG3 Rotation/Scaling Parameter D (dmy)
    pub bg3pd: u16,
    /// BG3 Reference Point X-Coordinate
    pub bg3x: u32,
    /// BG3 Reference Point Y-Coordinate
    pub bg3y: u32,
    /// Window 0 Horizontal Dimensions
    pub win0h: u16,
    /// Window 1 Horizontal Dimensions
    pub win1h: u16,
    /// Window 0 Vertical Dimensions
    pub win0v: u16,
    /// Window 1 Vertical Dimensions
    pub win1v: u16,
    /// Inside of Window 0 and 1
    pub winin: u16,
    /// Inside of OBJ Window & Outside of Windows
    pub winout: u16,
    /// Mosaic Size
    pub mosaic: u16,
    /// Color Special Effects Selection
    pub bldcnt: u16,
    /// Alpha Blending Coefficients
    pub bldalpha: u16,
    /// Brightness (Fade-In/Out) Coefficient
    pub bldy: u16,
}

impl Registers {
    pub(super) fn get_bg0_enabled(&self) -> bool {
        self.dispcnt.get_bit(8)
    }

    pub(super) fn get_bg1_enabled(&self) -> bool {
        self.dispcnt.get_bit(9)
    }

    pub(super) fn get_bg2_enabled(&self) -> bool {
        self.dispcnt.get_bit(10)
    }

    pub(super) fn get_bg3_enabled(&self) -> bool {
        self.dispcnt.get_bit(11)
    }

    pub(super) fn get_obj_enabled(&self) -> bool {
        self.dispcnt.get_bit(12)
    }

    pub(super) fn get_win0_enabled(&self) -> bool {
        self.dispcnt.get_bit(13)
    }

    pub(super) fn get_win1_enabled(&self) -> bool {
        self.dispcnt.get_bit(14)
    }

    pub(super) fn get_winobj_enabled(&self) -> bool {
        self.dispcnt.get_bit(15)
    }

    /// Info about vram fields used to render display.
    pub(super) fn get_bg_mode(&self) -> u8 {
        self.dispcnt.get_bits(0..=2).try_into().unwrap()
    }

    pub(super) fn get_obj_character_vram_mapping(&self) -> ObjMappingKind {
        self.dispcnt.get_bit(6).into()
    }

    pub(super) fn get_vcount_setting(&self) -> u8 {
        self.dispstat.get_byte(1)
    }

    pub(super) fn get_vblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(3)
    }

    pub(super) fn get_hblank_irq_enable(&self) -> bool {
        self.dispstat.get_bit(4)
    }

    pub(super) fn get_vcounter_irq_enable(&self) -> bool {
        self.dispstat.get_bit(5)
    }

    pub(super) fn set_vblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(0, value);
    }

    pub(super) fn set_hblank_flag(&mut self, value: bool) {
        self.dispstat.set_bit(1, value);
    }

    pub(super) fn set_vcounter_flag(&mut self, value: bool) {
        self.dispstat.set_bit(2, value);
    }
}
