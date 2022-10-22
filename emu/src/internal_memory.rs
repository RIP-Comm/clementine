use crate::bitwise::Bits;
use crate::io_device::IoDevice;
use crate::io_registers::LCDRegisters;

pub struct InternalMemory {
    /// From 0x03000000 to 0x03007FFF (32kb).
    internal_work_ram: [u8; 0x7FFF],

    /// From 0x05000000 to  0x050001FF (512 bytes, 256 colors)
    bg_palette_ram: [u8; 0x1FF],

    /// From 0x05000200 to 0x050003FF (512 bytes, 256 colors)
    obj_palette_ram: [u8; 0x1FF],

    /// From 0x06000000 to 0x06017FFF (96 kb).
    video_ram: [u8; 0x17FFF],

    /// From 0x04000000 to 0x04000055 (0x56 bytes).
    lcd_registers: LCDRegisters,
}

impl Default for InternalMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl InternalMemory {
    pub const fn new() -> Self {
        Self {
            internal_work_ram: [0; 0x7FFF],
            bg_palette_ram: [0; 0x1FF],
            obj_palette_ram: [0; 0x1FF],
            video_ram: [0; 0x17FFF],
            lcd_registers: LCDRegisters::new(),
        }
    }

    fn write_address_lcd_register(&mut self, address: u32, value: u8) {
        match address {
            0x04000000 => self.lcd_registers.dispcnt.set_byte(0, value),
            0x04000001 => self.lcd_registers.dispcnt.set_byte(1, value),
            0x04000002 => self.lcd_registers.green_swap.set_byte(0, value),
            0x04000003 => self.lcd_registers.green_swap.set_byte(1, value),
            0x04000004 => self.lcd_registers.dispstat.set_byte(0, value),
            0x04000005 => self.lcd_registers.dispstat.set_byte(1, value),
            0x04000008 => self.lcd_registers.bg0cnt.set_byte(0, value),
            0x04000009 => self.lcd_registers.bg0cnt.set_byte(1, value),
            0x0400000A => self.lcd_registers.bg1cnt.set_byte(0, value),
            0x0400000B => self.lcd_registers.bg1cnt.set_byte(1, value),
            0x0400000C => self.lcd_registers.bg2cnt.set_byte(0, value),
            0x0400000D => self.lcd_registers.bg2cnt.set_byte(1, value),
            0x0400000E => self.lcd_registers.bg3cnt.set_byte(0, value),
            0x0400000F => self.lcd_registers.bg3cnt.set_byte(1, value),
            0x04000010 => self.lcd_registers.bg0hofs.set_byte(0, value),
            0x04000011 => self.lcd_registers.bg0hofs.set_byte(1, value),
            0x04000012 => self.lcd_registers.bg0vofs.set_byte(0, value),
            0x04000013 => self.lcd_registers.bg0vofs.set_byte(1, value),
            0x04000014 => self.lcd_registers.bg1hofs.set_byte(0, value),
            0x04000015 => self.lcd_registers.bg1hofs.set_byte(1, value),
            0x04000016 => self.lcd_registers.bg1vofs.set_byte(0, value),
            0x04000017 => self.lcd_registers.bg1vofs.set_byte(1, value),
            0x04000018 => self.lcd_registers.bg2hofs.set_byte(0, value),
            0x04000019 => self.lcd_registers.bg2hofs.set_byte(1, value),
            0x0400001A => self.lcd_registers.bg2vofs.set_byte(0, value),
            0x0400001B => self.lcd_registers.bg2vofs.set_byte(1, value),
            0x0400001C => self.lcd_registers.bg3hofs.set_byte(0, value),
            0x0400001D => self.lcd_registers.bg3hofs.set_byte(1, value),
            0x0400001E => self.lcd_registers.bg3vofs.set_byte(0, value),
            0x0400001F => self.lcd_registers.bg3vofs.set_byte(1, value),
            0x04000020 => self.lcd_registers.bg2pa.set_byte(0, value),
            0x04000021 => self.lcd_registers.bg2pa.set_byte(1, value),
            0x04000022 => self.lcd_registers.bg2pb.set_byte(0, value),
            0x04000023 => self.lcd_registers.bg2pb.set_byte(1, value),
            0x04000024 => self.lcd_registers.bg2pc.set_byte(0, value),
            0x04000025 => self.lcd_registers.bg2pc.set_byte(1, value),
            0x04000026 => self.lcd_registers.bg2pd.set_byte(0, value),
            0x04000027 => self.lcd_registers.bg2pd.set_byte(1, value),
            0x04000028 => self.lcd_registers.bg2x.set_byte(0, value),
            0x04000029 => self.lcd_registers.bg2x.set_byte(1, value),
            0x0400002A => self.lcd_registers.bg2x.set_byte(2, value),
            0x0400002B => self.lcd_registers.bg2x.set_byte(3, value),
            0x0400002C => self.lcd_registers.bg2y.set_byte(0, value),
            0x0400002D => self.lcd_registers.bg2y.set_byte(1, value),
            0x0400002E => self.lcd_registers.bg2y.set_byte(2, value),
            0x0400002F => self.lcd_registers.bg2y.set_byte(3, value),
            0x04000030 => self.lcd_registers.bg3pa.set_byte(0, value),
            0x04000031 => self.lcd_registers.bg3pa.set_byte(1, value),
            0x04000032 => self.lcd_registers.bg3pb.set_byte(0, value),
            0x04000033 => self.lcd_registers.bg3pb.set_byte(1, value),
            0x04000034 => self.lcd_registers.bg3pc.set_byte(0, value),
            0x04000035 => self.lcd_registers.bg3pc.set_byte(1, value),
            0x04000036 => self.lcd_registers.bg3pd.set_byte(0, value),
            0x04000037 => self.lcd_registers.bg3pd.set_byte(1, value),
            0x04000038 => self.lcd_registers.bg3x.set_byte(0, value),
            0x04000039 => self.lcd_registers.bg3x.set_byte(1, value),
            0x0400003A => self.lcd_registers.bg3x.set_byte(2, value),
            0x0400003B => self.lcd_registers.bg3x.set_byte(3, value),
            0x0400003C => self.lcd_registers.bg3y.set_byte(0, value),
            0x0400003D => self.lcd_registers.bg3y.set_byte(1, value),
            0x0400003E => self.lcd_registers.bg3y.set_byte(2, value),
            0x0400003F => self.lcd_registers.bg3y.set_byte(3, value),
            0x04000040 => self.lcd_registers.win0h.set_byte(0, value),
            0x04000041 => self.lcd_registers.win0h.set_byte(1, value),
            0x04000042 => self.lcd_registers.win1h.set_byte(0, value),
            0x04000043 => self.lcd_registers.win1h.set_byte(1, value),
            0x04000044 => self.lcd_registers.win0v.set_byte(0, value),
            0x04000045 => self.lcd_registers.win0v.set_byte(1, value),
            0x04000046 => self.lcd_registers.win1v.set_byte(0, value),
            0x04000047 => self.lcd_registers.win1v.set_byte(1, value),
            0x04000048 => self.lcd_registers.winin.set_byte(0, value),
            0x04000049 => self.lcd_registers.winin.set_byte(1, value),
            0x0400004A => self.lcd_registers.winout.set_byte(0, value),
            0x0400004B => self.lcd_registers.winout.set_byte(1, value),
            0x0400004C => self.lcd_registers.mosaic.set_byte(0, value),
            0x0400004D => self.lcd_registers.mosaic.set_byte(1, value),
            // 0x0400004E, 0x0400004F are not used
            0x04000050 => self.lcd_registers.bldcnt.set_byte(0, value),
            0x04000051 => self.lcd_registers.bldcnt.set_byte(1, value),
            0x04000052 => self.lcd_registers.bldalpha.set_byte(0, value),
            0x04000053 => self.lcd_registers.bldalpha.set_byte(1, value),
            0x04000054 => self.lcd_registers.bldy.set_byte(0, value),
            0x04000055 => self.lcd_registers.bldy.set_byte(1, value),
            _ => panic!("Writing an read-only memory address: {address:b}"),
        }
    }

