use std::collections::HashMap;

use logger::log;

use crate::memory::io_device::IoDevice;
use crate::memory::lcd_registers::LCDRegisters;
use crate::memory::timer_registers::TimerRegisters;

use super::interrupts::Interrupts;

pub struct InternalMemory {
    /// From 0x00000000 to 0x00003FFF (16 KBytes).
    bios_system_rom: Vec<u8>,

    /// From 0x02000000 to 0x0203FFFF (256 KBytes).
    working_ram: Vec<u8>,

    /// From 0x03000000 to 0x03007FFF (32kb).
    working_iram: Vec<u8>,

    /// From 0x04000000 to 0x04000055 (0x56 bytes).
    pub lcd_registers: LCDRegisters,

    /// From 0x04000100 to 0x0400010E.
    timer_registers: TimerRegisters,

    /// From 0x04000200 to 040003FE
    interrupts: Interrupts,

    /// From 0x05000000 to  0x050001FF (512 bytes, 256 colors).
    pub bg_palette_ram: Vec<u8>,

    /// From 0x05000200 to 0x050003FF (512 bytes, 256 colors).
    pub obj_palette_ram: Vec<u8>,

    /// From 0x06000000 to 0x06017FFF (96 kb).
    pub video_ram: Vec<u8>,

    /// From 0x08000000 to 0x0FFFFFFF.
    /// Basically here you can find different kind of rom loaded.
    // TODO: Not sure if we should split this into
    // 08000000-09FFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 0
    // 0A000000-0BFFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 1
    // 0C000000-0DFFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 2
    // 0E000000-0E00FFFF Game Pak SRAM (max 64 KBytes) - 8bit Bus width
    // 0E010000-0FFFFFFF Not used
    pub rom: Vec<u8>,

    /// From 0x00004000 to 0x01FFFFFF.
    /// From 0x10000000 to 0xFFFFFFFF.
    unused_region: HashMap<usize, u8>,
}

impl Default for InternalMemory {
    fn default() -> Self {
        Self::new([0_u8; 0x00004000], vec![])
    }
}

impl InternalMemory {
    pub fn new(bios: [u8; 0x00004000], rom: Vec<u8>) -> Self {
        Self {
            bios_system_rom: bios.into(),
            working_ram: vec![0; 0x00040000],
            working_iram: vec![0; 0x00008000],
            lcd_registers: LCDRegisters::default(),
            timer_registers: TimerRegisters::default(),
            interrupts: Interrupts::default(),
            bg_palette_ram: vec![0; 0x200],
            obj_palette_ram: vec![0; 0x200],
            video_ram: vec![0; 0x00018000],
            rom,
            unused_region: HashMap::new(),
        }
    }
}

impl IoDevice for InternalMemory {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: Self::Address) -> Self::Value {
        match address {
            0x00000000..=0x00003FFF => self.bios_system_rom[address],
            0x02000000..=0x0203FFFF => self.working_ram[address - 0x02000000],
            0x03000000..=0x03007FFF => self.working_iram[address - 0x03000000],
            0x04000000..=0x04000055 => self.lcd_registers.read_at(address),
            0x04000100..=0x0400010E => self.timer_registers.read_at(address),
            0x04000200..=0x040003FE => self.interrupts.read_at(address),
            0x05000000..=0x050001FF => self.bg_palette_ram[address - 0x05000000],
            0x05000200..=0x050003FF => self.obj_palette_ram[address - 0x05000200],
            0x06000000..=0x06017FFF => self.video_ram[address - 0x06000000],
            0x08000000..=0x0FFFFFFF => self.rom[address - 0x08000000],
            0x03008000..=0x03FFFFFF | 0x00004000..=0x01FFFFFF | 0x10000000..=0xFFFFFFFF => {
                log("read on unused memory");
                self.unused_region
                    .get(&(address as usize))
                    .map_or(0, |v| *v)
            }
            _ => unimplemented!("Unimplemented memory region. {address:x}"),
        }
    }

    fn write_at(&mut self, address: Self::Address, value: Self::Value) {
        match address {
            0x00000000..=0x00003FFF => self.bios_system_rom[address] = value,
            0x02000000..=0x0203FFFF => self.working_ram[address - 0x02000000] = value,
            0x03000000..=0x03007FFF => self.working_iram[address - 0x03000000] = value,
            0x04000000..=0x04000055 => self.lcd_registers.write_at(address, value),
            0x04000100..=0x0400010E => self.timer_registers.write_at(address, value),
            0x04000200..=0x040003FE => self.interrupts.write_at(address, value),
            0x05000000..=0x050001FF => self.bg_palette_ram[address - 0x05000000] = value,
            0x05000200..=0x050003FF => self.obj_palette_ram[address - 0x05000200] = value,
            0x06000000..=0x06017FFF => self.video_ram[address - 0x06000000] = value,
            0x03008000..=0x03FFFFFF | 0x00004000..=0x01FFFFFF | 0x10000000..=0xFFFFFFFF => {
                log("write on unused memory");
                if self.unused_region.insert(address, value).is_some() {}
            }
            0x08000000..=0x0FFFFFFF => {
                self.rom[address - 0x08000000] = value;
            }

            _ => unimplemented!("Unimplemented memory region {address:x}."),
        }
    }
}

