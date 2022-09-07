#[allow(dead_code)] // FIXME: remove this `allow` when all member are used.
pub(crate) struct CartridgeHeader {
    pub(crate) rom_entry_point: [u8; 4],
    pub(crate) nintendo_logo: [u8; 156],
    pub(crate) game_title: String,
    pub(crate) game_code: String,
    pub(crate) marker_code: String,
    pub(crate) fixed_value: [u8; 1],
    pub(crate) main_unit_code: [u8; 1],
    pub(crate) device_type: [u8; 1],
    pub(crate) reserved_area_1: [u8; 7],
    pub(crate) software_version: [u8; 1],
    pub(crate) complement_check: [u8; 1],
    pub(crate) reserved_area_2: [u8; 2],
    pub(crate) ram_entry_point: [u8; 4],
    pub(crate) boot_mode: [u8; 1],
    pub(crate) slave_id_number: [u8; 1],
    pub(crate) not_used: [u8; 26],
    pub(crate) joybus_mode_entry_point: [u8; 4],
}

impl CartridgeHeader {
    pub fn new(data: &[u8]) -> Self {
        let rom_entry_point = Self::extract_rom_entry_point(data);
        let nintendo_logo = Self::extract_nintendo_logo(data);
        let game_title = Self::extract_game_title(data);
        let game_code = Self::extract_game_code(data);
        let marker_code = Self::extract_marker_code(data);
        let fixed_value = Self::extract_fixed_value(data);
        let main_unit_code = Self::extract_main_unit_code(data);
        let device_type = Self::extract_device_type(data);
        let reserved_area_1 = Self::extract_reserved_area_1(data);
        let software_version = Self::extract_software_version(data);
        let complement_check = Self::extract_complement_check(data);
        let reserved_area_2 = Self::extract_reserved_area_2(data);
        let ram_entry_point = Self::extract_ram_entry_point(data);
        let boot_mode = Self::extract_boot_mode(data);
        let slave_id_number = Self::extract_slave_id_number(data);
        let not_used = Self::extract_not_used(data);
        let joybus_mode_entry_point = Self::extract_joybus_mode_entry_point(data);

        verify_checksum(data).expect("Invalid checksum");

        Self {
            rom_entry_point,
            nintendo_logo,
            game_title,
            game_code,
            marker_code,
            fixed_value,
            main_unit_code,
            device_type,
            reserved_area_1,
            software_version,
            complement_check,
            reserved_area_2,
            ram_entry_point,
            boot_mode,
            slave_id_number,
            not_used,
            joybus_mode_entry_point,
        }
    }

    // 32bit ARM branch opcode, eg. "B rom_start"
    fn extract_rom_entry_point(data: &[u8]) -> [u8; 4] {
        data[0x000..=0x003]
            .try_into()
            .expect("extracting rom entry point")
    }

    // compressed bitmap, required!
    fn extract_nintendo_logo(data: &[u8]) -> [u8; 156] {
        data[0x004..=0x09F]
            .try_into()
            .expect("extracting nintendo logo")
    }

    // uppercase ascii, max 12 characters
    fn extract_game_title(data: &[u8]) -> String {
        let game_title_bytes: [u8; 12] = data[0x0A0..=0x0AB]
            .try_into()
            .expect("extracting game title");

        String::from_utf8(game_title_bytes.into()).expect("parsing game title")
    }

    // uppercase ascii, 4 characters
    fn extract_game_code(data: &[u8]) -> String {
        let game_code_bytes: [u8; 4] = data[0x0AC..=0x0AF]
            .try_into()
            .expect("extracting game code");

        String::from_utf8(game_code_bytes.into()).expect("parsing game code")
    }

    // uppercase ascii, 2 characters
    fn extract_marker_code(data: &[u8]) -> String {
        let marker_code_bytes: [u8; 2] = data[0x0B0..=0x0B1]
            .try_into()
            .expect("extracting marker code");

        String::from_utf8(marker_code_bytes.into()).expect("parsing marker code")
    }

    // must be 96h, required!
    fn extract_fixed_value(data: &[u8]) -> [u8; 1] {
        data[0x0B2..=0x0B2]
            .try_into()
            .expect("extracting fixed value")
    }

