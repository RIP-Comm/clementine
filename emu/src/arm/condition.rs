/// In ARM state, all instructions are conditionally executed according to the state of the CPSR,
///  condition codes and the instruction’s condition field.
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
