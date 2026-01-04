//! # GBA Cartridge Header
//!
//! Every GBA ROM starts with a 192-byte header containing metadata about
//! the game and boot information. This module parses and validates that header.
//!
//! ## Header Layout (0x000 - 0x0BF)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                        GBA Cartridge Header                                 │
//! ├──────────┬────────┬──────────────────────────────────────────────────────────┤
//! │  Offset  │  Size  │  Description                                            │
//! ├──────────┼────────┼──────────────────────────────────────────────────────────┤
//! │  0x000   │   4    │  ROM Entry Point (ARM branch instruction)               │
//! │  0x004   │  156   │  Nintendo Logo (compressed bitmap, verified by BIOS)    │
//! │  0x0A0   │   12   │  Game Title (uppercase ASCII)                           │
//! │  0x0AC   │   4    │  Game Code (e.g., "AXVE" for Pokémon Ruby)              │
//! │  0x0B0   │   2    │  Maker Code (e.g., "01" for Nintendo)                   │
//! │  0x0B2   │   1    │  Fixed Value (must be 0x96)                             │
//! │  0x0B3   │   1    │  Main Unit Code (0x00 for GBA)                          │
//! │  0x0B4   │   1    │  Device Type                                            │
//! │  0x0B5   │   7    │  Reserved (zero-filled)                                 │
//! │  0x0BC   │   1    │  Software Version                                       │
//! │  0x0BD   │   1    │  Complement Check (header checksum)                     │
//! │  0x0BE   │   2    │  Reserved (zero-filled)                                 │
//! └──────────┴────────┴──────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Multiboot Header (0x0C0 - 0x0E3)
//!
//! Additional fields used for multiboot (download play) games:
//!
//! | Offset | Size | Description                    |
//! |--------|------|--------------------------------|
//! | 0x0C0  |  4   | RAM Entry Point                |
//! | 0x0C4  |  1   | Boot Mode (set by BIOS)        |
//! | 0x0C5  |  1   | Slave ID Number                |
//! | 0x0C6  | 26   | Not Used                       |
//! | 0x0E0  |  4   | Joybus Entry Point             |
//!
//! ## Boot-Critical Fields
//!
//! The BIOS enforces three critical fields during boot. If any fail, the GBA halts:
//!
//! 1. **Nintendo Logo (0x004)**: The BIOS compares this 156-byte bitmap against its
//!    internal copy. This was a legal/DRM measure - pirates including the logo could
//!    be sued for trademark infringement.
//!
//! 2. **Header Checksum (0x0BD)**: Calculated as:
//!    ```text
//!    checksum = -(sum of bytes 0xA0..0xBC) - 0x19
//!    ```
//!    If this doesn't match byte 0xBD, the GBA halts.
//!
//! 3. **Entry Point (0x000)**: Usually an ARM branch instruction (opcode 0xEA).
//!    After BIOS runs, PC is set to 0x08000000 and this instruction executes,
//!    typically jumping over the header to game code.

/// The official Nintendo logo that must be present in the cartridge header.
/// The BIOS compares this against its internal copy during boot.
/// This is 156 bytes of compressed bitmap data.
#[rustfmt::skip]
pub const NINTENDO_LOGO: [u8; 156] = [
    0x24, 0xFF, 0xAE, 0x51, 0x69, 0x9A, 0xA2, 0x21, 0x3D, 0x84, 0x82, 0x0A,
    0x84, 0xE4, 0x09, 0xAD, 0x11, 0x24, 0x8B, 0x98, 0xC0, 0x81, 0x7F, 0x21,
    0xA3, 0x52, 0xBE, 0x19, 0x93, 0x09, 0xCE, 0x20, 0x10, 0x46, 0x4A, 0x4A,
    0xF8, 0x27, 0x31, 0xEC, 0x58, 0xC7, 0xE8, 0x33, 0x82, 0xE3, 0xCE, 0xBF,
    0x85, 0xF4, 0xDF, 0x94, 0xCE, 0x4B, 0x09, 0xC1, 0x94, 0x56, 0x8A, 0xC0,
    0x13, 0x72, 0xA7, 0xFC, 0x9F, 0x84, 0x4D, 0x73, 0xA3, 0xCA, 0x9A, 0x61,
    0x58, 0x97, 0xA3, 0x27, 0xFC, 0x03, 0x98, 0x76, 0x23, 0x1D, 0xC7, 0x61,
    0x03, 0x04, 0xAE, 0x56, 0xBF, 0x38, 0x84, 0x00, 0x40, 0xA7, 0x0E, 0xFD,
    0xFF, 0x52, 0xFE, 0x03, 0x6F, 0x95, 0x30, 0xF1, 0x97, 0xFB, 0xC0, 0x85,
    0x60, 0xD6, 0x80, 0x25, 0xA9, 0x63, 0xBE, 0x03, 0x01, 0x4E, 0x38, 0xE2,
    0xF9, 0xA2, 0x34, 0xFF, 0xBB, 0x3E, 0x03, 0x44, 0x78, 0x00, 0x90, 0xCB,
    0x88, 0x11, 0x3A, 0x94, 0x65, 0xC0, 0x7C, 0x63, 0x87, 0xF0, 0x3C, 0xAF,
    0xD6, 0x25, 0xE4, 0x8B, 0x38, 0x0A, 0xAC, 0x72, 0x21, 0xD4, 0xF8, 0x07,
];

