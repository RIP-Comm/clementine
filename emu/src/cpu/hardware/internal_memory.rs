use std::collections::HashMap;

use logger::log;
use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

use super::get_unmasked_address;

#[derive(Serialize, Deserialize)]
pub struct InternalMemory {
    /// From 0x00000000 to 0x00003FFF (16 `KBytes`).
    bios_system_rom: Vec<u8>,

    /// From 0x02000000 to 0x0203FFFF (256 `KBytes`).
    working_ram: Vec<u8>,

    /// From 0x03000000 to 0x03007FFF (32kb).
    working_iram: Vec<u8>,

    /// From 0x08000000 to 0x0FFFFFFF.
    /// Basically here you can find different kind of rom loaded.
    // TODO: Not sure if we should split this into
    // 08000000-09FFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 0
    // 0A000000-0BFFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 1
    // 0C000000-0DFFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 2
    // 0E000000-0E00FFFF Game Pak SRAM (max 64 KBytes) - 8bit Bus width
    // 0E010000-0FFFFFFF Not used
    pub rom: Vec<u8>,

    /// From 0x00004000 to `0x01FF_FFFF`.
    /// From 0x10000000 to `0xFFFF_FFFF`.
    unused_region: HashMap<usize, u8>,
}

impl Default for InternalMemory {
    fn default() -> Self {
        Self::new([0_u8; 0x0000_4000], vec![])
    }
}

impl InternalMemory {
    #[must_use]
    pub fn new(bios: [u8; 0x0000_4000], rom: Vec<u8>) -> Self {
        Self {
            bios_system_rom: bios.to_vec(),
            working_ram: vec![0; 0x0004_0000],
            working_iram: vec![0; 0x0000_8000],
            rom,
            unused_region: HashMap::new(),
        }
    }

    fn read_rom(&self, address: usize) -> u8 {
        if address < self.rom.len() {
            self.rom[address]
        } else {
            // Preamble:
            // The GamePak ROM is an halfword addressable memory
            // and it uses a 16bits bus to transfer data and a
            // 24bits(32MB halfword addressed) bus to transfer the address to read.
            // So technically we can't just read 1 byte from the ROM, we
            // request the halfword and then we take the upper/lower 8bits
            // depending on the address least significant bit.
            //
            // https://rust-console.github.io/gbatek-gbaonly/#auxgbagamepakbus
            // In GamePak ROM, the 16bits data and the
            // lower 16bits of the address are transferred on the same bus (AD0-15),
            // the higher 8bits of the address (24bits in total, remember halfword addressing)
            // are transferred via A16-23.
            // When requesting an address which is "empty", the GamePak ROM doesn't overwrite the
            // value present in the AD0-15 bus, which then will still contain the lower 16bits of the address.
            // CPU will then use this as if it was the value read from the ROM.
            //
            // Here we get the 24bits address (halfword addressing) by shifting right by 1
            // and we take only the 16 lower bits. We use this as if it was the value read from the ROM
            // and we get the 0 or 1 byte depending on the LSB in the address.
            (((address >> 1) & 0xFFFF) as u16).get_byte((address & 0b1) as u8)
        }
    }
}

