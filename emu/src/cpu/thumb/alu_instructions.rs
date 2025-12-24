//! # Thumb Mode ALU Instructions
//!
//! This module defines the ALU operations available in Thumb mode (Format 4).
//!
//! ## Differences from ARM ALU Operations
//!
//! Thumb ALU instructions are more limited than ARM:
//! - Only operate on low registers (R0-R7)
//! - Always update condition flags (no S bit to control this)
//! - Shifts are separate instructions, not combined with ALU ops
//! - Include some operations not in ARM's data processing (NEG, MUL)
//!
//! ## Instruction Format (Format 4)
//!
//! ```text
//! 15  14  13  12  11  10  9   8   7   6   5   4   3   2   1   0
//! [ 0   1   0   0   0   0 ] [  Op (4 bits)  ] [  Rs ] [  Rd  ]
//!                               ↑               ↑       ↑
//!                               │               │       └─ Destination/operand register
//!                               │               └───────── Source register
//!                               └───────────────────────── Operation (see table below)
//! ```
//!
//! ## Operation Table
//!
//! ```text
//! ┌──────┬──────┬─────────────────────────────────────────────────────────┐
//! │  Op  │ Inst │ Operation                                               │
//! ├──────┼──────┼─────────────────────────────────────────────────────────┤
//! │ 0000 │ AND  │ Rd = Rd AND Rs                                          │
//! │ 0001 │ EOR  │ Rd = Rd XOR Rs                                          │
//! │ 0010 │ LSL  │ Rd = Rd << Rs (logical shift left)                      │
//! │ 0011 │ LSR  │ Rd = Rd >> Rs (logical shift right)                     │
//! │ 0100 │ ASR  │ Rd = Rd >> Rs (arithmetic shift right, sign-extended)   │
//! │ 0101 │ ADC  │ Rd = Rd + Rs + C (add with carry)                       │
//! │ 0110 │ SBC  │ Rd = Rd - Rs - !C (subtract with carry)                 │
//! │ 0111 │ ROR  │ Rd = Rd rotated right by Rs                             │
//! │ 1000 │ TST  │ Set flags on Rd AND Rs (result discarded)               │
//! │ 1001 │ NEG  │ Rd = 0 - Rs (negate, not available in ARM data proc)    │
//! │ 1010 │ CMP  │ Set flags on Rd - Rs (result discarded)                 │
//! │ 1011 │ CMN  │ Set flags on Rd + Rs (result discarded)                 │
//! │ 1100 │ ORR  │ Rd = Rd OR Rs                                           │
//! │ 1101 │ MUL  │ Rd = Rd * Rs (multiply, not in ARM data processing)     │
//! │ 1110 │ BIC  │ Rd = Rd AND NOT Rs (bit clear)                          │
//! │ 1111 │ MVN  │ Rd = NOT Rs (move not)                                  │
//! └──────┴──────┴─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Examples
//!
//! ```text
//! AND R0, R1      ; R0 = R0 AND R1, flags updated
//! LSL R2, R3      ; R2 = R2 << R3, flags updated
//! NEG R4, R5      ; R4 = -R5 (0 - R5), flags updated
//! MUL R6, R7      ; R6 = R6 * R7, flags updated
//! ```

use serde::{Deserialize, Serialize};

/// Thumb mode ALU operation codes (Format 4 instructions).
///
/// These operations work on low registers (R0-R7) and always update flags.
/// The destination register is also the first operand (except for NEG and MVN).
#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum ThumbModeAluInstruction {
    /// `AND Rd, Rs` - Rd = Rd AND Rs
    And = 0x0,
    /// `EOR Rd, Rs` - Rd = Rd XOR Rs
    Eor = 0x1,
    /// `LSL Rd, Rs` - Rd = Rd << Rs (shift amount from Rs)
    Lsl = 0x2,
    /// `LSR Rd, Rs` - Rd = Rd >> Rs (logical, shift amount from Rs)
    Lsr = 0x3,
    /// `ASR Rd, Rs` - Rd = Rd >> Rs (arithmetic, preserves sign)
    Asr = 0x4,
    /// `ADC Rd, Rs` - Rd = Rd + Rs + Carry
    Adc = 0x5,
    /// `SBC Rd, Rs` - Rd = Rd - Rs - NOT(Carry)
    Sbc = 0x6,
    /// `ROR Rd, Rs` - Rd = Rd rotated right by Rs
    Ror = 0x7,
    /// `TST Rd, Rs` - Set flags on Rd AND Rs (result discarded)
    Tst = 0x8,
    /// `NEG Rd, Rs` - Rd = 0 - Rs (negate). Not in ARM data processing!
    Neg = 0x9,
    /// `CMP Rd, Rs` - Set flags on Rd - Rs (result discarded)
    Cmp = 0xA,
    /// `CMN Rd, Rs` - Set flags on Rd + Rs (result discarded)
    Cmn = 0xB,
    /// `ORR Rd, Rs` - Rd = Rd OR Rs
    Orr = 0xC,
    /// `MUL Rd, Rs` - Rd = Rd * Rs. Not in ARM data processing!
    Mul = 0xD,
    /// `BIC Rd, Rs` - Rd = Rd AND NOT Rs (bit clear)
    Bic = 0xE,
    /// `MVN Rd, Rs` - Rd = NOT Rs
    Mvn = 0xF,
}