    // 00h for current GBA models
    fn extract_main_unit_code(data: &[u8]) -> [u8; 1] {
        data[0x0B3..=0x0B3]
            .try_into()
            .expect("extracting main unit code")
    }

    // usually 00h (bit7=DACS/debug related)
    fn extract_device_type(data: &[u8]) -> [u8; 1] {
        data[0x0B4..=0x0B4]
            .try_into()
            .expect("extracting device type")
    }

    // should be zero filled
    fn extract_reserved_area_1(data: &[u8]) -> [u8; 7] {
        data[0x0B5..=0x0BB]
            .try_into()
            .expect("extracting reserved area 1")
    }

    // usually 00h
    fn extract_software_version(data: &[u8]) -> [u8; 1] {
        data[0x0BC..=0x0BC]
            .try_into()
            .expect("extracting software version")
    }

    // header checksum, required!
    fn extract_complement_check(data: &[u8]) -> [u8; 1] {
        // TODO: Do we check if extract_complement_check header (data[0x0BD..=0x0BD]) is correct?
        data[0x0BD..=0x0BD]
            .try_into()
            .expect("extracting complement check")
    }

    // should be zero filled
    fn extract_reserved_area_2(data: &[u8]) -> [u8; 2] {
        data[0x0BE..=0x0BF]
            .try_into()
            .expect("extracting reserved area 2")
    }

    // 32bit ARM branch opcode, eg. "B ram_start"
    fn extract_ram_entry_point(data: &[u8]) -> [u8; 4] {
        // This entry is used only if the GBA has been booted
        // by using Normal or Multiplay transfer mode (but not by Joybus mode).
        data[0x0C0..=0x0C3]
            .try_into()
            .expect("extracting ram entry point")
    }

    // init as 00h - BIOS overwrites this value!
    fn extract_boot_mode(data: &[u8]) -> [u8; 1] {
        // The slave GBA download procedure overwrites this byte by a value which is indicating
        // the used multiboot transfer mode.
        // Value  Expl.
        // 01h    Joybus mode
        // 02h    Normal mode
        // 03h    Multiplay mode
        // Be sure that your uploaded program does not contain important
        // program code or data at this location, or at the ID-byte location below.
        data[0x0C4..=0x0C4]
            .try_into()
            .expect("extracting boot mode")
    }

    // init as 00h - BIOS overwrites this value!
    fn extract_slave_id_number(data: &[u8]) -> [u8; 1] {
        // If the GBA has been booted in Normal or Multiplay mode,
        // this byte becomes overwritten by the slave ID number of the local GBA
        // (that'd be always 01h for normal mode).
        // Value  Expl.
        // 01h    Slave #1
        // 02h    Slave #2
        // 03h    Slave #3
        // When booted in Joybus mode, the value is NOT changed and
        // remains the same as uploaded from the master GBA.
        data[0x0C5..=0x0C5]
            .try_into()
            .expect("extracting slave id number")
    }

    // seems to be unused
    fn extract_not_used(data: &[u8]) -> [u8; 26] {
        data[0x0C6..=0x0DF].try_into().expect("extracting not used")
    }

    // 32bit ARM branch opcode, eg. "B joy_start"
    fn extract_joybus_mode_entry_point(data: &[u8]) -> [u8; 4] {
        // If the GBA has been booted by using Joybus transfer mode,
        // then the entry point is located at this address rather than at 20000C0h (ram_entry_point - data[0x0C0..=0x0C3]).
        // Either put your initialization procedure directly at this address, or redirect
        // to the actual boot procedure by depositing a "B <start>" opcode here (either one using 32bit ARM code).
        // Or, if you are not intending to support joybus mode (which is probably rarely used), ignore this entry.
        data[0x0E0..=0x0E3]
            .try_into()
            .expect("extracting joybus mode entry point")
    }
}

fn verify_checksum(data: &[u8]) -> Result<(), ()> {
    let checksum_expected = data[0xBD];
    let checksum = data[0xA0..=0xBC]
        .iter()
        .fold(0u8, |acc, &item| acc.wrapping_sub(item))
        .wrapping_sub(0x19);
    match checksum == checksum_expected {
        true => Ok(()),
        false => Err(()),
    }
}
