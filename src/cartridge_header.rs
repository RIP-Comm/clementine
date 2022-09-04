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
        }
    }

    // Address: 000h
    // Bytes:   4
    // Info:    32bit ARM branch opcode
    fn extract_rom_entry_point(data: &[u8]) -> [u8; 4] {
        data[0x000..=0x003]
            .try_into()
            .expect("extracting ROM entry point")
    }

    // Address: 004h
    // Bytes:   156
    // Info:    compressed bitmap, required!
    fn extract_nintendo_logo(data: &[u8]) -> [u8; 156] {
        data[0x004..=0x09F]
            .try_into()
            .expect("extracting nintendo logo")
    }

    // Address: 0A0h
    // Bytes:   12
    // Info:    uppercase ascii, max 12 characters
    fn extract_game_title(data: &[u8]) -> String {
        let game_title_bytes: [u8; 12] = data[0x0A0..=0x0AB]
            .try_into()
            .expect("extracting game title");
        
        String::from_utf8(game_title_bytes.into())
            .expect("parsing game title")
    }

    // Address: 0ACh
    // Bytes:   4
    // Info:    uppercase ascii, 4 characters
    fn extract_game_code(data: &[u8]) -> String {
        let game_code_bytes: [u8; 4] = data[0x0AC..=0x0AF]
            .try_into()
            .expect("extracting game code");
        
        String::from_utf8(game_code_bytes.into())
            .expect("parsing game code")
    }

    // Address: 0B0h
    // Bytes:   2
    // Info:    uppercase ascii, 2 characters
    fn extract_marker_code(data: &[u8]) -> String {
        let marker_code_bytes: [u8; 2] = data[0x0B0..=0x0B1]
            .try_into()
            .expect("extracting marker code");
        
        String::from_utf8(marker_code_bytes.into())
            .expect("parsing marker code")
    }

    // Address: 0B2h
    // Bytes:   1
    // Info:    must be 96h, required!
    fn extract_fixed_value(data: &[u8]) -> [u8; 1] {
        // TODO: Do we check if data[0x0B2..=0x0B2] is equals to 96h?
        data[0x0B2..=0x0B2]
            .try_into()
            .expect("extracting fixed value")
    }

    // Address: 0B3h
    // Bytes:   1
    // Info:    00h for current GBA models
    fn extract_main_unit_code(data: &[u8]) -> [u8; 1] {
        // TODO: Do we check if data[0x0B2..=0x0B2] is equals to 00h?
        data[0x0B3..=0x0B3]
            .try_into()
            .expect("extracting main unit code")
    }

    // Address: 0B4h
    // Bytes:   1
    // Info:    usually 00h (bit7=DACS/debug related)
    fn extract_device_type(data: &[u8]) -> [u8; 1] {
        data[0x0B4..=0x0B4]
            .try_into()
            .expect("extracting device type")
    }

    // Address: 0B5h
    // Bytes:   7
    // Info:    should be zero filled
    fn extract_reserved_area_1(data: &[u8]) -> [u8; 7] {
        data[0x0B5..=0x0BB]
            .try_into()
            .expect("extracting reserved area 1")
    }

    // Address: 0BCh
    // Bytes:   1
    // Info:    usually 00h
    fn extract_software_version(data: &[u8]) -> [u8; 1] {
        // TODO: Do we check if data[0x0B2..=0x0B2] is equals to 00h?
        data[0x0BC..=0x0Bc]
            .try_into()
            .expect("extracting software version")
    }

    // Address: 0BDh
    // Bytes:   1
    // Info:    header checksum, required!
    fn extract_complement_check(data: &[u8]) -> [u8; 1] {
        data[0x0BD..=0x0BE]
            .try_into()
            .expect("extracting complement check")
    }

    // Address: 0BEh
    // Bytes:   2
    // Info:    should be zero filled
    fn extract_reserved_area_2(data: &[u8]) -> [u8; 2] {
        data[0x0BE..=0x0BF]
            .try_into()
            .expect("extracting reserved area 2")
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
