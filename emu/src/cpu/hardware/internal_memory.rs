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

/// Cartridge backup memory type, detected from a marker string in the ROM.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupType {
    /// No save hardware.
    #[default]
    None,
    /// 32 KB battery-backed SRAM, a flat 8-bit memory.
    Sram,
    /// 64 KB (512 Kbit) command-driven Flash.
    Flash64,
    /// 128 KB (1 Mbit) command-driven Flash with two banks.
    Flash128,
    /// Serial EEPROM, accessed via DMA in the upper Game Pak region.
    Eeprom,
}

impl BackupType {
    /// Detect the backup type from the ID string the ROM embeds (e.g.
    /// `SRAM_V`, `FLASH1M_V`). Longer/more specific markers are checked first.
    fn detect(rom: &[u8]) -> Self {
        let has = |pat: &[u8]| rom.windows(pat.len()).any(|w| w == pat);

        if has(b"EEPROM_V") {
            Self::Eeprom
        } else if has(b"FLASH1M_V") {
            Self::Flash128
        } else if has(b"FLASH512_V") || has(b"FLASH_V") {
            Self::Flash64
        } else if has(b"SRAM_V") || has(b"SRAM_F_V") {
            Self::Sram
        } else {
            Self::None
        }
    }

    /// Size in bytes of the backing buffer for this type.
    const fn buffer_size(self) -> usize {
        match self {
            Self::Flash64 => 0x1_0000,  // 64 KB
            Self::Flash128 => 0x2_0000, // 128 KB
            // SRAM is 32 KB; None/Eeprom keep a small unused buffer.
            Self::Sram | Self::None | Self::Eeprom => 0x8000,
        }
    }

    /// Flash manufacturer/device ID pair reported in ID mode.
    const fn flash_id(self) -> (u8, u8) {
        match self {
            // Panasonic MN63F805MNP (512 Kbit)
            Self::Flash64 => (0x32, 0x1B),
            // Sanyo LE26FV10N1TS (1 Mbit)
            _ => (0x62, 0x13),
        }
    }
}

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
    /// Skipped during serialization: read-only, already loaded from file at startup.
    #[serde(skip)]
    pub bios_system_rom: Vec<u8>,

    /// From 0x02000000 to 0x0203FFFF (256 `KBytes`).
    working_ram: Vec<u8>,

    /// From 0x03000000 to 0x03007FFF (32kb).
    working_iram: Vec<u8>,

    /// From 0x08000000 to 0x0FFFFFFF.
    /// Basically here you can find different kind of rom loaded.
    /// Skipped during serialization: read-only, already loaded from file at startup.
    // TODO: Not sure if we should split this into
    // 08000000-09FFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 0
    // 0A000000-0BFFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 1
    // 0C000000-0DFFFFFF Game Pak ROM/FlashROM (max 32MB) - Wait State 2
    // 0E000000-0E00FFFF Game Pak SRAM (max 64 KBytes) - 8bit Bus width
    // 0E010000-0FFFFFFF Not used
    #[serde(skip)]
    pub rom: Vec<u8>,

    /// Backup memory backing buffer (SRAM or Flash), sized to the detected
    /// backup type. Game Pak save data lives here.
    sram: Vec<u8>,

    /// Detected cartridge backup type, routing accesses to `0x0E00_0000`.
    ///
    /// Derived from the ROM, which is itself not serialized, so this is skipped
    /// and re-detected after a save state load. Keeping it out of the layout
    /// also avoids breaking existing saves when the field was added.
    #[serde(skip)]
    backup_type: BackupType,

    /// Flash memory state machine
    flash_state: FlashState,

    /// Flash bank selection for 128KB flash (0 or 1)
    flash_bank: u8,

    /// GPIO registers for RTC/rumble/etc (at ROM offset 0xC4-0xC9)
    /// Register layout: 0xC4=data, 0xC6=direction, 0xC8=control
    gpio_data: u16, // Pin state (4-bit)
    gpio_direction: u16, // Pin direction (4-bit, 1=output, 0=input)
    gpio_control: u16,   // GPIO enable/control (1-bit)

    /// True once the game has written a GPIO register, marking a cart that has
    /// the GPIO chip (RTC etc). Until then ROM offsets 0xC4-0xC9 read as plain
    /// ROM, since non-GPIO carts store code and data there.
    gpio_present: bool,

    /// From 0x00004000 to `0x01FF_FFFF`.
    /// From 0x10000000 to `0xFFFF_FFFF`.
    unused_region: HashMap<usize, u8>,

    /// EEPROM serial transfer state. Not serialized: it is only meaningful in
    /// the middle of a transfer, and the stored EEPROM contents live in `sram`.
    /// `eeprom_buffer` accumulates the bits the game clocks in and `eeprom_out`
    /// holds the bits to clock back out for a read.
    #[serde(skip)]
    eeprom_buffer: Vec<bool>,
    #[serde(skip)]
    eeprom_out: Vec<bool>,
    #[serde(skip)]
    eeprom_out_pos: usize,
}

