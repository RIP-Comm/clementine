//! # Instruction Encoding Flags and Types
//!
//! Common types used across ARM and Thumb instruction decoding and execution.
//! These represent the various fields and flags encoded in instructions.
//!
//! ## Memory Access Types
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    Memory Access Instruction Flags                      │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  LoadStoreKind:    Load (LDR) vs Store (STR)                           │
//! │  ReadWriteKind:    Word (32-bit) vs Byte (8-bit)                       │
//! │  Indexing:         Pre (calculate address before) vs Post (after)      │
//! │  Offsetting:       Up (add offset) vs Down (subtract offset)           │
//! │                                                                         │
//! │  Example: LDRB R0, [R1, #4]!                                           │
//! │           ↑  ↑      ↑   ↑ ↑                                            │
//! │           │  │      │   │ └─ Write-back (update R1)                    │
//! │           │  │      │   └─── Offset = 4                                │
//! │           │  │      └─────── Pre-indexed                               │
//! │           │  └────────────── Byte access                               │
//! │           └───────────────── Load operation                            │
//! │                                                                         │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Shift Operations
//!
//! The ARM barrel shifter can perform these shifts at no extra cost:
//!
//! | `ShiftKind` | Operation           | Example          | Result           |
//! |-----------|---------------------|------------------|------------------|
//! | LSL       | Logical Shift Left  | 0x0F LSL #4      | 0xF0             |
//! | LSR       | Logical Shift Right | 0xF0 LSR #4      | 0x0F             |
//! | ASR       | Arithmetic Shift R  | 0x80 ASR #4      | 0xF8 (sign ext)  |
//! | ROR       | Rotate Right        | 0x0F ROR #4      | 0xF0000000       |
//!
//! ## Operand Types
//!
//! Instructions can use immediate values or register values:
//!
//! ```text
//! ADD R0, R1, #10      ; Immediate: OperandKind::Immediate
//! ADD R0, R1, R2       ; Register:  OperandKind::Register
//! ADD R0, R1, R2, LSL #3  ; Register with shift
//! ```
use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

/// Specifies whether an operand is an immediate value or a register.
///
/// In ARM instructions, the I bit (bit 25) typically indicates this:
/// - I=0: Operand is a (possibly shifted) register
/// - I=1: Operand is a rotated immediate
///
/// Note: For some instructions (like Single Data Transfer), the meaning
/// is inverted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OperandKind {
    /// The operand is an immediate value embedded in the instruction.
    Immediate,
    /// The operand comes from a register (possibly with a shift applied).
    Register,
}

impl From<bool> for OperandKind {
    fn from(b: bool) -> Self {
        if b { Self::Immediate } else { Self::Register }
    }
}

/// Operation for Thumb "Move Compare Add Subtract Immediate" instructions.
///
/// These are the basic operations that can be performed with an 8-bit immediate
/// value in Thumb mode (Format 3 instructions).
///
/// ```text
/// Instruction format: 001 Op Rd Offset8
///                         ↑
///                         Operation (2 bits)
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    /// `MOV Rd, #Imm` - Move immediate to register
    Mov,
    /// `CMP Rd, #Imm` - Compare register with immediate (sets flags only)
    Cmp,
    /// `ADD Rd, #Imm` - Add immediate to register
    Add,
    /// `SUB Rd, #Imm` - Subtract immediate from register
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

