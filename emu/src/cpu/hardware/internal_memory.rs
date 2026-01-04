//! Internal memory storage: BIOS, RAM, ROM, and Flash.
//!
//! This module implements the GBA's main memory regions that store code and data.
//! The [`InternalMemory`] struct holds the actual byte arrays for each region.
//!
//! # Memory Regions
//!
//! | Region       | Address Range           | Size   | Description                      |
//! |--------------|-------------------------|--------|----------------------------------|
//! | BIOS         | `0x0000_0000-0000_3FFF` | 16 KB  | System ROM (read-only)           |
//! | WRAM         | `0x0200_0000-0203_FFFF` | 256 KB | Work RAM (mirrored every 256KB)  |
//! | IWRAM        | `0x0300_0000-0300_7FFF` | 32 KB  | Internal Work RAM (fast, mirrored) |
//! | ROM          | `0x0800_0000-0DFF_FFFF` | 32 MB  | Game Pak ROM (3 wait states)     |
//! | SRAM/Flash   | `0x0E00_0000-0E01_FFFF` | 128 KB | Save data storage                |
//!
//! # Address Mirroring
//!
//! RAM regions mirror throughout their address space:
//! - **WRAM**: Mirrors every 256KB (`0x0204_0000` = `0x0200_0000`)
//! - **IWRAM**: Mirrors every 32KB (`0x0300_8000` = `0x0300_0000`)
//!
//! # Flash Memory State Machine
//!
//! The Flash save memory uses a command-based state machine ([`FlashState`]) to handle:
//! - **ID Mode**: Returns manufacturer/device ID for detection
//! - **Erase**: Chip erase or 4KB sector erase
//! - **Write**: Single byte programming (can only clear bits)
//! - **Bank Select**: Switch between 64KB banks (for 128KB flash)
//!
//! Commands use a specific sequence written to addresses `0x5555` and `0x2AAA`.
//!
//! # GPIO (RTC Support)
//!
//! The module also handles GPIO registers at ROM offset `0xC4-0xC9` used by some
//! games (like Pokemon) for Real-Time Clock communication:
//! - `0xC4`: Data register (pin state)
//! - `0xC6`: Direction register (1=output, 0=input)
//! - `0xC8`: Control register (GPIO enable)
//!
//! # Empty ROM Reads
//!
//! When reading past the end of the loaded ROM, the GBA returns the lower 16 bits
//! of the requested address (due to how the Game Pak bus works). This is emulated
//! in `read_rom`.

#![allow(clippy::unreadable_literal)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

use super::get_unmasked_address;

/// Flash memory state for command handling
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashState {
    #[default]
    Ready,
    Command1,      // Received 0xAA at 0x5555
    Command2,      // Received 0x55 at 0x2AAA
    IdMode,        // ID mode - reads return manufacturer/device ID
    EraseCommand,  // Received 0x80 - waiting for erase sequence
    EraseCommand1, // Erase: received 0xAA at 0x5555
    EraseCommand2, // Erase: received 0x55 at 0x2AAA, waiting for erase type
    BankSelect,    // Waiting for bank number (for 128KB flash)
    WriteCommand,  // Ready to write a byte
}

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

    /// From 0x0E000000 to 0x0E01FFFF (128 `KBytes` for Flash).
    /// Game Pak SRAM/Flash used for save data.
    sram: Vec<u8>,

    /// Flash memory state machine
    flash_state: FlashState,

    /// Flash bank selection for 128KB flash (0 or 1)
    flash_bank: u8,

    /// GPIO registers for RTC/rumble/etc (at ROM offset 0xC4-0xC9)
    /// Register layout: 0xC4=data, 0xC6=direction, 0xC8=control
    gpio_data: u16, // Pin state (4-bit)
    gpio_direction: u16, // Pin direction (4-bit, 1=output, 0=input)
    gpio_control: u16,   // GPIO enable/control (1-bit)

    /// From 0x00004000 to `0x01FF_FFFF`.
    /// From 0x10000000 to `0xFFFF_FFFF`.
    unused_region: HashMap<usize, u8>,
}

impl InternalMemory {
    #[must_use]
    pub fn new(bios: [u8; 0x0000_4000], rom: &[u8]) -> Self {
        Self {
            bios_system_rom: bios.to_vec(),
            working_ram: vec![0; 0x0004_0000],
            working_iram: vec![0; 0x0000_8000],
            rom: rom.to_vec(),
            sram: vec![0xFF; 0x0002_0000], // 128KB Flash, initialized to 0xFF (erased state)
            flash_state: FlashState::Ready,
            flash_bank: 0,
            gpio_data: 0,      // All pins low initially
            gpio_direction: 0, // All pins as inputs initially
            gpio_control: 1,   // GPIO enabled (allow reads)
            unused_region: HashMap::new(),
        }
    }
}

