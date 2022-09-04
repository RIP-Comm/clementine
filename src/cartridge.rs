use std::{
    fs::File,
    io::{self, Read},
    rc::Rc,
};

pub(crate) struct CartridgeHeader {
    entry_point: [u8; 4],
    nintendo_logo: [u8; 156],
    game_title: String,
    game_code: String,
    //TODO: MISSING FIELDS...
}

impl CartridgeHeader {
    pub fn new(data: &[u8]) -> Self {
        Self::verify_checksum(data).expect("Invalid checksum");

        let entry_point: [u8; 4] = data[0x00..0x04].try_into().unwrap();
        let nintendo_logo: [u8; 156] = data[0x04..0xA0].try_into().unwrap();
        let game_title = Self::fetch_ascii(data, 0xA0, 0xAC);
        let game_code = Self::fetch_ascii(data, 0xAC, 0xB0);

        // fetch other data here

        Self {
            nintendo_logo,
            entry_point,
            game_code,
            game_title,
        }
    }

    pub(crate) fn entry_point(&self) -> &[u8; 4] {
        &self.entry_point
    }

    pub(crate) fn nintendo_logo(&self) -> &[u8; 156] {
        &self.nintendo_logo
    }

    pub(crate) fn game_title(&self) -> &str {
        self.game_title.as_str()
    }

    pub(crate) fn game_code(&self) -> &str {
        self.game_code.as_str()
    }

    /// Retrieves an ascii sequence of chars, if not ascii the program will panic.
    ///
    /// Data fetched as follows: `[from..to]`, `to` excluded
    fn fetch_ascii(data: &[u8], from: usize, to: usize) -> String {
        let str = String::from_utf8(data[from..to].to_vec()).unwrap();
        if !str.is_ascii() {
            panic!("Not an ascii sequence");
        }
        str
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
}

pub(crate) struct Cartridge {
    header: CartridgeHeader,
    rom: Rc<Vec<u8>>,
}

impl Cartridge {
    pub(crate) fn from_file(path: &str) -> io::Result<Self> {
        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;
        data.shrink_to_fit();

        Ok(Cartridge::new(data.as_slice()))
    }

    /// Creates a new Cardridge from raw data
    pub(crate) fn new(data: &[u8]) -> Cartridge {
        let header_data = &data[0x000..0x0C0]; //First 192 bytes, which represents the header
        let header = CartridgeHeader::new(header_data);

        //TODO: determine the real starting point of the ROM(from the rom entry point)
        let rom = Rc::new(data.into());

        Self { header, rom }
    }

    pub(crate) fn header(&self) -> &CartridgeHeader {
        &self.header
    }

    pub(crate) fn rom(&self) -> Rc<Vec<u8>> {
        self.rom.clone()
    }
}
