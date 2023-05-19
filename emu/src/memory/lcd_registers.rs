use std::collections::HashMap;

use logger::log;

use crate::{
    bitwise::Bits,
    memory::io_registers::{IORegister, IORegisterAccessControl},
};

use super::io_device::IoDevice;

pub struct LCDRegisters {
    /// LCD Control
    pub dispcnt: IORegister,
    /// Undocumented - Green Swap
    pub green_swap: IORegister,
    /// General LCD Status (STAT, LYC)
    pub dispstat: IORegister,
    /// Vertical Counter (LY)
    pub vcount: IORegister,
    /// BG0 Control
    pub bg0cnt: IORegister,
    /// BG1 Control
    pub bg1cnt: IORegister,
    /// BG2 Control
    pub bg2cnt: IORegister,
    /// BG3 Control
    pub bg3cnt: IORegister,
    /// BG0 X-Offset
    pub bg0hofs: IORegister,
    /// BG0 Y_Offset
    pub bg0vofs: IORegister,
    /// BG1 X-Offset
    pub bg1hofs: IORegister,
    /// BG1 Y_Offset
    pub bg1vofs: IORegister,
    /// BG2 X-Offset
    pub bg2hofs: IORegister,
    /// BG2 Y_Offset
    pub bg2vofs: IORegister,
    /// BG3 X-Offset
    pub bg3hofs: IORegister,
    /// BG3 Y_Offset
    pub bg3vofs: IORegister,
    /// BG2 Rotation/Scaling Parameter A (dx)
    pub bg2pa: IORegister,
    /// BG2 Rotation/Scaling Parameter B (dmx)
    pub bg2pb: IORegister,
    /// BG2 Rotation/Scaling Parameter C (dy)
    pub bg2pc: IORegister,
    /// BG2 Rotation/Scaling Parameter D (dmy)
    pub bg2pd: IORegister,
    /// BG2 Reference Point X-Coordinate
    pub bg2x: IORegister,
    /// BG2 Reference Point Y-Coordinate
    pub bg2y: IORegister,
    /// BG3 Rotation/Scaling Parameter A (dx)
    pub bg3pa: IORegister,
    /// BG3 Rotation/Scaling Parameter B (dmx)
    pub bg3pb: IORegister,
    /// BG3 Rotation/Scaling Parameter C (dy)
    pub bg3pc: IORegister,
    /// BG3 Rotation/Scaling Parameter D (dmy)
    pub bg3pd: IORegister,
    /// BG3 Reference Point X-Coordinate
    pub bg3x: IORegister,
    /// BG3 Reference Point Y-Coordinate
    pub bg3y: IORegister,
    /// Window 0 Horizontal Dimensions
    pub win0h: IORegister,
    /// Window 1 Horizontal Dimensions
    pub win1h: IORegister,
    /// Window 0 Vertical Dimensions
    pub win0v: IORegister,
    /// Window 1 Vertical Dimensions
    pub win1v: IORegister,
    /// Inside of Window 0 and 1
    pub winin: IORegister,
    /// Inside of OBJ Window & Outside of Windows
    pub winout: IORegister,
    /// Mosaic Size
    pub mosaic: IORegister,
    /// Color Special Effects Selection
    pub bldcnt: IORegister,
    /// Alpha Blending Coefficients
    pub bldalpha: IORegister,
    /// Brightness (Fade-In/Out) Coefficient
    pub bldy: IORegister,
    unused_region: HashMap<usize, u8>,
}