impl Default for InternalMemory {
    /// Creates an `InternalMemory` with properly-sized memory regions.
    ///
    /// This is primarily used for testing. For actual emulation, use
    /// [`InternalMemory::new`] with real BIOS and ROM data.
    fn default() -> Self {
        Self {
            bios_system_rom: vec![0; 0x0000_4000], // 16 KB BIOS
            working_ram: vec![0; 0x0004_0000],     // 256 KB EWRAM
            working_iram: vec![0; 0x0000_8000],    // 32 KB IWRAM
            rom: vec![0; 0x0200_0000],             // 32 MB ROM (max size)
            sram: vec![0xFF; 0x0002_0000],         // 128 KB Flash
            flash_state: FlashState::Ready,
            flash_bank: 0,
            gpio_data: 0,
            gpio_direction: 0,
            gpio_control: 1,
            unused_region: HashMap::new(),
        }
    }
}

impl InternalMemory {
    fn read_rom(&self, address: usize) -> u8 {
        // GPIO port region (for RTC in Pokemon Fire Red/Leaf Green)
        // Located at ROM addresses 0xC4-0xC9 (16-bit aligned)
        // 0xC4/0xC5 = Data register (pin state)
        // 0xC6/0xC7 = Direction register
        // 0xC8/0xC9 = Control register
        if (0xC4..=0xC9).contains(&address) {
            let value = match address {
                0xC4 => self.gpio_data.get_byte(0),
                0xC5 => self.gpio_data.get_byte(1),
                0xC6 => self.gpio_direction.get_byte(0),
                0xC7 => self.gpio_direction.get_byte(1),
                0xC8 => self.gpio_control.get_byte(0),
                0xC9 => self.gpio_control.get_byte(1),
                _ => unreachable!(),
            };
            tracing::debug!(
                "GPIO READ: offset 0x{:04X} = 0x{:02X} (data=0x{:04X}, dir=0x{:04X}, ctrl=0x{:04X})",
                address,
                value,
                self.gpio_data,
                self.gpio_direction,
                self.gpio_control
            );
            return value;
        }

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
            #[allow(clippy::cast_possible_truncation)]
            {
                (((address >> 1) & 0xFFFF) as u16).get_byte((address & 0b1) as u8)
            }
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
                let unmasked = get_unmasked_address(address, 0x00FF_F000, 0xFF00_0FFF, 12, 8);
                let idx = unmasked - 0x0300_0000;
                let value = self.working_iram[idx];

                // Debug: Log reads around the problematic address
                if (0x0300_36A0..=0x0300_36B0).contains(&unmasked) {
                    tracing::debug!(
                        "IWRAM READ: addr=0x{address:08X}, unmasked=0x{unmasked:08X}, idx=0x{idx:04X}, value=0x{value:02X}"
                    );
                }

                // Log reads from IRQ handler pointer area
                if unmasked >= 0x03007FFC {
                    tracing::debug!(
                        "!!! READ FROM IRQ HANDLER POINTER AREA !!!\n  \
                         Address: 0x{address:08X} (unmask to 0x{unmasked:08X}), Value: 0x{value:02X}"
                    );
                }

                value
            }
            0x0800_0000..=0x09FF_FFFF => self.read_rom(address - 0x0800_0000),
            0x0A00_0000..=0x0BFF_FFFF => self.read_rom(address - 0x0A00_0000),
            0x0C00_0000..=0x0DFF_FFFF => self.read_rom(address - 0x0C00_0000),
            0x0E00_0000..=0x0E01_FFFF => {
                let offset = address - 0x0E00_0000;

                // In ID mode, return manufacturer/device ID
                if self.flash_state == FlashState::IdMode {
                    let id_value = match offset {
                        // Sanyo LE26FV10N1TS (128KB / 1Mbit), same as mGBA uses
                        0x0000 => 0x62, // Manufacturer ID (Sanyo)
                        0x0001 => 0x13, // Device ID (1Mbit = 128KB)
                        _ => 0xFF,
                    };
                    tracing::debug!(
                        "Flash ID READ: addr=0x{address:08X}, offset=0x{offset:04X}, value=0x{id_value:02X}"
                    );
                    return id_value;
                }

                // Normal read: apply bank offset for 128KB flash
                let real_offset = (self.flash_bank as usize * 0x10000) + (offset & 0xFFFF);
                let value = if real_offset < self.sram.len() {
                    self.sram[real_offset]
                } else {
                    0xFF
                };
                tracing::debug!(
                    "Flash READ: addr=0x{:08X}, bank={}, offset=0x{:04X}, value=0x{:02X}",
                    address,
                    self.flash_bank,
                    offset,
                    value
                );
                value
            }
            0x0000_4000..=0x01FF_FFFF | 0x1000_0000..=0xFFFF_FFFF => {
                tracing::debug!("READ on unused memory 0x{address:08X}");
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => unimplemented!("Unimplemented memory region. {address:x}"),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x0000_0000..=0x0000_3FFF => {
                // BIOS is read-only, ignore writes
                // (Some games may try to write here, but it should have no effect)
            }
            0x0200_0000..=0x0203_FFFF => self.working_ram[address - 0x0200_0000] = value,
            // Mirror
            0x0204_0000..=0x02FF_FFFF => {
                self.working_ram[get_unmasked_address(address, 0x00FF_0000, 0xFF00_FFFF, 16, 4)
                    - 0x0200_0000] = value;
            }
            0x0300_0000..=0x0300_7FFF => {
                // Log writes to IRQ handler pointer area (last 4 bytes of IWRAM)
                if address >= 0x03007FFC {
                    tracing::debug!(
                        "!!! WRITE TO IRQ HANDLER POINTER AREA !!!\n  \
                         Address: 0x{address:08X}, Value: 0x{value:02X}",
                    );
                }
                // Log writes to IRQ handler code area (for debugging)
                if (0x03003580..0x03003600).contains(&address) {
                    tracing::debug!(
                        "!!! WRITE TO IRQ HANDLER CODE AREA !!!\n  \
                         Address: 0x{address:08X}, Value: 0x{value:02X}",
                    );
                }
                // Debug: Log writes around the problematic address
                if (0x0300_36A0..=0x0300_36B0).contains(&address) {
                    let idx = address - 0x0300_0000;
                    tracing::debug!(
                        "IWRAM WRITE: addr=0x{address:08X}, idx=0x{idx:04X}, value=0x{value:02X}"
                    );
                }
                self.working_iram[address - 0x0300_0000] = value;
            }
            // Mirror
            0x0300_8000..=0x03FF_FFFF => {
                let unmasked = get_unmasked_address(address, 0x00FF_F000, 0xFF00_0FFF, 12, 8);
                // Log writes to IRQ handler pointer area (mirrors to last 4 bytes of IWRAM)
                if unmasked >= 0x03007FFC {
                    tracing::debug!(
                        "!!! WRITE TO IRQ HANDLER POINTER AREA (mirrored) !!!\n  \
                         Address: 0x{address:08X} (unmask to 0x{unmasked:08X}), Value: 0x{value:02X}",
                    );
                }
                self.working_iram[unmasked - 0x0300_0000] = value;
            }
            0x0800_0000..=0x0DFF_FFFF => {
                // Check if this is a GPIO write (ROM offset 0xC4-0xC9)
                let rom_offset = address & 0x01FFFFFF; // Mask to get offset within ROM region
                if (0xC4..=0xC9).contains(&rom_offset) {
                    tracing::debug!("GPIO WRITE: offset 0x{rom_offset:04X} = 0x{value:02X}");
                    match rom_offset {
                        0xC4 => self.gpio_data.set_byte(0, value),
                        0xC5 => self.gpio_data.set_byte(1, value),
                        0xC6 => self.gpio_direction.set_byte(0, value),
                        0xC7 => self.gpio_direction.set_byte(1, value),
                        0xC8 => self.gpio_control.set_byte(0, value),
                        0xC9 => self.gpio_control.set_byte(1, value),
                        _ => unreachable!(),
                    }
                    tracing::debug!(
                        "  GPIO state: data=0x{:04X}, dir=0x{:04X}, ctrl=0x{:04X}",
                        self.gpio_data,
                        self.gpio_direction,
                        self.gpio_control
                    );
                } else {
                    // ROM is read-only, writes are ignored
                    tracing::debug!("Attempted write to ROM at {address:#010x}");
                }
            }
            0x0E00_0000..=0x0E01_FFFF => {
                // Flash command/write handling
                let offset = (address - 0x0E00_0000) & 0xFFFF; // 64KB offset within current bank

                tracing::debug!(
                    "Flash WRITE: addr=0x{:08X}, offset=0x{:04X}, value=0x{:02X}, state={:?}",
                    address,
                    offset,
                    value,
                    self.flash_state
                );

                // Handle Flash commands based on state machine
                match self.flash_state {
                    FlashState::Ready => {
                        // First command byte: 0xAA to 0x5555
                        if offset == 0x5555 && value == 0xAA {
                            self.flash_state = FlashState::Command1;
                        }
                    }
                    FlashState::Command1 => {
                        // Second command byte: 0x55 to 0x2AAA
                        if offset == 0x2AAA && value == 0x55 {
                            self.flash_state = FlashState::Command2;
                        } else {
                            self.flash_state = FlashState::Ready;
                        }
                    }
                    FlashState::Command2 => {
                        // Third command byte determines operation
                        if offset == 0x5555 {
                            match value {
                                0x90 => {
                                    // Enter ID mode
                                    tracing::debug!("Flash: Entering ID mode");
                                    self.flash_state = FlashState::IdMode;
                                }
                                0xF0 => {
                                    // Exit ID mode / Reset
                                    tracing::debug!("Flash: Reset/Exit ID mode");
                                    self.flash_state = FlashState::Ready;
                                }
                                0x80 => {
                                    // Erase command prefix
                                    tracing::debug!("Flash: Erase command prefix");
                                    self.flash_state = FlashState::EraseCommand;
                                }
                                0xA0 => {
                                    // Write byte command
                                    tracing::debug!("Flash: Write command");
                                    self.flash_state = FlashState::WriteCommand;
                                }
                                0xB0 => {
                                    // Bank switch command (for 128KB flash)
                                    tracing::debug!("Flash: Bank switch command");
                                    self.flash_state = FlashState::BankSelect;
                                }
                                _ => {
                                    tracing::debug!("Flash: Unknown command 0x{value:02X}");
                                    self.flash_state = FlashState::Ready;
                                }
                            }
                        } else {
                            self.flash_state = FlashState::Ready;
                        }
                    }
                    FlashState::IdMode => {
                        // Any write to 0x5555 with 0xF0 exits ID mode
                        if value == 0xF0 {
                            tracing::debug!("Flash: Exit ID mode");
                            self.flash_state = FlashState::Ready;
                        }
                        // Also handle standard command sequence in ID mode
                        else if offset == 0x5555 && value == 0xAA {
                            self.flash_state = FlashState::Command1;
                        }
                    }
                    FlashState::EraseCommand => {
                        // After 0x80, expect another 0xAA,0x55,command sequence
                        // The state machine needs to cycle through Command1->Command2->actual erase
                        if offset == 0x5555 && value == 0xAA {
                            self.flash_state = FlashState::EraseCommand1;
                        } else {
                            self.flash_state = FlashState::Ready;
                        }
                    }
                    FlashState::EraseCommand1 => {
                        if offset == 0x2AAA && value == 0x55 {
                            self.flash_state = FlashState::EraseCommand2;
                        } else {
                            self.flash_state = FlashState::Ready;
                        }
                    }
                    FlashState::EraseCommand2 => {
                        if value == 0x10 && offset == 0x5555 {
                            // Chip erase
                            tracing::debug!("Flash: Chip erase");
                            self.sram.fill(0xFF);
                            self.flash_state = FlashState::Ready;
                        } else if value == 0x30 {
                            // Sector erase (4KB sector)
                            let sector_base =
                                (self.flash_bank as usize * 0x10000) + (offset & 0xF000);
                            tracing::debug!("Flash: Sector erase at 0x{sector_base:05X}");
                            for i in 0..0x1000 {
                                if sector_base + i < self.sram.len() {
                                    self.sram[sector_base + i] = 0xFF;
                                }
                            }
                        }
                        self.flash_state = FlashState::Ready;
                    }
                    FlashState::BankSelect => {
                        // Bank number written to 0x0000
                        if offset == 0x0000 {
                            self.flash_bank = value & 0x01; // Only 0 or 1 for 128KB
                            tracing::debug!("Flash: Bank set to {}", self.flash_bank);
                        }
                        self.flash_state = FlashState::Ready;
                    }
                    FlashState::WriteCommand => {
                        // Write single byte to flash
                        let real_offset = (self.flash_bank as usize * 0x10000) + offset;
                        if real_offset < self.sram.len() {
                            // Flash write: can only clear bits (AND operation)
                            self.sram[real_offset] &= value;
                            tracing::debug!(
                                "Flash: Write 0x{value:02X} to offset 0x{real_offset:05X}"
                            );
                        }
                        self.flash_state = FlashState::Ready;
                    }
                }
            }
            0x0E02_0000..=0x0FFF_FFFF => {
                // Outside Flash range, ignore
                tracing::debug!("Attempted write to unused GamePak region at {address:#010x}");
            }
            _ => {
                tracing::debug!("WRITE to unused memory 0x{address:08X} = 0x{value:02X}");
                self.unused_region.insert(address, value);
            }
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
    fn test_bios_is_read_only() {
        let mut im = InternalMemory::default();
        // BIOS is read-only, writes should be ignored
        let original = im.read_at(0x000001EC);
        im.write_at(0x000001EC, 10);
        // Value should not have changed
        assert_eq!(im.read_at(0x000001EC), original);
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
