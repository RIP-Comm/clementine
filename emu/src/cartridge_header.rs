use std::{error::Error, fs::File, io::Read};

#[allow(dead_code)] // FIXME: remove this `allow` when all member are used.

/// Contains the information of the cartridge header.
/// The OpCodes are all encoded in 32bit by default
pub struct CartridgeHeader {
    rom_entry_point: u32,
    nintendo_logo: [u8; 156],
    game_title: String,
    game_code: String,
    marker_code: String,
    main_unit_code: u8,
    device_type: u8,
    reserved_area_1: [u8; 7],
    software_version: u8,
    complement_check: u8,
    reserved_area_2: [u8; 2],
    ram_entry_point: u32,
    boot_mode: u8,
    slave_id_number: u8,
    joybus_mode_entry_point: u32,
}

#[allow(dead_code)]
impl CartridgeHeader {
    pub fn new(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        execute_checks(data)?;

        let rom_entry_point = u32::from_be_bytes(data[0x00..0x04].try_into()?);
        let nintendo_logo = data[0x04..0xA0].try_into()?;
        let game_title = into_ascii_str(&data[0xA0..0xAC])?;
        let game_code = into_ascii_str(&data[0xAC..0xB0])?;
        let marker_code = into_ascii_str(&data[0xB0..0xB2])?;
        let main_unit_code = data[0xB3];
        let device_type = data[0xB4];
        let reserved_area_1 = data[0xB5..0xBC].try_into()?;
        let software_version = data[0xBC];
        let complement_check = data[0xBD];
        let reserved_area_2 = data[0xBE..0xC0].try_into()?;

        // ---- Multiboot header ----
        let ram_entry_point = u32::from_be_bytes(data[0xC0..0xC4].try_into()?);
        let boot_mode = data[0xC4];
        let slave_id_number = 0xC5;
        let joybus_mode_entry_point = u32::from_be_bytes(data[0x0E0..0x0E4].try_into()?);

        Ok(Self {
            rom_entry_point,
            nintendo_logo,
            game_title,
            game_code,
            marker_code,
            main_unit_code,
            device_type,
            reserved_area_1,
            software_version,
            complement_check,
            reserved_area_2,
            ram_entry_point,
            boot_mode,
            slave_id_number,
            joybus_mode_entry_point,
        })
    }

    pub fn from_file(path: &str) -> Result<(Self, Vec<u8>), Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        let cartridge = Self::new(&data)?;

        Ok((cartridge, data))
    }

    /// 32bit ARM branch opcode
    pub const fn rom_entry_point(&self) -> u32 {
        self.rom_entry_point
    }

    /// Compressed bitmap
    pub const fn nintendo_logo(&self) -> &[u8; 156] {
        &self.nintendo_logo
    }

    pub fn game_title(&self) -> &str {
        self.game_title.as_str()
    }

    pub fn game_code(&self) -> &str {
        self.game_code.as_str()
    }

    pub fn marker_code(&self) -> &str {
        self.marker_code.as_str()
    }

    /// 00h for current GBA models
    pub const fn main_unit_code(&self) -> u8 {
        self.main_unit_code
    }

    /// Usually 0x00
    pub const fn device_type(&self) -> u8 {
        self.device_type
    }

    /// Should be zero filled
    pub const fn reserved_area_1(&self) -> &[u8; 7] {
        &self.reserved_area_1
    }

    /// Usually 0x00
    pub const fn software_version(&self) -> u8 {
        self.software_version
    }

    pub const fn complement_check(&self) -> u8 {
        self.complement_check
    }

    /// Should be zero filled
    pub const fn reserved_area_2(&self) -> [u8; 2] {
        self.reserved_area_2
    }

    /// 32bit ARM branch opcode
    pub const fn ram_entry_point(&self) -> u32 {
        // This entry is used only if the GBA has been booted
        // by using Normal or Multiplay transfer mode (but not by Joybus mode).
        self.ram_entry_point
    }

    /// Init as 00h, BIOS overwrites this value
    pub const fn boot_mode(&self) -> u8 {
        // The slave GBA download procedure overwrites this byte by a value which is indicating
        // the used multiboot transfer mode.
        // Value  Expl.
        // 01h    Joybus mode
        // 02h    Normal mode
        // 03h    Multiplay mode
        // Be sure that your uploaded program does not contain important
        // program code or data at this location, or at the ID-byte location below.
        self.boot_mode
    }

    /// Init as 00h, BIOS overwrites this value
    pub const fn slave_id_number(&self) -> u8 {
        // If the GBA has been booted in Normal or Multiplay mode,
        // this byte becomes overwritten by the slave ID number of the local GBA
        // (that'd be always 01h for normal mode).
        // Value  Expl.
        // 01h    Slave #1
        // 02h    Slave #2
        // 03h    Slave #3
        // When booted in Joybus mode, the value is NOT changed and
        // remains the same as uploaded from the master GBA.
        self.slave_id_number
    }

    /// 32bit ARM branch opcode
    pub const fn joybus_mode_entry_point(&self) -> u32 {
        // If the GBA has been booted by using Joybus transfer mode,
        // then the entry point is located at this address rather than at 20000C0h (ram_entry_point - data[0x0C0..=0x0C3]).
        // Either put your initialization procedure directly at this address, or redirect
        // to the actual boot procedure by depositing a "B <start>" opcode here (either one using 32bit ARM code).
        // Or, if you are not intending to support joybus mode (which is probably rarely used), ignore this entry.
        self.joybus_mode_entry_point
    }
}

fn execute_checks(data: &[u8]) -> Result<(), Box<dyn Error>> {
    if data[0xB2] != 0x96 {
        return Err("Wrong fixed value".into());
    }

    // Header checksum verify
    let checksum_expected = data[0xBD];
    let checksum = data[0xA0..0xBD]
        .iter()
        .fold(0u8, |acc, &item| acc.wrapping_sub(item))
        .wrapping_sub(0x19);

    if checksum != checksum_expected {
        return Err(format!("Expected {} but got {}", checksum_expected, checksum).into());
    }

    Ok(())
}

fn into_ascii_str(data: &[u8]) -> Result<String, Box<dyn Error>> {
    let string = String::from_utf8(data.into())?;

    for chr in string.chars() {
        if !chr.is_ascii() {
            return Err("Not a valid ASCII sequence".into());
        }
    }

    Ok(string)
}