impl From<u16> for ThumbModeAluInstruction {
    fn from(alu_op_code: u16) -> Self {
        use ThumbModeAluInstruction::{
            Adc, And, Asr, Bic, Cmn, Cmp, Eor, Lsl, Lsr, Mul, Mvn, Neg, Orr, Ror, Sbc, Tst,
        };
        match alu_op_code {
            0x0 => And,
            0x1 => Eor,
            0x2 => Lsl,
            0x3 => Lsr,
            0x4 => Asr,
            0x5 => Adc,
            0x6 => Sbc,
            0x7 => Ror,
            0x8 => Tst,
            0x9 => Neg,
            0xA => Cmp,
            0xB => Cmn,
            0xC => Orr,
            0xD => Mul,
            0xE => Bic,
            0xF => Mvn,
            _ => unreachable!(),
        }
    }
}

/// Operations for Thumb "High Register Operations / Branch Exchange" (Format 5).
///
/// These are the only Thumb instructions that can access high registers (R8-R15).
/// They're encoded specially to allow specifying registers beyond R0-R7.
///
/// ## Instruction Format (Format 5)
///
/// ```text
/// 15  14  13  12  11  10  9   8   7   6   5   4   3   2   1   0
/// [ 0   1   0   0   0   1 ] [Op] [H1] [H2] [   Rs/Hs   ] [Rd/Hd]
///                            ↑    ↑    ↑
///                            │    │    └─ 1 = Rs is R8-R15, 0 = Rs is R0-R7
///                            │    └────── 1 = Rd is R8-R15, 0 = Rd is R0-R7
///                            └─────────── Operation (2 bits)
/// ```
///
/// ## Operations
///
/// | Op  | Instruction | Description                              |
/// |-----|-------------|------------------------------------------|
/// | 00  | ADD         | Rd = Rd + Rs (no flag update!)           |
/// | 01  | CMP         | Compare Rd with Rs (flags only)          |
/// | 10  | MOV         | Rd = Rs (no flag update!)                |
/// | 11  | BX/BLX      | Branch to Rs, exchange if Rs\[0\]=1      |
///
/// ## Important Notes
///
/// - ADD and MOV in this format do NOT update flags (unlike Format 4)
/// - BX to an address with bit 0 set switches to Thumb mode
/// - BX to an address with bit 0 clear switches to ARM mode
/// - Using R15 (PC) as Rs gives PC+4 due to pipeline
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThumbHighRegisterOperation {
    /// `ADD Rd, Rs` - Add without flag update. Can use high registers.
    Add,
    /// `CMP Rd, Rs` - Compare and set flags. Can use high registers.
    Cmp,
    /// `MOV Rd, Rs` - Move without flag update. Can use high registers.
    Mov,
    /// `BX Rs` / `BLX Rs` - Branch (and link) with exchange.
    /// Switches to ARM mode if Rs bit 0 is clear.
    BxOrBlx,
}

impl std::fmt::Display for ThumbHighRegisterOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mov => f.write_str("MOV"),
            Self::Cmp => f.write_str("CMP"),
            Self::Add => f.write_str("ADD"),
            Self::BxOrBlx => f.write_str("BX"),
        }
    }
}

impl From<u16> for ThumbHighRegisterOperation {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Add,
            1 => Self::Cmp,
            2 => Self::Mov,
            3 => Self::BxOrBlx,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion_thumb_alu_op() {
        let op: ThumbModeAluInstruction = 0b0000.into();
        assert_eq!(op, ThumbModeAluInstruction::And);
        let op: ThumbModeAluInstruction = 0b0001.into();
        assert_eq!(op, ThumbModeAluInstruction::Eor);
        let op: ThumbModeAluInstruction = 0b1110.into();
        assert_eq!(op, ThumbModeAluInstruction::Bic);
        let op: ThumbModeAluInstruction = 0b1111.into();
        assert_eq!(op, ThumbModeAluInstruction::Mvn);
    }
}