    fn read_address_lcd_register(&self, address: u32) -> u8 {
        match address {
            0x04000000 => self.lcd_registers.dispcnt.read().get_byte(0),
            0x04000001 => self.lcd_registers.dispcnt.read().get_byte(1),
            0x04000002 => self.lcd_registers.green_swap.read().get_byte(0),
            0x04000003 => self.lcd_registers.green_swap.read().get_byte(1),
            0x04000004 => self.lcd_registers.dispstat.read().get_byte(0),
            0x04000005 => self.lcd_registers.dispstat.read().get_byte(1),
            0x04000006 => self.lcd_registers.vcount.read().get_byte(0),
            0x04000007 => self.lcd_registers.vcount.read().get_byte(1),
            0x04000008 => self.lcd_registers.bg0cnt.read().get_byte(0),
            0x04000009 => self.lcd_registers.bg0cnt.read().get_byte(1),
            0x0400000A => self.lcd_registers.bg1cnt.read().get_byte(0),
            0x0400000B => self.lcd_registers.bg1cnt.read().get_byte(1),
            0x0400000C => self.lcd_registers.bg2cnt.read().get_byte(0),
            0x0400000D => self.lcd_registers.bg2cnt.read().get_byte(1),
            0x0400000E => self.lcd_registers.bg3cnt.read().get_byte(0),
            0x0400000F => self.lcd_registers.bg3cnt.read().get_byte(1),
            0x04000048 => self.lcd_registers.winin.read().get_byte(0),
            0x04000049 => self.lcd_registers.winin.read().get_byte(1),
            0x0400004A => self.lcd_registers.winout.read().get_byte(0),
            0x0400004B => self.lcd_registers.winout.read().get_byte(1),
            0x04000050 => self.lcd_registers.bldcnt.read().get_byte(0),
            0x04000051 => self.lcd_registers.bldcnt.read().get_byte(1),
            0x04000052 => self.lcd_registers.bldalpha.read().get_byte(0),
            0x04000053 => self.lcd_registers.bldalpha.read().get_byte(1),
            _ => panic!("Reading an write-only memory address: {address:b}"),
        }
    }
}