impl InternalMemory {
    pub fn read_word(&self, address: usize) -> u32 {
        let part_0: u32 = self.read_at(address).try_into().unwrap();
        let part_1: u32 = self.read_at(address + 1).try_into().unwrap();
        let part_2: u32 = self.read_at(address + 2).try_into().unwrap();
        let part_3: u32 = self.read_at(address + 3).try_into().unwrap();

        part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_work_ram() {
        let mut im = InternalMemory::default();

        let address = 0x03000005;
        im.write_at(address, 5);

        assert_eq!(im.working_iram[5], 5);
    }

    #[test]
    fn test_last_byte_work_ram() {
        let mut im = InternalMemory::default();

        let address = 0x03007FFF;
        im.write_at(address, 5);

        assert_eq!(im.working_iram[0x7FFF], 5);
    }

    #[test]
    fn test_read_work_ram() {
        let mut im = InternalMemory::default();
        im.working_iram[5] = 10;

        let address = 0x03000005;
        assert_eq!(im.read_at(address), 10);
    }

    #[test]
    fn test_write_lcd_reg() {
        let mut im = InternalMemory::default();
        let address = 0x04000048; // WININ lower byte

        im.write_at(address, 10);

        assert_eq!(im.lcd_registers.winin.read(), 10);

        let address = 0x04000049; // WININ higher byte

        im.write_at(address, 5);
        assert_eq!(im.lcd_registers.winin.read(), (5 << 8) | 10);
    }

    #[test]
    fn test_read_lcd_reg() {
        let mut im = InternalMemory::default();
        let address = 0x04000048; // WININ lower byte

        im.lcd_registers.winin.write((5 << 8) | 10);

        assert_eq!(im.read_at(address), 10);

        let address = 0x04000049; // WININ higher byte

        assert_eq!(im.read_at(address), 5);
    }

    #[test]
    fn write_bg_palette_ram() {
        let mut im = InternalMemory::default();
        let address = 0x05000008;

        im.write_at(address, 10);
        assert_eq!(im.bg_palette_ram[8], 10);
    }

    #[test]
    fn read_bg_palette_ram() {
        let mut im = InternalMemory::default();
        im.bg_palette_ram[8] = 15;

        let address = 0x05000008;
        let value = im.read_at(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_bg_palette_ram() {
        let mut im = InternalMemory::default();

        let address = 0x050001FF;
        im.write_at(address, 5);

        assert_eq!(im.bg_palette_ram[0x1FF], 5);
    }

    #[test]
    fn write_obj_palette_ram() {
        let mut im = InternalMemory::default();
        let address = 0x05000208;

        im.write_at(address, 10);
        assert_eq!(im.obj_palette_ram[8], 10);
    }

    #[test]
    fn read_obj_palette_ram() {
        let mut im = InternalMemory::default();
        im.obj_palette_ram[8] = 15;

        let address = 0x05000208;

        let value = im.read_at(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_obj_palette_ram() {
        let mut im = InternalMemory::default();

        let address = 0x050003FF;
        im.write_at(address, 5);

        assert_eq!(im.obj_palette_ram[0x1FF], 5);
    }

    #[test]
    fn write_vram() {
        let mut im = InternalMemory::default();
        let address = 0x06000004;

        im.write_at(address, 23);
        assert_eq!(im.video_ram[4], 23);
    }

    #[test]
    fn read_vram() {
        let mut im = InternalMemory::default();
        im.video_ram[4] = 15;

        let address = 0x06000004;
        let value = im.read_at(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_vram() {
        let mut im = InternalMemory::default();

        let address = 0x06017FFF;
        im.write_at(address, 5);

        assert_eq!(im.video_ram[0x17FFF], 5);
    }

    #[test]
    fn test_read_write_bios_memory() {
        let mut im = InternalMemory::default();
        im.write_at(0x000001EC, 10);
        assert_eq!(im.read_at(0x000001EC), 10);
    }

    #[test]
    fn test_write_timer_register() {
        let mut im = InternalMemory::default();
        let address = 0x04000100;

        im.write_at(address, 10);
        assert_eq!(im.timer_registers.tm0cnt_l.read(), 10);
    }

    #[test]
    fn test_read_timer_register() {
        let mut im = InternalMemory::default();
        let address = 0x04000100;

        im.timer_registers.tm0cnt_l.write((5 << 8) | 10);

        assert_eq!(im.read_at(address), 10);
    }

    #[test]
    fn test_read_rom() {
        let mut im = InternalMemory::default();
        im.rom = vec![1, 1, 1, 1];
        let address = 0x08000000;
        assert_eq!(im.read_at(address), 1);
    }
}
