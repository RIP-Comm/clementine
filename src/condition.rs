/// Arm opcode condition.
#[derive(Debug, Eq, PartialEq)]
pub enum Condition {
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
            0x0 => Self::EQ,
            0x1 => Self::NE,
            0x2 => Self::CS,
            0x3 => Self::CC,
            0x4 => Self::MI,
            0x5 => Self::PL,
            0x6 => Self::VS,
            0x7 => Self::VC,
            0x8 => Self::HI,
            0x9 => Self::LS,
            0xA => Self::GE,
            0xB => Self::LT,
            0xC => Self::GT,
            0xD => Self::LE,
            0xE => Self::AL,
            0xF => Self::NV,
            _ => unreachable!(),
        }
    }
}

/// Control bits condition.
pub enum ModeBits {
    OldUser = 0x00,
    OldFiq = 0x01,
    OldIrq = 0x02,
    OldSupervisor = 0x03,
    User = 0x10,
    Fiq = 0x11,
    Irq = 0x12,
    Supervisor = 0x13,
    Abort = 0x17,
    Undefined = 0x1B,
    System = 0x1F,
}

impl From<u8> for ModeBits {
    fn from(item: u8) -> Self {
        match item {
            0x00 => Self::OldUser,
            0x01 => Self::OldFiq,
            0x02 => Self::OldIrq,
            0x03 => Self::OldSupervisor,
            0x10 => Self::User,
            0x11 => Self::Fiq,
            0x12 => Self::Irq,
            0x13 => Self::Supervisor,
            0x17 => Self::Abort,
            0x1B => Self::Undefined,
            0x1F => Self::System,
            _ => unreachable!(),
        }
    }
}
