use crate::{
    bitwise::Bits,
    memory::io_registers::{IORegister, IORegisterAccessControl},
};

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
}

impl Default for LCDRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl LCDRegisters {
    pub const fn new() -> Self {
        use IORegisterAccessControl::*;

        Self {
            dispcnt: IORegister::with_access_control(ReadWrite),
            green_swap: IORegister::with_access_control(ReadWrite),
            dispstat: IORegister::with_access_control(ReadWrite),
            vcount: IORegister::with_access_control(Read),
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
        }
    }

    /// Info about vram fields used to render display.
    pub fn get_bg_mode(&self) -> u8 {
        self.dispcnt.read().get_bits(0..=2) as u8
    }

    /// [false | true] = Gameboy Advance | Gameboy Color.
    /// It is a reserverd bit: only BIOS opcodes can write it.
    pub fn get_cgb_mode(&self) -> bool {
        self.dispcnt.read().is_bit_on(3)
    }

    /// Selected frame = [0-1]. Other values are not allowed.
    pub fn get_frame_select(&self) -> u8 {
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
}