impl InternalMemory {
    #[must_use]
    pub fn read_at(&self, address: usize) -> u8 {
        match address {
            0x0000_0000..=0x0000_3FFF => self.bios_system_rom[address],
            0x0200_0000..=0x02FF_FFFF => {
                self.working_ram
                    [get_unmasked_address(address, 0x00FF_0000, 0xFF00_FFFF, 16, 4) - 0x0200_0000]
            }
            0x0300_0000..=0x03FF_FFFF => {
                self.working_iram
                    [get_unmasked_address(address, 0x00FF_F000, 0xFF00_0FFF, 12, 8) - 0x0300_0000]
            }
            0x0800_0000..=0x09FF_FFFF => self.read_rom(address - 0x0800_0000),
            0x0A00_0000..=0x0BFF_FFFF => self.read_rom(address - 0x0A00_0000),
            0x0C00_0000..=0x0DFF_FFFF => self.read_rom(address - 0x0C00_0000),
            0x0E00_0000..=0x0E00_FFFF => unimplemented!("SRAM region is unimplemented"),
            0x0000_4000..=0x01FF_FFFF | 0x1000_0000..=0xFFFF_FFFF => {
                log(format!("read on unused memory {address:x}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => unimplemented!("Unimplemented memory region. {address:x}"),
        }
    }

    pub fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x0000_0000..=0x0000_3FFF => self.bios_system_rom[address] = value,
            0x0200_0000..=0x0203_FFFF => self.working_ram[address - 0x0200_0000] = value,
            // Mirror
            0x0204_0000..=0x02FF_FFFF => {
                self.working_ram[get_unmasked_address(address, 0x00FF_0000, 0xFF00_FFFF, 16, 4)
                    - 0x0200_0000] = value;
            }
            0x0300_0000..=0x0300_7FFF => self.working_iram[address - 0x0300_0000] = value,
            // Mirror
            0x0300_8000..=0x03FF_FFFF => {
                self.working_iram[get_unmasked_address(address, 0x00FF_F000, 0xFF00_0FFF, 12, 8)
                    - 0x0300_0000] = value;
            }
            0x0800_0000..=0x0FFF_FFFF => {
                // TODO: this should be split
                self.rom[address - 0x0800_0000] = value;
            }
            _ => unimplemented!("Unimplemented memory region {address:x}."),
        }
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
    fn test_read_write_bios_memory() {
        let mut im = InternalMemory::default();
        im.write_at(0x000001EC, 10);
        assert_eq!(im.read_at(0x000001EC), 10);
    }

    #[test]
    fn test_read_rom() {
        let im = InternalMemory {
            rom: vec![1, 2, 3, 4],
            ..Default::default()
        };
        let address = 0x08000000;
        assert_eq!(im.read_at(address), 1);

        // Testing reading in empty rom
        let address = 0x09FF_FFFF;
        assert_eq!(im.read_at(address), 0xFF);

        let address = 0x09FF_FFEE;
        assert_eq!(im.read_at(address), 0xF7);

        let address = 0x09FF_FFEF;
        assert_eq!(im.read_at(address), 0xFF);
    }

    #[test]
    fn test_mirror_3ffffxx() {
        let mut im = InternalMemory::default();
        im.working_iram[0x7FF0] = 5;

        assert_eq!(im.read_at(0x3FFFFF0), 5);

        im.write_at(0x3FFFFA0, 10);

        assert_eq!(im.working_iram[0x7FA0], 10);
    }

    #[test]
    fn test_mirror_wram() {
        let mut im = InternalMemory::default();
        im.working_ram[0x010003] = 5;

        assert_eq!(im.read_at(0x02010003), 5);
        assert_eq!(im.read_at(0x02050003), 5);
        assert_eq!(im.read_at(0x02350003), 5);
        assert_eq!(im.read_at(0x02F50003), 5);

        im.write_at(0x02010003, 2);
        assert_eq!(im.working_ram[0x010003], 2);

        im.write_at(0x02050003, 1);
        assert_eq!(im.working_ram[0x010003], 1);

        im.write_at(0x02350010, 1);
        assert_eq!(im.working_ram[0x010010], 1);

        im.write_at(0x02F5003F, 1);
        assert_eq!(im.working_ram[0x01003F], 1);
    }

    #[test]
    fn test_mirror_iram() {
        let mut im = InternalMemory::default();
        im.working_iram[0x21FF] = 5;

        assert_eq!(im.read_at(0x030021FF), 5);
        assert_eq!(im.read_at(0x0300A1FF), 5);
        assert_eq!(im.read_at(0x030121FF), 5);
        assert_eq!(im.read_at(0x03FFA1FF), 5);

        im.write_at(0x030021FF, 2);
        assert_eq!(im.working_iram[0x21FF], 2);

        im.write_at(0x0300A1FF, 1);
        assert_eq!(im.working_iram[0x21FF], 1);

        im.write_at(0x030171FF, 10);
        assert_eq!(im.working_iram[0x71FF], 10);

        im.write_at(0x03FFF1FF, 1);
        assert_eq!(im.working_iram[0x71FF], 1);
    }
}
