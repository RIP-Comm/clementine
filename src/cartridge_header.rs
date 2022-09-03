pub(crate) struct CartridgeHeader {
    pub(crate) title: String,
}

impl CartridgeHeader {
    pub fn new(data: &[u8]) -> Self {
        let t = data[0x00A0..0x00AC].to_vec();
        let t = String::from_utf8(t).expect("reading title");

        verify_checksum(data).expect("Invalid checksum");

        Self { title: t }
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