/// Parsed GBA cartridge header.
///
/// Contains all metadata from the ROM header, including game title,
/// codes, and the validated checksum.
pub struct CartridgeHeader {
    /// ARM branch instruction at ROM start (usually jumps over header).
    pub rom_entry_point: [u8; 4],
    /// Nintendo logo bitmap - BIOS verifies this matches its internal copy.
    pub nintendo_logo: [u8; 156],
    /// Game title (uppercase ASCII, max 12 chars).
    pub game_title: String,
    /// Game code (4 chars, e.g., "BPEE" for Pokemon Emerald).
    pub game_code: String,
    /// Maker/publisher code (2 chars, e.g., "01" for Nintendo).
    pub marker_code: String,
    /// Must be 0x96 for valid cartridge.
    pub fixed_value: u8,
    /// Main unit code (0x00 for GBA).
    pub main_unit_code: u8,
    /// Device type (usually 0x00, bit 7 = DACS/debug).
    pub device_type: u8,
    /// Reserved area (should be zero).
    pub reserved_area_1: [u8; 7],
    /// Software version (usually 0x00).
    pub software_version: u8,
    /// Header checksum stored in ROM.
    pub complement_check: u8,
    /// Calculated header checksum (should match `complement_check`).
    pub calculated_checksum: u8,
    /// Reserved area (should be zero).
    pub reserved_area_2: [u8; 2],
    /// RAM entry point for multiboot.
    pub ram_entry_point: [u8; 4],
    /// Boot mode (set by BIOS during multiboot).
    pub boot_mode: u8,
    /// Slave ID for multiboot.
    pub slave_id_number: u8,
    /// Unused area.
    pub not_used: [u8; 26],
    /// Joybus mode entry point.
    pub joybus_mode_entry_point: [u8; 4],

    // Validation results
    /// Whether the Nintendo logo matches the expected value.
    pub logo_valid: bool,
    /// Whether the header checksum is valid.
    pub checksum_valid: bool,
    /// Whether the fixed value (0x96) is correct.
    pub fixed_value_valid: bool,
}

impl CartridgeHeader {
    /// Create a new `CartridgeHeader` from a slice of bytes.
    ///
    /// # Panics
    /// Panics if the data slice is too short to contain a valid header.
    #[must_use]
    pub fn new(data: &[u8]) -> Self {
        let nintendo_logo = Self::extract_nintendo_logo(data);
        let fixed_value = data[0x0B2];
        let complement_check = data[0x0BD];
        let calculated_checksum = Self::calculate_checksum(data);

        let logo_valid = nintendo_logo == NINTENDO_LOGO;
        let checksum_valid = complement_check == calculated_checksum;
        let fixed_value_valid = fixed_value == 0x96;

        if !logo_valid {
            tracing::warn!("Nintendo logo does not match expected value");
        }
        if !checksum_valid {
            tracing::warn!(
                "Header checksum mismatch: expected {complement_check:#04X}, calculated {calculated_checksum:#04X}"
            );
        }
        if !fixed_value_valid {
            tracing::warn!("Fixed value at 0xB2 is {fixed_value:#04X}, expected 0x96");
        }

        Self {
            rom_entry_point: Self::extract_rom_entry_point(data),
            nintendo_logo,
            game_title: Self::extract_game_title(data),
            game_code: Self::extract_game_code(data),
            marker_code: Self::extract_marker_code(data),
            fixed_value,
            main_unit_code: data[0x0B3],
            device_type: data[0x0B4],
            reserved_area_1: Self::extract_reserved_area_1(data),
            software_version: data[0x0BC],
            complement_check,
            calculated_checksum,
            reserved_area_2: Self::extract_reserved_area_2(data),
            ram_entry_point: Self::extract_ram_entry_point(data),
            boot_mode: data[0x0C4],
            slave_id_number: data[0x0C5],
            not_used: Self::extract_not_used(data),
            joybus_mode_entry_point: Self::extract_joybus_mode_entry_point(data),
            logo_valid,
            checksum_valid,
            fixed_value_valid,
        }
    }

