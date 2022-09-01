pub(crate) struct CartridgeHeader {
    pub(crate) title: String,
}

impl CartridgeHeader {
    pub fn new(data: Vec<u8>) -> Self {
        let t = data[0x00A0..0x00AC].to_vec();
        let t = String::from_utf8(t).expect("reading title");
        Self { title: t }
    }
}
