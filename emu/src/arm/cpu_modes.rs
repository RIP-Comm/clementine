#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
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