    /// Calculate the header checksum.
    /// Formula: checksum = -(sum of bytes 0xA0..0xBC) - 0x19
    fn calculate_checksum(data: &[u8]) -> u8 {
        data[0xA0..0xBD]
            .iter()
            .fold(0u8, |acc, &byte| acc.wrapping_sub(byte))
            .wrapping_sub(0x19)
    }

    /// Check if all boot-critical fields are valid.
    /// The BIOS will halt if any of these fail.
    #[must_use]
    pub const fn is_bootable(&self) -> bool {
        self.logo_valid && self.checksum_valid && self.fixed_value_valid
    }

    /// Get the entry point address decoded from the ARM branch instruction.
    /// Returns the target address the branch jumps to.
    #[must_use]
    pub fn entry_point_address(&self) -> u32 {
        // ARM branch instruction format: 0xEA + 24-bit signed offset
        // The offset is in words (4 bytes), and PC is 8 bytes ahead during execution
        let opcode = u32::from_le_bytes(self.rom_entry_point);
        if (opcode >> 24) == 0xEA {
            // B instruction
            let offset = opcode & 0x00FF_FFFF;
            // sign-extend 24-bit to 32-bit and calculate entry address
            #[allow(clippy::cast_possible_wrap)] // intentional reinterpretation as signed
            let signed_offset = if offset & 0x0080_0000 != 0 {
                (offset | 0xFF00_0000) as i32
            } else {
                offset as i32
            };
            // PC + 8 + (offset * 4), where PC = 0x08000000
            // result is always a valid GBA ROM address (32-bit)
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let addr = (0x0800_0008_i64 + i64::from(signed_offset) * 4) as u32;
            addr
        } else {
            0x0800_0000 // not a branch, return ROM start
        }
    }

    /// Check if the entry point looks like a valid ARM branch instruction.
    #[must_use]
    pub const fn has_valid_entry_point(&self) -> bool {
        // check if it's a B (branch) instruction: 0xEA______
        (self.rom_entry_point[3] == 0xEA) || (self.rom_entry_point[3] == 0xEB)
    }

    /// Extract ROM entry point (4 bytes at 0x000).
    fn extract_rom_entry_point(data: &[u8]) -> [u8; 4] {
        data[0x000..0x004]
            .try_into()
            .expect("extracting rom entry point")
    }

    /// Extract Nintendo logo (156 bytes at 0x004).
    fn extract_nintendo_logo(data: &[u8]) -> [u8; 156] {
        data[0x004..0x0A0]
            .try_into()
            .expect("extracting nintendo logo")
    }

    /// Extract game title (12 bytes at 0x0A0, uppercase ASCII).
    fn extract_game_title(data: &[u8]) -> String {
        String::from_utf8_lossy(&data[0x0A0..0x0AC])
            .trim_end_matches('\0')
            .to_string()
    }

    /// Extract game code (4 bytes at 0x0AC).
    fn extract_game_code(data: &[u8]) -> String {
        String::from_utf8_lossy(&data[0x0AC..0x0B0])
            .trim_end_matches('\0')
            .to_string()
    }

    /// Extract maker code (2 bytes at 0x0B0).
    fn extract_marker_code(data: &[u8]) -> String {
        String::from_utf8_lossy(&data[0x0B0..0x0B2])
            .trim_end_matches('\0')
            .to_string()
    }

    /// Extract reserved area 1 (7 bytes at 0x0B5).
    fn extract_reserved_area_1(data: &[u8]) -> [u8; 7] {
        data[0x0B5..0x0BC]
            .try_into()
            .expect("extracting reserved area 1")
    }

    /// Extract reserved area 2 (2 bytes at 0x0BE).
    fn extract_reserved_area_2(data: &[u8]) -> [u8; 2] {
        data[0x0BE..0x0C0]
            .try_into()
            .expect("extracting reserved area 2")
    }

    /// Extract RAM entry point for multiboot (4 bytes at 0x0C0).
    fn extract_ram_entry_point(data: &[u8]) -> [u8; 4] {
        data[0x0C0..0x0C4]
            .try_into()
            .expect("extracting ram entry point")
    }

    /// Extract unused area (26 bytes at 0x0C6).
    fn extract_not_used(data: &[u8]) -> [u8; 26] {
        data[0x0C6..0x0E0].try_into().expect("extracting not used")
    }

    /// Extract Joybus mode entry point (4 bytes at 0x0E0).
    fn extract_joybus_mode_entry_point(data: &[u8]) -> [u8; 4] {
        data[0x0E0..0x0E4]
            .try_into()
            .expect("extracting joybus entry point")
    }
}
