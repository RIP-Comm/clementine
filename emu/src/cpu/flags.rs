use crate::bitwise::Bits;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperandKind {
    Immediate,
    Register,
}

impl From<bool> for OperandKind {
    fn from(b: bool) -> Self {
        match b {
            false => Self::Register,
            true => Self::Immediate,
        }
    }
}

/// Operation to perform in the Move Compare Add Subtract Immediate instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Operation {
    Mov,
    Cmp,
    Add,
    Sub,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mov => f.write_str("MOV"),
            Self::Cmp => f.write_str("CMP"),
            Self::Add => f.write_str("ADD"),
            Self::Sub => f.write_str("SUB"),
        }
    }
}

impl From<u16> for Operation {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Mov,
            1 => Self::Cmp,
            2 => Self::Add,
            3 => Self::Sub,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShiftKind {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

impl From<u16> for ShiftKind {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            3 => Self::Ror,
            _ => unreachable!(),
        }
    }
}

impl From<u32> for ShiftKind {
    fn from(op: u32) -> Self {
        match op {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            3 => Self::Ror,
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for ShiftKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lsl => f.write_str("LSL"),
            Self::Lsr => f.write_str("LSR"),
            Self::Asr => f.write_str("ASR"),
            Self::Ror => f.write_str("ROR"),
        }
    }
}

/// There two different kind of write or read for memory.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReadWriteKind {
    /// Word is a u32 value for ARM mode and u16 for Thumb mode.
    #[default]
    Word,

    /// Byte is a u8 value.
    Byte,
}

impl From<bool> for ReadWriteKind {
    fn from(value: bool) -> Self {
        if value {
            Self::Byte
        } else {
            Self::Word
        }
    }
}

impl From<u32> for ReadWriteKind {
    fn from(op_code: u32) -> Self {
        op_code.get_bit(22).into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadStoreKind {
    Store,
    Load,
}

impl From<bool> for LoadStoreKind {
    fn from(b: bool) -> Self {
        match b {
            false => Self::Store,
            true => Self::Load,
        }
    }
}

impl std::fmt::Display for LoadStoreKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load => write!(f, "LDR"),
            Self::Store => write!(f, "STR"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Indexing {
    /// Add offset after transfer.
    Post,

    /// Add offset before transfer.
    Pre,
}

impl From<bool> for Indexing {
    fn from(state: bool) -> Self {
        match state {
            false => Self::Post,
            true => Self::Pre,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Offsetting {
    /// Subtract the offset from base.
    Down,

    /// Add the offset to base.
    Up,
}

impl From<bool> for Offsetting {
    fn from(state: bool) -> Self {
        match state {
            false => Self::Down,
            true => Self::Up,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HalfwordDataTransferOffsetKind {
    Immediate { offset: u32 },
    Register { register: u32 },
}
