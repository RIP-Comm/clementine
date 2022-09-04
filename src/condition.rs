/// Arm opcode condition.
pub(crate) enum Condition {
    EQ = 0x0,
    NE = 0x1,
    CS = 0x2,
    CC = 0x3,
    MI = 0x4,
    PL = 0x5,
    VS = 0x6,
    VC = 0x7,
    HI = 0x8,
    LS = 0x9,
    GE = 0xA,
    LT = 0xB,
    GT = 0xC,
    LE = 0xD,
    AL = 0xE,
    NV = 0xF,
}

impl From<u8> for Condition {
    fn from(item: u8) -> Self {
        match item {
            0xA => Condition::GE,
            0xE => Condition::AL,
            _ => todo!(),
        }
    }
}