impl Default for LCDRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl LCDRegisters {
    pub fn new() -> Self {
        use IORegisterAccessControl::*;

        Self {
            dispcnt: IORegister::with_access_control(ReadWrite),
            green_swap: IORegister::with_access_control(ReadWrite),
            dispstat: IORegister::with_access_control(ReadWrite),
            vcount: IORegister::with_access_control(ReadWrite),
            bg0cnt: IORegister::with_access_control(ReadWrite),
            bg1cnt: IORegister::with_access_control(ReadWrite),
            bg2cnt: IORegister::with_access_control(ReadWrite),
            bg3cnt: IORegister::with_access_control(ReadWrite),
            bg0hofs: IORegister::with_access_control(Write),
            bg0vofs: IORegister::with_access_control(Write),
            bg1hofs: IORegister::with_access_control(Write),
            bg1vofs: IORegister::with_access_control(Write),
            bg2hofs: IORegister::with_access_control(Write),
            bg2vofs: IORegister::with_access_control(Write),
            bg3hofs: IORegister::with_access_control(Write),
            bg3vofs: IORegister::with_access_control(Write),
            bg2pa: IORegister::with_access_control(Write),
            bg2pb: IORegister::with_access_control(Write),
            bg2pc: IORegister::with_access_control(Write),
            bg2pd: IORegister::with_access_control(Write),
            bg2x: IORegister::with_access_control(Write),
            bg2y: IORegister::with_access_control(Write),
            bg3pa: IORegister::with_access_control(Write),
            bg3pb: IORegister::with_access_control(Write),
            bg3pc: IORegister::with_access_control(Write),
            bg3pd: IORegister::with_access_control(Write),
            bg3x: IORegister::with_access_control(Write),
            bg3y: IORegister::with_access_control(Write),
            win0h: IORegister::with_access_control(Write),
            win1h: IORegister::with_access_control(Write),
            win0v: IORegister::with_access_control(Write),
            win1v: IORegister::with_access_control(Write),
            winin: IORegister::with_access_control(ReadWrite),
            winout: IORegister::with_access_control(ReadWrite),
            mosaic: IORegister::with_access_control(Write),
            bldcnt: IORegister::with_access_control(ReadWrite),
            bldalpha: IORegister::with_access_control(ReadWrite),
            bldy: IORegister::with_access_control(Write),
            unused_region: HashMap::new(),
        }
    }

    /// Info about vram fields used to render display.
    pub fn get_bg_mode(&self) -> u8 {
        self.dispcnt.read().get_bits(0..=2).try_into().unwrap()
    }

    /// [false | true] = Gameboy Advance | Gameboy Color.
    /// It is a reserverd bit: only BIOS opcodes can write it.
    pub fn get_cgb_mode(&self) -> bool {
        self.dispcnt.read().is_bit_on(3)
    }

    /// Selected frame = [0-1]. Other values are not allowed.
    pub fn get_frame_select(&self) -> usize {
        self.dispcnt.read().is_bit_on(4).into()
    }

    /// True allows access to OAM during H-Blank.
    pub fn h_blank_interval_free(&self) -> bool {
        self.dispcnt.read().is_bit_on(5)
    }

    /// False means two dimensional
    pub fn obj_char_mapping_one_dimensional(&self) -> bool {
        self.dispcnt.read().is_bit_on(6)
    }

    /// True allows FAST access to VRAM, Palette, OAM
    pub fn forced_blank(&self) -> bool {
        self.dispcnt.read().is_bit_on(7)
    }

    // [false | true] = OFF | ON
    pub fn display_bg0(&self) -> bool {
        self.dispcnt.read().is_bit_on(8)
    }

    // [false | true] = OFF | ON
    pub fn display_bg1(&self) -> bool {
        self.dispcnt.read().is_bit_on(9)
    }

    // [false | true] = OFF | ON
    pub fn display_bg2(&self) -> bool {
        self.dispcnt.read().is_bit_on(10)
    }

    // [false | true] = OFF | ON
    pub fn display_bg3(&self) -> bool {
        self.dispcnt.read().is_bit_on(11)
    }

    // [false | true] = OFF | ON
    pub fn display_obj(&self) -> bool {
        self.dispcnt.read().is_bit_on(12)
    }

    // [false | true] = OFF | ON
    pub fn window0_display_flag(&self) -> bool {
        self.dispcnt.read().is_bit_on(13)
    }

    // [false | true] = OFF | ON
    pub fn window1_display_flag(&self) -> bool {
        self.dispcnt.read().is_bit_on(14)
    }

    // [false | true] = OFF | ON
    pub fn obj_window_display_flag(&self) -> bool {
        self.dispcnt.read().is_bit_on(15)
    }

    fn get_bg_index_cnt(&self, bg_index: usize) -> &IORegister {
        match bg_index {
            0 => &self.bg0cnt,
            1 => &self.bg1cnt,
            2 => &self.bg2cnt,
            3 => &self.bg3cnt,
            _ => panic!("Impossible BG index CNT!"),
        }
    }

    /// 0 = Highest
    pub fn get_bg_priority(&self, bg_index: usize) -> u8 {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().get_bits(0..=1).try_into().unwrap()
    }

    /// 0-3, in units of 16 `KBytes`.
    pub fn character_base_block(&self, bg_index: usize) -> u8 {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().get_bits(2..=3).try_into().unwrap()
    }

