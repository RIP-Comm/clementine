#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    /// The normal ARM program execution state.
    User = 0b10000,

    /// Designed to support a data transfer or channel process.
    Fiq = 0b10001,

    /// Used for general-purpose interrupt handling.
    Irq = 0b10010,

    /// Protected mode for the operating system
    Supervisor = 0b10011,

    /// Entered after a data or instruction prefetch abort.
    Abort = 0b10111,

    /// Entered when an undefined instruction is executed
    Undefined = 0b11011,

    /// A privileged user mode for the operating system.
    System = 0b11111,
}

impl From<Mode> for u32 {
    fn from(m: Mode) -> Self {
        m as Self
    }
}

impl TryFrom<u32> for Mode {
    type Error = String;

    fn try_from(n: u32) -> Result<Self, Self::Error> {
        match n {
            0b10000 => Ok(Self::User),
            0b10001 => Ok(Self::Fiq),
            0b10010 => Ok(Self::Irq),
            0b10011 => Ok(Self::Supervisor),
            0b10111 => Ok(Self::Abort),
            0b11011 => Ok(Self::Undefined),
            0b11111 => Ok(Self::System),
            _ => Err(String::from("Unexpected value for Mode")),
        }
    }
}

/// Represents the CPU state (ARM/THUMB).
pub enum CpuState {
    /// Which operates with 16-bit, halfword-aligned THUMB instructions.
    /// In this state, the PC uses bit 1 to select between alternate halfwords.
    Thumb,

    /// Which executes 32-bit, word-aligned ARM instructions.
    Arm,
}

impl From<CpuState> for bool {
    fn from(state: CpuState) -> Self {
        match state {
            CpuState::Arm => false,
            CpuState::Thumb => true,
        }
    }
}

impl From<bool> for CpuState {
    fn from(state: bool) -> Self {
        match state {
            true => Self::Thumb,
            false => Self::Arm,
        }
    }
}
