use crate::bitwise::Bits;

pub enum IORegisterAccessControl {
    Read,
    Write,
    ReadWrite,
}

pub struct IORegister {
    value: u32,
    access: IORegisterAccessControl,
}

impl IORegister {
    pub fn read(&self) -> u32 {
        match self.access {
            IORegisterAccessControl::Read | IORegisterAccessControl::ReadWrite => self.value,
            _ => panic!("Requested IO Register cannot be read."),
        }
    }

    pub fn write(&mut self, value: u32) {
        match self.access {
            IORegisterAccessControl::Write | IORegisterAccessControl::ReadWrite => {
                self.value = value;
            }

            _ => panic!("Requested IO Register cannot be written."),
        }
    }

    pub fn set_byte(&mut self, byte: u8, value: u8) {
        match self.access {
            IORegisterAccessControl::Write | IORegisterAccessControl::ReadWrite => {
                self.value.set_byte(byte, value);
            }

            _ => panic!("Requested IO Register cannot be written."),
        }
    }

    pub const fn with_access_control(access: IORegisterAccessControl) -> Self {
        Self { value: 0, access }
    }
}

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use IORegisterAccessControl::*;
    #[test]
    #[should_panic]
    fn write_only_register() {
        let reg = IORegister::with_access_control(Write);

        reg.read();
    }

    #[test]
    #[should_panic]
    fn read_only_register_write() {
        let mut reg = IORegister::with_access_control(Read);

        reg.write(5);
    }

    #[test]
    #[should_panic]
    fn read_only_register_set_byte() {
        let mut reg = IORegister::with_access_control(Read);

        reg.set_byte(3, 4);
    }

    #[test]
    fn test_ioregister() {
        let mut reg = IORegister::with_access_control(ReadWrite);

        assert_eq!(reg.read(), 0);

        reg.write(5);

        assert_eq!(reg.read(), 5);

        reg.set_byte(1, 3);

        assert_eq!(reg.read(), (3 << 8) | 5);
    }
}
