use crate::bitwise::Bits;

/// There two different kind of write or read for memory.
#[derive(Default, Debug, PartialEq, Eq)]
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

#[derive(PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub enum Offsetting {
    /// Substract the offset from base.
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
