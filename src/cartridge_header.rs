pub(crate) struct CartridgeHeader {
    pub(crate) entry_point: [u8; 4],
    pub(crate) nintendo_logo: [u8; 156],
    pub(crate) game_title: String,
    pub(crate) game_code: String,
}

impl CartridgeHeader {
    pub fn new(data: &[u8]) -> Self {
        let entry_point = Self::extract_entry_point(data);
        let nintendo_logo = Self::extract_nintendo_logo(data);
        let game_title = Self::extract_game_title(data);
        let game_code = Self::extract_game_code(data);

        verify_checksum(data).expect("Invalid checksum");

        Self {
            entry_point,
            nintendo_logo,
            game_title,
            game_code,
        }
    }

    fn extract_entry_point(data: &[u8]) -> [u8; 4] {
        data[0x000..=0x003]
            .try_into()
            .expect("extracting entry point")
    }

    fn extract_nintendo_logo(data: &[u8]) -> [u8; 156] {
        data[0x004..=0x09F]
            .try_into()
            .expect("extracting nintendo logo")
    }

    fn extract_game_title(data: &[u8]) -> String {
        let game_title_bytes: [u8; 12] = data[0x0A0..=0x0AB]
            .try_into()
            .expect("extracting game title");
        String::from_utf8(game_title_bytes.into()).expect("parsing game title")
    }

    fn extract_game_code(data: &[u8]) -> String {
        let game_code_bytes: [u8; 4] = data[0x0AC..=0x0AF]
            .try_into()
            .expect("extracting game code");
        String::from_utf8(game_code_bytes.into()).expect("parsing game code")
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
