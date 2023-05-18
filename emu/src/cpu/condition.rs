/// In ARM state, all instructions are conditionally executed according to the state of the CPSR,
/// condition codes and the instruction’s condition field.
/// This field (bits 31:28) determines the circumstances under which an instruction is to be executed.
/// If the state of the C, N, Z and V flags fulfils the conditions encoded by the field,
/// the instruction is executed, otherwise it is ignored.
/// In the absence of a suffix, the condition field of most instructions is set to "Always" (sufix AL).
/// This means the instruction will always be executed regardless of the CPSR condition codes.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Condition {
    /// Z set (equal).
    EQ = 0x0,

    /// Z clear (not equal).
    NE = 0x1,

    /// C set (unsigned higher or same).
    CS = 0x2,

    /// C clear (unsigned lower).
    CC = 0x3,

    /// N set (negative).
    MI = 0x4,

    /// N clear (positive or zero).
    PL = 0x5,

    /// V set (overflow).
    VS = 0x6,

    /// V clear (no overflow).
    VC = 0x7,

    /// C set and Z clear (unsigned higher).
    HI = 0x8,

    /// C clear or Z set (unsigned lower or same).
    LS = 0x9,

    /// N equals V (greater or equal).
    GE = 0xA,

    /// N not equal to V (less then).
    LT = 0xB,

    /// Z clear AND (N equals V) (greater then).
    GT = 0xC,

    /// Z set OR (N not equals V) (less then or equal).
    LE = 0xD,

    /// ignored.
    AL = 0xE,

    /// The sixteenth (1111) is reserved, and must not be used.
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

impl std::fmt::Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EQ => f.write_str("EQ"),
            Self::NE => f.write_str("NE"),
            Self::CS => f.write_str("CS"),
            Self::CC => f.write_str("CC"),
            Self::MI => f.write_str("MI"),
            Self::PL => f.write_str("PL"),
            Self::VS => f.write_str("VS"),
            Self::VC => f.write_str("VC"),
            Self::HI => f.write_str("HI"),
            Self::LS => f.write_str("LS"),
            Self::GE => f.write_str("GE"),
            Self::LT => f.write_str("LT"),
            Self::GT => f.write_str("GT"),
            Self::LE => f.write_str("LE"),
            Self::AL => Ok(()),
            Self::NV => f.write_str("_NEVER"),
        }
    }
}