    /// [false | true]: Disabled | Enabled
    pub fn mosaic(&self, bg_index: usize) -> bool {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().is_bit_on(6)
    }

    /// Full means 1 palette/256 colors.
    /// Otherwise means 16 palette/16 colors.
    pub fn palette_full(&self, bg_index: usize) -> bool {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().is_bit_on(7)
    }

    /// 0-31, in units of 2 Kbytes.
    pub fn screen_base_block(&self, bg_index: usize) -> u8 {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().get_bits(8..=12).try_into().unwrap()
    }

    /// BG0/B1 not used.
    /// BG2/BG3: false = Wraparaound
    pub fn display_area_overflow_transparent(&self, bg_index: usize) -> bool {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().is_bit_on(13)
    }

    /// Screen size
    /// Value    Text Mode      Rotation/Scaling Mode
    ///   0      256x256 (2K)   128x128   (256 bytes)
    ///   1      512x256 (4K)   256x256   (1K)
    ///   2      256x512 (4K)   512x512   (4K)
    ///   3      512x512 (8K)   1024x1024 (16K)
    pub fn screen_size(&self, bg_index: usize) -> usize {
        let reg = self.get_bg_index_cnt(bg_index);
        reg.read().get_bits(14..=15).try_into().unwrap()
    }
}

impl IoDevice for LCDRegisters {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: usize) -> u8 {
        match address {
            0x04000000 => self.dispcnt.read().get_byte(0),
            0x04000001 => self.dispcnt.read().get_byte(1),
            0x04000002 => self.green_swap.read().get_byte(0),
            0x04000003 => self.green_swap.read().get_byte(1),
            0x04000004 => self.dispstat.read().get_byte(0),
            0x04000005 => self.dispstat.read().get_byte(1),
            0x04000006 => self.vcount.read().get_byte(0),
            0x04000007 => self.vcount.read().get_byte(1),
            0x04000008 => self.bg0cnt.read().get_byte(0),
            0x04000009 => self.bg0cnt.read().get_byte(1),
            0x0400000A => self.bg1cnt.read().get_byte(0),
            0x0400000B => self.bg1cnt.read().get_byte(1),
            0x0400000C => self.bg2cnt.read().get_byte(0),
            0x0400000D => self.bg2cnt.read().get_byte(1),
            0x0400000E => self.bg3cnt.read().get_byte(0),
            0x0400000F => self.bg3cnt.read().get_byte(1),
            0x04000048 => self.winin.read().get_byte(0),
            0x04000049 => self.winin.read().get_byte(1),
            0x0400004A => self.winout.read().get_byte(0),
            0x0400004B => self.winout.read().get_byte(1),
            0x04000050 => self.bldcnt.read().get_byte(0),
            0x04000051 => self.bldcnt.read().get_byte(1),
            0x04000052 => self.bldalpha.read().get_byte(0),
            0x04000053 => self.bldalpha.read().get_byte(1),
            0x0400004E..=0x0400004F | 0x04000056..=0x0400005F => {
                log("read on unused memory");
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("Reading an write-only memory address: {address:b}"),
        }
    }

    fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x04000000 => self.dispcnt.set_byte(0, value),
            0x04000001 => self.dispcnt.set_byte(1, value),
            0x04000002 => self.green_swap.set_byte(0, value),
            0x04000003 => self.green_swap.set_byte(1, value),
            0x04000004 => self.dispstat.set_byte(0, value),
            0x04000005 => self.dispstat.set_byte(1, value),
            0x04000008 => self.bg0cnt.set_byte(0, value),
            0x04000006 => self.vcount.set_byte(0, value),
            0x04000007 => self.vcount.set_byte(1, value),
            0x04000009 => self.bg0cnt.set_byte(1, value),
            0x0400000A => self.bg1cnt.set_byte(0, value),
            0x0400000B => self.bg1cnt.set_byte(1, value),
            0x0400000C => self.bg2cnt.set_byte(0, value),
            0x0400000D => self.bg2cnt.set_byte(1, value),
            0x0400000E => self.bg3cnt.set_byte(0, value),
            0x0400000F => self.bg3cnt.set_byte(1, value),
            0x04000010 => self.bg0hofs.set_byte(0, value),
            0x04000011 => self.bg0hofs.set_byte(1, value),
            0x04000012 => self.bg0vofs.set_byte(0, value),
            0x04000013 => self.bg0vofs.set_byte(1, value),
            0x04000014 => self.bg1hofs.set_byte(0, value),
            0x04000015 => self.bg1hofs.set_byte(1, value),
            0x04000016 => self.bg1vofs.set_byte(0, value),
            0x04000017 => self.bg1vofs.set_byte(1, value),
            0x04000018 => self.bg2hofs.set_byte(0, value),
            0x04000019 => self.bg2hofs.set_byte(1, value),
            0x0400001A => self.bg2vofs.set_byte(0, value),
            0x0400001B => self.bg2vofs.set_byte(1, value),
            0x0400001C => self.bg3hofs.set_byte(0, value),
            0x0400001D => self.bg3hofs.set_byte(1, value),
            0x0400001E => self.bg3vofs.set_byte(0, value),
            0x0400001F => self.bg3vofs.set_byte(1, value),
            0x04000020 => self.bg2pa.set_byte(0, value),
            0x04000021 => self.bg2pa.set_byte(1, value),
            0x04000022 => self.bg2pb.set_byte(0, value),
            0x04000023 => self.bg2pb.set_byte(1, value),
            0x04000024 => self.bg2pc.set_byte(0, value),
            0x04000025 => self.bg2pc.set_byte(1, value),
            0x04000026 => self.bg2pd.set_byte(0, value),
            0x04000039 => self.bg3x.set_byte(1, value),
            0x04000027 => self.bg2pd.set_byte(1, value),
            0x04000028 => self.bg2x.set_byte(0, value),
            0x04000029 => self.bg2x.set_byte(1, value),
            0x0400002A => self.bg2x.set_byte(2, value),
            0x0400002B => self.bg2x.set_byte(3, value),
            0x0400002C => self.bg2y.set_byte(0, value),
            0x0400002D => self.bg2y.set_byte(1, value),
            0x0400002E => self.bg2y.set_byte(2, value),
            0x0400002F => self.bg2y.set_byte(3, value),
            0x04000030 => self.bg3pa.set_byte(0, value),
            0x04000031 => self.bg3pa.set_byte(1, value),
            0x04000032 => self.bg3pb.set_byte(0, value),
            0x04000033 => self.bg3pb.set_byte(1, value),
            0x04000034 => self.bg3pc.set_byte(0, value),
            0x04000035 => self.bg3pc.set_byte(1, value),
            0x04000036 => self.bg3pd.set_byte(0, value),
            0x04000037 => self.bg3pd.set_byte(1, value),
            0x04000038 => self.bg3x.set_byte(0, value),
            0x0400003A => self.bg3x.set_byte(2, value),
            0x0400003B => self.bg3x.set_byte(3, value),
            0x0400003C => self.bg3y.set_byte(0, value),
            0x0400003D => self.bg3y.set_byte(1, value),
            0x0400003E => self.bg3y.set_byte(2, value),
            0x0400003F => self.bg3y.set_byte(3, value),
            0x04000040 => self.win0h.set_byte(0, value),
            0x04000041 => self.win0h.set_byte(1, value),
            0x04000042 => self.win1h.set_byte(0, value),
            0x04000043 => self.win1h.set_byte(1, value),
            0x04000044 => self.win0v.set_byte(0, value),
            0x04000045 => self.win0v.set_byte(1, value),
            0x04000046 => self.win1v.set_byte(0, value),
            0x04000047 => self.win1v.set_byte(1, value),
            0x04000048 => self.winin.set_byte(0, value),
            0x04000049 => self.winin.set_byte(1, value),
            0x0400004A => self.winout.set_byte(0, value),
            0x0400004B => self.winout.set_byte(1, value),
            0x0400004C => self.mosaic.set_byte(0, value),
            0x0400004D => self.mosaic.set_byte(1, value),
            // 0x0400004E, 0x0400004F are not used
            0x04000050 => self.bldcnt.set_byte(0, value),
            0x04000051 => self.bldcnt.set_byte(1, value),
            0x04000052 => self.bldalpha.set_byte(0, value),
            0x04000053 => self.bldalpha.set_byte(1, value),
            0x04000054 => self.bldy.set_byte(0, value),
            0x04000055 => self.bldy.set_byte(1, value),
            0x0400004E..=0x0400004F | 0x04000056..=0x0400005F => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            _ => panic!("Writing an read-only memory address: {address:x}"),
        }
    }
}