impl InternalMemory {
    #[must_use]
    pub fn new(bios: [u8; 0x0000_4000], rom: &[u8]) -> Self {
        let backup_type = BackupType::detect(rom);
        Self {
            bios_system_rom: bios.to_vec(),
            working_ram: vec![0; 0x0004_0000],
            working_iram: vec![0; 0x0000_8000],
            rom: rom.to_vec(),
            // 0xFF is the erased state for both Flash and an unwritten SRAM cell.
            sram: vec![0xFF; backup_type.buffer_size()],
            backup_type,
            flash_state: FlashState::Ready,
            flash_bank: 0,
            gpio_data: 0,      // All pins low initially
            gpio_direction: 0, // All pins as inputs initially
            gpio_control: 1,   // GPIO enabled (allow reads)
            gpio_present: false,
            unused_region: HashMap::new(),
            eeprom_buffer: Vec::new(),
            eeprom_out: Vec::new(),
            eeprom_out_pos: 0,
        }
    }

    /// Re-detect the backup type from the ROM. Called after a save state load,
    /// where the ROM is restored separately and `backup_type` is not serialized.
    pub fn redetect_backup_type(&mut self) {
        self.backup_type = BackupType::detect(&self.rom);
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
            backup_type: BackupType::Flash128,
            flash_state: FlashState::Ready,
            flash_bank: 0,
            gpio_data: 0,
            gpio_direction: 0,
            gpio_control: 1,
            gpio_present: false,
            unused_region: HashMap::new(),
            eeprom_buffer: Vec::new(),
            eeprom_out: Vec::new(),
            eeprom_out_pos: 0,
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
        if self.gpio_present && (0xC4..=0xC9).contains(&address) {
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
    /// Fast-path word read for contiguous memory regions (ROM, WRAM, IWRAM).
    /// Returns `None` for I/O or special regions that need byte-by-byte access.
    #[inline]
    #[must_use]
    pub fn try_read_word(&self, address: usize) -> Option<u32> {
        match address {
            // ROM (wait state 0, 1, 2) - most common case (instruction fetches)
            0x0800_0000..=0x0DFF_FFFC => {
                // A GPIO cart maps 0xC4-0xC9 to registers, so route overlapping
                // reads to the slow byte path where they see live GPIO state.
                let raw = address & 0x01FF_FFFF;
                if self.gpio_present && raw <= 0xC9 && raw + 3 >= 0xC4 {
                    return None;
                }
                let offset = (address & 0x01FF_FFFF) % self.rom.len().max(1);
                if offset + 3 < self.rom.len() {
                    Some(u32::from_le_bytes([
                        self.rom[offset],
                        self.rom[offset + 1],
                        self.rom[offset + 2],
                        self.rom[offset + 3],
                    ]))
                } else {
                    None // Near end of ROM or GPIO region, use slow path
                }
            }
            // EWRAM (256KB, mirrored)
            0x0200_0000..=0x02FF_FFFC => {
                let offset = (address - 0x0200_0000) & 0x3_FFFF; // 256KB mask
                Some(u32::from_le_bytes([
                    self.working_ram[offset],
                    self.working_ram[offset + 1],
                    self.working_ram[offset + 2],
                    self.working_ram[offset + 3],
                ]))
            }
            // IWRAM (32KB, mirrored)
            0x0300_0000..=0x03FF_FFFC => {
                let offset = (address - 0x0300_0000) & 0x7FFF; // 32KB mask
                Some(u32::from_le_bytes([
                    self.working_iram[offset],
                    self.working_iram[offset + 1],
                    self.working_iram[offset + 2],
                    self.working_iram[offset + 3],
                ]))
            }
            // BIOS
            0x0000_0000..=0x0000_3FFC => Some(u32::from_le_bytes([
                self.bios_system_rom[address],
                self.bios_system_rom[address + 1],
                self.bios_system_rom[address + 2],
                self.bios_system_rom[address + 3],
            ])),
            _ => None,
        }
    }

    /// Fast-path halfword read for contiguous memory regions.
    /// Returns `None` for I/O or special regions that need byte-by-byte access.
    #[inline]
    #[must_use]
    pub fn try_read_half_word(&self, address: usize) -> Option<u16> {
        match address {
            // ROM (wait state 0, 1, 2)
            0x0800_0000..=0x0DFF_FFFE => {
                let raw = address & 0x01FF_FFFF;
                if self.gpio_present && raw <= 0xC9 && raw + 1 >= 0xC4 {
                    return None;
                }
                let offset = (address & 0x01FF_FFFF) % self.rom.len().max(1);
                if offset + 1 < self.rom.len() {
                    Some(u16::from_le_bytes([self.rom[offset], self.rom[offset + 1]]))
                } else {
                    None
                }
            }
            // EWRAM
            0x0200_0000..=0x02FF_FFFE => {
                let offset = (address - 0x0200_0000) & 0x3_FFFF;
                Some(u16::from_le_bytes([
                    self.working_ram[offset],
                    self.working_ram[offset + 1],
                ]))
            }
            // IWRAM
            0x0300_0000..=0x03FF_FFFE => {
                let offset = (address - 0x0300_0000) & 0x7FFF;
                Some(u16::from_le_bytes([
                    self.working_iram[offset],
                    self.working_iram[offset + 1],
                ]))
            }
            // BIOS
            0x0000_0000..=0x0000_3FFE => Some(u16::from_le_bytes([
                self.bios_system_rom[address],
                self.bios_system_rom[address + 1],
            ])),
            _ => None,
        }
    }

    /// Whether an access to `address` targets the serial EEPROM. EEPROM lives in
    /// the upper Game Pak region: the whole `0x0D` block for ROMs up to 16 MB, or
    /// only its last 256 bytes for larger ROMs (which use `0x0D` for ROM itself).
    #[must_use]
    pub fn is_eeprom_access(&self, address: usize) -> bool {
        if self.backup_type != BackupType::Eeprom {
            return false;
        }
        if self.rom.len() > 0x0100_0000 {
            (0x0DFF_FF00..=0x0DFF_FFFF).contains(&address)
        } else {
            (0x0D00_0000..=0x0DFF_FFFF).contains(&address)
        }
    }

    /// Clock one bit into the EEPROM from a DMA halfword write (only bit 0 is
    /// used). The bits accumulate until a read interprets the command.
    pub fn eeprom_write(&mut self, value: u16) {
        // A write that begins a new command after a read has finished clears the
        // stale output.
        if self.eeprom_out_pos >= self.eeprom_out.len() && !self.eeprom_out.is_empty() {
            self.eeprom_out.clear();
            self.eeprom_out_pos = 0;
        }
        self.eeprom_buffer.push(value & 1 == 1);
    }

    /// Clock one bit out of the EEPROM for a DMA halfword read. The first read
    /// after a command has been clocked in interprets that command.
    pub fn eeprom_read(&mut self) -> u16 {
        if !self.eeprom_buffer.is_empty() {
            self.eeprom_interpret_command();
        }

        if self.eeprom_out_pos < self.eeprom_out.len() {
            let bit = self.eeprom_out[self.eeprom_out_pos];
            self.eeprom_out_pos += 1;
            return u16::from(bit);
        }

        // Idle or programming finished: report ready.
        1
    }

    /// Interpret a fully clocked-in command. `11` is a read request, `10` a write
    /// request. The address width (6 or 14 bits) is inferred from the length of
    /// the bit stream.
    fn eeprom_interpret_command(&mut self) {
        let buffer = std::mem::take(&mut self.eeprom_buffer);
        if buffer.len() < 3 || !buffer[0] {
            return; // Not a valid command.
        }

        let is_read = buffer[1];
        let bits_to_addr = |bits: &[bool]| {
            bits.iter()
                .fold(0usize, |acc, &b| (acc << 1) | usize::from(b))
        };

        if is_read {
            // Read request: 2 command + address + stop. A 14-bit read is 17 bits.
            let width: usize = if buffer.len() >= 17 { 14 } else { 6 };
            let addr = bits_to_addr(&buffer[2..2 + width]);
            let base = Self::eeprom_block_offset(addr, width);

            // Output is 4 ignored zero bits followed by 64 data bits, MSB first.
            self.eeprom_out = Vec::with_capacity(68);
            self.eeprom_out.extend(std::iter::repeat_n(false, 4));
            for i in 0..64 {
                let byte = self.sram[base + i / 8];
                self.eeprom_out.push((byte >> (7 - (i % 8))) & 1 == 1);
            }
        } else {
            // Write request: 2 command + address + 64 data + stop. A 14-bit write
            // is 81 bits.
            let width: usize = if buffer.len() >= 81 { 14 } else { 6 };
            let addr = bits_to_addr(&buffer[2..2 + width]);
            let base = Self::eeprom_block_offset(addr, width);

            let data = &buffer[2 + width..2 + width + 64];
            for (i, byte) in data.chunks(8).enumerate() {
                self.sram[base + i] = byte.iter().fold(0u8, |acc, &b| (acc << 1) | u8::from(b));
            }
            // Programming done, so subsequent reads report ready.
            self.eeprom_out.clear();
        }
        self.eeprom_out_pos = 0;
    }

    /// Byte offset in `sram` for an EEPROM block, masked to the decoded size (64
    /// blocks for 6-bit, 1024 for 14-bit, and each block is 8 bytes).
    const fn eeprom_block_offset(addr: usize, width: usize) -> usize {
        let blocks = if width == 14 { 1024 } else { 64 };
        (addr % blocks) * 8
    }

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
            0x0E00_0000..=0x0FFF_FFFF => {
                let offset = address - 0x0E00_0000;

                match self.backup_type {
                    // No backup hardware: the bus floats high.
                    BackupType::None | BackupType::Eeprom => 0xFF,
                    // SRAM is a flat 8-bit memory, mirrored across the region.
                    BackupType::Sram => self.sram[offset & (self.sram.len() - 1)],
                    BackupType::Flash64 | BackupType::Flash128 => {
                        // In ID mode, return the manufacturer/device ID.
                        if self.flash_state == FlashState::IdMode {
                            let (manufacturer, device) = self.backup_type.flash_id();
                            return match offset {
                                0x0000 => manufacturer,
                                0x0001 => device,
                                _ => 0xFF,
                            };
                        }

                        // Normal read: apply the bank offset for 128KB flash.
                        let real_offset = (self.flash_bank as usize * 0x10000) + (offset & 0xFFFF);
                        self.sram.get(real_offset).copied().unwrap_or(0xFF)
                    }
                }
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
                    self.gpio_present = true;
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
            0x0E00_0000..=0x0FFF_FFFF => {
                let offset = (address - 0x0E00_0000) & 0xFFFF; // 64KB offset within current bank

                match self.backup_type {
                    // No backup hardware: writes are dropped.
                    BackupType::None | BackupType::Eeprom => {}
                    // SRAM is written directly, mirrored across the region.
                    BackupType::Sram => {
                        let masked = (address - 0x0E00_0000) & (self.sram.len() - 1);
                        self.sram[masked] = value;
                    }
                    BackupType::Flash64 | BackupType::Flash128 => {
                        self.flash_write(offset, value);
                    }
                }
            }
            _ => {
                tracing::debug!("WRITE to unused memory 0x{address:08X} = 0x{value:02X}");
                self.unused_region.insert(address, value);
            }
        }
    }

    /// Handle a write to Flash memory, driving the command state machine.
    #[allow(clippy::too_many_lines)]
    fn flash_write(&mut self, offset: usize, value: u8) {
        tracing::debug!(
            "Flash WRITE: offset=0x{:04X}, value=0x{:02X}, state={:?}",
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
                    let sector_base = (self.flash_bank as usize * 0x10000) + (offset & 0xF000);
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
                    tracing::debug!("Flash: Write 0x{value:02X} to offset 0x{real_offset:05X}");
                }
                self.flash_state = FlashState::Ready;
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

    #[allow(clippy::cast_possible_truncation)]
    fn eeprom_round_trip(width: usize, addr: usize) {
        let mut im = InternalMemory {
            backup_type: BackupType::Eeprom,
            sram: vec![0xFF; 0x8000],
            ..Default::default()
        };
        let data: [u8; 8] = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];

        let push_addr = |cmd: &mut Vec<u8>| {
            for i in (0..width).rev() {
                cmd.push(((addr >> i) & 1) as u8);
            }
        };

        // Write command: 10 + address + 64 data bits + stop.
        let mut cmd = vec![1u8, 0];
        push_addr(&mut cmd);
        for &byte in &data {
            for i in (0..8).rev() {
                cmd.push((byte >> i) & 1);
            }
        }
        cmd.push(0);
        for b in cmd {
            im.eeprom_write(u16::from(b));
        }
        // A read after the stop bit commits the programming.
        assert_eq!(im.eeprom_read(), 1);

        // Read command: 11 + address + stop.
        let mut cmd = vec![1u8, 1];
        push_addr(&mut cmd);
        cmd.push(0);
        for b in cmd {
            im.eeprom_write(u16::from(b));
        }

        // 4 dummy bits + 64 data bits, MSB first.
        let out: Vec<u16> = (0..68).map(|_| im.eeprom_read()).collect();
        let mut got = [0u8; 8];
        for (byte_i, slot) in got.iter_mut().enumerate() {
            for bit_i in 0..8 {
                *slot = (*slot << 1) | out[4 + byte_i * 8 + bit_i] as u8;
            }
        }
        assert_eq!(got, data, "EEPROM read must return what was written");
    }

    #[test]
    fn gpio_intercepts_halfword_reads_only_after_a_gpio_write() {
        let mut im = InternalMemory::default();
        im.rom[0xC4] = 0xAA;
        im.rom[0xC5] = 0xBB;

        // No GPIO write yet: 0xC4 reads plain ROM, fast halfword path included.
        assert_eq!(im.try_read_half_word(0x0800_00C4), Some(0xBBAA));
        assert_eq!(im.read_at(0x0800_00C4), 0xAA);

        // The game enables GPIO and writes the data register.
        im.write_at(0x0800_00C6, 0x0F); // direction = output
        im.write_at(0x0800_00C4, 0x05); // data

        // Now the fast path defers and the read returns the live GPIO register.
        assert_eq!(im.try_read_half_word(0x0800_00C4), None);
        assert_eq!(im.read_at(0x0800_00C4), 0x05);
    }

    #[test]
    fn eeprom_6bit_write_then_read_round_trips() {
        eeprom_round_trip(6, 3);
    }

    #[test]
    fn eeprom_14bit_write_then_read_round_trips() {
        eeprom_round_trip(14, 500);
    }
}