/// The type of shift operation performed by the barrel shifter.
///
/// The ARM7TDMI has a barrel shifter that can shift/rotate the second operand
/// as part of data processing instructions, at no additional cycle cost.
///
/// ## Shift Operations Explained
///
/// ```text
/// Original: 0b11110000 (0xF0)
///
/// LSL #2:   0b11000000 (0xC0)    ← Bits shift left, zeros fill from right
///              ←←
///
/// LSR #2:   0b00111100 (0x3C)    ← Bits shift right, zeros fill from left
///                →→
///
/// ASR #2:   0b11111100 (0xFC)    ← Bits shift right, sign bit fills from left
///           ~~   →→              (preserves sign for signed numbers)
///
/// ROR #2:   0b00111100 (0x3C)    ← Bits rotate right, falling bits wrap to left
///           ↺↺
/// ```
///
/// ## Special Cases
///
/// - **LSL #0**: No shift, but carry flag may be affected
/// - **LSR #0**: Encoded as LSR #32 (result is 0, carry = bit 31)
/// - **ASR #0**: Encoded as ASR #32 (result is all 0s or all 1s based on sign)
/// - **ROR #0**: Encoded as RRX (rotate right through carry by 1)
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShiftKind {
    /// Logical Shift Left - shift bits left, fill with zeros.
    Lsl,
    /// Logical Shift Right - shift bits right, fill with zeros.
    Lsr,
    /// Arithmetic Shift Right - shift bits right, fill with sign bit (preserves sign).
    Asr,
    /// Rotate Right - bits that fall off the right wrap around to the left.
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

/// The data size for memory read/write operations.
///
/// Determined by the B bit in load/store instructions:
/// - B=0: Word access (32-bit for ARM, 16-bit for some Thumb ops)
/// - B=1: Byte access (8-bit)
///
/// ## Alignment Rules
///
/// ```text
/// Word access (LDR/STR):
///   - Address should be word-aligned (bits 0-1 = 00)
///   - Misaligned reads are rotated (GBA quirk, not trapped)
///
/// Byte access (LDRB/STRB):
///   - Any address is valid
///   - LDRB zero-extends to 32 bits
///   - LDRSB sign-extends to 32 bits
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReadWriteKind {
    /// Word access (32-bit). Default for most load/store operations.
    #[default]
    Word,

    /// Byte access (8-bit). Used with `LDRB`/`STRB` instructions.
    Byte,
}

impl From<bool> for ReadWriteKind {
    fn from(value: bool) -> Self {
        if value { Self::Byte } else { Self::Word }
    }
}

impl From<u32> for ReadWriteKind {
    fn from(op_code: u32) -> Self {
        op_code.get_bit(22).into()
    }
}

/// Whether a memory operation is a load (read) or store (write).
///
/// Determined by the L bit in load/store instructions:
/// - L=0: Store (write register to memory)
/// - L=1: Load (read memory to register)
///
/// ```text
/// STR R0, [R1]    ; Store: Memory[R1] = R0
/// LDR R0, [R1]    ; Load:  R0 = Memory[R1]
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LoadStoreKind {
    /// Store: write a register value to memory.
    Store,
    /// Load: read a value from memory into a register.
    Load,
}

impl From<bool> for LoadStoreKind {
    fn from(b: bool) -> Self {
        if b { Self::Load } else { Self::Store }
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

/// When to apply the offset in indexed addressing modes.
///
/// Determined by the P bit in load/store instructions:
/// - P=0: Post-indexed (offset applied after the transfer)
/// - P=1: Pre-indexed (offset applied before the transfer)
///
/// ## Pre-indexed vs Post-indexed
///
/// ```text
/// Pre-indexed (P=1):
///   LDR R0, [R1, #4]     ; Address = R1 + 4, then load
///   LDR R0, [R1, #4]!    ; Same + write-back: R1 = R1 + 4
///
/// Post-indexed (P=0):
///   LDR R0, [R1], #4     ; Address = R1, load, then R1 = R1 + 4
///                        ; (always writes back)
/// ```
///
/// Post-indexed addressing always writes back the calculated address,
/// regardless of the W bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Indexing {
    /// Post-indexed: use base address for transfer, then add offset to base.
    /// Always writes back the new address to the base register.
    Post,

    /// Pre-indexed: add offset to base before transfer.
    /// May optionally write back the new address (if W bit is set).
    Pre,
}

impl From<bool> for Indexing {
    fn from(state: bool) -> Self {
        if state { Self::Pre } else { Self::Post }
    }
}

/// The direction of the offset in indexed addressing modes.
///
/// Determined by the U bit in load/store instructions:
/// - U=0: Subtract offset from base (Down)
/// - U=1: Add offset to base (Up)
///
/// ```text
/// U=1 (Up):   LDR R0, [R1, #4]   ; Address = R1 + 4
/// U=0 (Down): LDR R0, [R1, #-4]  ; Address = R1 - 4
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Offsetting {
    /// Subtract the offset from the base address.
    Down,

    /// Add the offset to the base address.
    Up,
}

impl From<bool> for Offsetting {
    fn from(state: bool) -> Self {
        if state { Self::Up } else { Self::Down }
    }
}

/// The offset type for halfword/signed byte transfer instructions.
///
/// These instructions (`LDRH`, `STRH`, `LDRSB`, `LDRSH`) support two offset modes:
///
/// ## Immediate Offset
///
/// An 8-bit immediate split across two fields in the instruction:
/// ```text
/// Bits 11-8: High nibble of offset
/// Bits 3-0:  Low nibble of offset
/// Combined:  (high << 4) | low
/// ```
///
/// ## Register Offset
///
/// A register containing the offset value. Only the lower 4 bits of the
/// instruction specify the register number.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HalfwordDataTransferOffsetKind {
    /// Immediate offset (8-bit value split across instruction fields).
    Immediate {
        /// The combined offset value.
        offset: u32,
    },
    /// Register offset (offset is the value in the specified register).
    Register {
        /// The register number (0-15) containing the offset.
        register: u32,
    },
}