impl IoDevice for InternalMemory {
    type Address = u32;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        match address {
            0x03000000..=0x03007FFF => self.internal_work_ram[(address - 0x03000000) as usize],
            0x04000000..=0x04000055 => self.read_address_lcd_register(address),
            0x05000000..=0x050001FF => self.bg_palette_ram[(address - 0x05000000) as usize],
            0x05000200..=0x050003FF => self.obj_palette_ram[(address - 0x05000200) as usize],
            0x06000000..=0x06017FFF => self.video_ram[(address - 0x06000000) as usize],
            _ => unimplemented!("Unimplemented memory region."),
        }
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        match address {
            0x03000000..=0x03007FFF => {
                self.internal_work_ram[(address - 0x03000000) as usize] = value
            }
            0x04000000..=0x04000055 => self.write_address_lcd_register(address, value),
            0x05000000..=0x050001FF => self.bg_palette_ram[(address - 0x05000000) as usize] = value,
            0x05000200..=0x050003FF => {
                self.obj_palette_ram[(address - 0x05000200) as usize] = value
            }
            0x06000000..=0x06017FFF => self.video_ram[(address - 0x06000000) as usize] = value,
            _ => unimplemented!("Unimplemented memory region {address}."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_work_ram() {
        let mut im = InternalMemory::new();

        let address = 0x03000005;
        im.write_at(address, 5);

        assert_eq!(im.internal_work_ram[5], 5);
    }

    #[test]
    fn test_read_work_ram() {
        let mut im = InternalMemory::new();
        im.internal_work_ram[5] = 10;

        let address = 0x03000005;
        assert_eq!(im.read_at(address), 10);
    }

    #[test]
    fn test_write_lcd_reg() {
        let mut im = InternalMemory::new();
        let address = 0x04000048; // WININ lower byte

        im.write_at(address, 10);

        assert_eq!(im.lcd_registers.winin.read(), 10);

        let address = 0x04000049; // WININ higher byte

        im.write_at(address, 5);
        assert_eq!(im.lcd_registers.winin.read(), (5 << 8) | 10);
    }

    #[test]
    fn test_read_lcd_reg() {
        let mut im = InternalMemory::new();
        let address = 0x04000048; // WININ lower byte

        im.lcd_registers.winin.write((5 << 8) | 10);

        assert_eq!(im.read_at(address), 10);

        let address = 0x04000049; // WININ higher byte

        assert_eq!(im.read_at(address), 5);
    }

    #[test]
    fn write_bg_palette_ram() {
        let mut im = InternalMemory::new();
        let address = 0x05000008;

        im.write_at(address, 10);
        assert_eq!(im.bg_palette_ram[8], 10);
    }

    #[test]
    fn read_bg_palette_ram() {
        let mut im = InternalMemory::new();
        im.bg_palette_ram[8] = 15;

        let address = 0x05000008;
        let value = im.read_at(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn write_obj_palette_ram() {
        let mut im = InternalMemory::new();
        let address = 0x05000208;

        im.write_at(address, 10);
        assert_eq!(im.obj_palette_ram[8], 10);
    }

    #[test]
    fn read_obj_palette_ram() {
        let mut im = InternalMemory::new();
        im.obj_palette_ram[8] = 15;

        let address = 0x05000208;

        let value = im.read_at(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn write_vram() {
        let mut im = InternalMemory::new();
        let address = 0x06000004;

        im.write_at(address, 23);
        assert_eq!(im.video_ram[4], 23);
    }

    #[test]
    fn read_vram() {
        let mut im = InternalMemory::new();
        im.video_ram[4] = 15;

        let address = 0x06000004;
        let value = im.read_at(address);

        assert_eq!(value, 15);
    }
}
