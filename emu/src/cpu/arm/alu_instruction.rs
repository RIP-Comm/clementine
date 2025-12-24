//! # ARM ALU Instructions and Barrel Shifter
//!
//! This module implements the ARM data processing instructions (ALU operations)
//! and the barrel shifter that provides free shifts on the second operand.
//!
//! ## Data Processing Instruction Format
//!
//! ```text
//! 31-28  27-26  25   24-21   20   19-16  15-12  11-0
//! [Cond] [ 00 ] [I] [OpCode] [S] [ Rn ] [ Rd ] [Operand2]
//!                ↑     ↑      ↑    ↑      ↑       ↑
//!                │     │      │    │      │       └─ Second operand (see below)
//!                │     │      │    │      └───────── Destination register
//!                │     │      │    └──────────────── First operand register
//!                │     │      └───────────────────── Set condition codes (S bit)
//!                │     └──────────────────────────── ALU operation (4 bits)
//!                └────────────────────────────────── Immediate operand flag
//! ```
//!
//! ## The 16 ALU Operations
//!
//! ```text
//! ┌────────┬─────────┬────────────────────────────────────────────────────────┐
//! │ OpCode │  Instr  │ Operation                                              │
//! ├────────┼─────────┼────────────────────────────────────────────────────────┤
//! │  0000  │   AND   │ Rd = Rn AND Op2        (Logical AND)                   │
//! │  0001  │   EOR   │ Rd = Rn XOR Op2        (Exclusive OR)                  │
//! │  0010  │   SUB   │ Rd = Rn - Op2          (Subtract)                      │
//! │  0011  │   RSB   │ Rd = Op2 - Rn          (Reverse Subtract)              │
//! │  0100  │   ADD   │ Rd = Rn + Op2          (Add)                           │
//! │  0101  │   ADC   │ Rd = Rn + Op2 + C      (Add with Carry)                │
//! │  0110  │   SBC   │ Rd = Rn - Op2 - !C     (Subtract with Carry)           │
//! │  0111  │   RSC   │ Rd = Op2 - Rn - !C     (Reverse Subtract with Carry)   │
//! │  1000  │   TST   │ Rn AND Op2, flags only (Test bits)                     │
//! │  1001  │   TEQ   │ Rn XOR Op2, flags only (Test Equivalence)              │
//! │  1010  │   CMP   │ Rn - Op2, flags only   (Compare)                       │
//! │  1011  │   CMN   │ Rn + Op2, flags only   (Compare Negative)              │
//! │  1100  │   ORR   │ Rd = Rn OR Op2         (Logical OR)                    │
//! │  1101  │   MOV   │ Rd = Op2               (Move, Rn ignored)              │
//! │  1110  │   BIC   │ Rd = Rn AND NOT Op2    (Bit Clear)                     │
//! │  1111  │   MVN   │ Rd = NOT Op2           (Move Not, Rn ignored)          │
//! └────────┴─────────┴────────────────────────────────────────────────────────┘
//! ```
//!
//! ## The Barrel Shifter
//!
//! The second operand (Op2) can be shifted before the ALU operation at no
//! additional cycle cost. This is one of ARM's most powerful features.
//!
//! ### Operand2 Formats
//!
//! **When I=0 (Register with shift):**
//! ```text
//! Bits 11-4: Shift amount/register
//! Bits 3-0:  Rm (register to shift)
//!
//! Shift by immediate:  [11-7: amount][6-5: type][4: 0][3-0: Rm]
//! Shift by register:   [11-8: Rs][7: 0][6-5: type][4: 1][3-0: Rm]
//! ```
//!
//! **When I=1 (Immediate with rotation):**
//! ```text
//! Bits 11-8: Rotate amount (multiplied by 2)
//! Bits 7-0:  8-bit immediate value
//!
//! Result = imm8 ROR (rotate * 2)
//! ```
//!
//! ### Shift Types
//!
//! | Type  | Encoding | Description              |
//! |-------|----------|--------------------------|
//! | LSL   | 00       | Logical Shift Left       |
//! | LSR   | 01       | Logical Shift Right      |
//! | ASR   | 10       | Arithmetic Shift Right   |
//! | ROR   | 11       | Rotate Right             |
//!
//! ### Examples
//!
//! ```text
//! ADD R0, R1, R2          ; R0 = R1 + R2
//! ADD R0, R1, R2, LSL #3  ; R0 = R1 + (R2 << 3)
//! ADD R0, R1, R2, LSL R3  ; R0 = R1 + (R2 << R3)
//! ADD R0, R1, #0x1F00     ; R0 = R1 + 0x1F00 (immediate)
//! ```

use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;
use crate::cpu::flags::ShiftKind;

/// ARM mode ALU instruction opcodes.
///
/// These are the 16 data processing operations encoded in bits 24-21
/// of ARM data processing instructions.
///
/// Operations are divided into:
/// - **Logical**: AND, EOR, TST, TEQ, ORR, MOV, BIC, MVN
/// - **Arithmetic**: SUB, RSB, ADD, ADC, SBC, RSC, CMP, CMN
///
/// The distinction matters for how the carry flag is set:
/// - Logical operations: carry comes from the barrel shifter
/// - Arithmetic operations: carry comes from the ALU operation itself
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArmModeAluInstr {
    /// Logical AND: `Rd = Rn AND Op2`
    And = 0x0,
    /// Exclusive OR: `Rd = Rn XOR Op2`
    Eor = 0x1,
    /// Subtract: `Rd = Rn - Op2`
    Sub = 0x2,
    /// Reverse Subtract: `Rd = Op2 - Rn`
    Rsb = 0x3,
    /// Add: `Rd = Rn + Op2`
    Add = 0x4,
    /// Add with Carry: `Rd = Rn + Op2 + C`
    Adc = 0x5,
    /// Subtract with Carry: `Rd = Rn - Op2 - !C`
    Sbc = 0x6,
    /// Reverse Subtract with Carry: `Rd = Op2 - Rn - !C`
    Rsc = 0x7,
    /// Test bits (AND, flags only, no result written)
    Tst = 0x8,
    /// Test Equivalence (XOR, flags only, no result written)
    Teq = 0x9,
    /// Compare (SUB, flags only, no result written)
    Cmp = 0xA,
    /// Compare Negative (ADD, flags only, no result written)
    Cmn = 0xB,
    /// Logical OR: `Rd = Rn OR Op2`
    Orr = 0xC,
    /// Move: `Rd = Op2` (Rn is ignored)
    Mov = 0xD,
    /// Bit Clear: `Rd = Rn AND NOT Op2`
    Bic = 0xE,
    /// Move Not: `Rd = NOT Op2` (Rn is ignored)
    Mvn = 0xF,
}

impl std::fmt::Display for ArmModeAluInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::And => f.write_str("AND"),
            Self::Eor => f.write_str("EOR"),
            Self::Sub => f.write_str("SUB"),
            Self::Rsb => f.write_str("RSB"),
            Self::Add => f.write_str("ADD"),
            Self::Adc => f.write_str("ADC"),
            Self::Sbc => f.write_str("SBC"),
            Self::Rsc => f.write_str("RSC"),
            Self::Tst => f.write_str("TST"),
            Self::Teq => f.write_str("TEQ"),
            Self::Cmp => f.write_str("CMP"),
            Self::Cmn => f.write_str("CMN"),
            Self::Orr => f.write_str("ORR"),
            Self::Mov => f.write_str("MOV"),
            Self::Bic => f.write_str("BIC"),
            Self::Mvn => f.write_str("MVN"),
        }
    }
}

/// Classification of ALU instructions for flag handling.
///
/// This distinction determines how the carry flag is updated:
/// - **Logical**: Carry comes from the barrel shifter operation
/// - **Arithmetic**: Carry comes from the ALU add/subtract operation
#[derive(Eq, PartialEq, Debug)]
pub enum AIKind {
    /// Logical operations (AND, EOR, TST, TEQ, ORR, MOV, BIC, MVN).
    /// Carry flag is set by the barrel shifter, not the ALU.
    Logical,
    /// Arithmetic operations (ADD, ADC, SUB, SBC, RSB, RSC, CMP, CMN).
    /// Carry flag is set by the arithmetic operation itself.
    Arithmetic,
}

/// Trait to classify ALU instructions as logical or arithmetic.
pub trait Kind {
    /// Returns whether this is a logical or arithmetic operation.
    fn kind(&self) -> AIKind;
}

impl Kind for ArmModeAluInstr {
    fn kind(&self) -> AIKind {
        match &self {
            Self::And
            | Self::Eor
            | Self::Tst
            | Self::Teq
            | Self::Orr
            | Self::Mov
            | Self::Bic
            | Self::Mvn => AIKind::Logical,
            Self::Sub
            | Self::Rsb
            | Self::Add
            | Self::Adc
            | Self::Sbc
            | Self::Rsc
            | Self::Cmp
            | Self::Cmn => AIKind::Arithmetic,
        }
    }
}

impl From<u32> for ArmModeAluInstr {
    fn from(alu_op_code: u32) -> Self {
        match alu_op_code {
            0x0 => Self::And,
            0x1 => Self::Eor,
            0x2 => Self::Sub,
            0x3 => Self::Rsb,
            0x4 => Self::Add,
            0x5 => Self::Adc,
            0x6 => Self::Sbc,
            0x7 => Self::Rsc,
            0x8 => Self::Tst,
            0x9 => Self::Teq,
            0xA => Self::Cmp,
            0xB => Self::Cmn,
            0xC => Self::Orr,
            0xD => Self::Mov,
            0xE => Self::Bic,
            0xF => Self::Mvn,
            _ => unreachable!(),
        }
    }
}

/// Result of an ALU or shift operation, including flags.
///
/// This struct captures both the computed result and the condition flags
/// that should be set if the S bit is set in the instruction.
///
/// ## Flag Meanings
///
/// - **carry**: For shifts, the last bit shifted out. For arithmetic, overflow past 32 bits.
/// - **overflow**: Signed overflow (result doesn't fit in signed 32-bit).
/// - **sign**: Bit 31 of the result (negative in signed interpretation).
/// - **zero**: Result is exactly zero.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
pub struct ArithmeticOpResult {
    /// The computed result value.
    pub result: u32,
    /// Carry flag (C): last bit shifted out, or unsigned overflow.
    pub carry: bool,
    /// Overflow flag (V): signed arithmetic overflow.
    pub overflow: bool,
    /// Sign flag (N): bit 31 of result.
    pub sign: bool,
    /// Zero flag (Z): result is zero.
    pub zero: bool,
}

/// Perform a barrel shifter operation.
///
/// The ARM barrel shifter can shift or rotate a value as part of the second
/// operand calculation. This function implements all four shift types with
/// their special cases.
///
/// # Arguments
///
/// * `kind` - The type of shift (LSL, LSR, ASR, ROR)
/// * `shift_amount` - How many bits to shift (0-255, but behavior varies)
/// * `rm` - The value to shift (from register Rm)
/// * `carry` - The current carry flag (used for RRX encoding)
///
/// # Returns
///
/// An [`ArithmeticOpResult`] with the shifted value and the new carry flag.
/// Note that `overflow`, `sign`, and `zero` are not set by this function.
///
/// # Special Cases
///
/// - `LSL #0`: No shift, carry unchanged
/// - `LSR #0`: Encodes `LSR #32`, result is 0, carry = bit 31
/// - `ASR #0`: Encodes `ASR #32`, result is sign-extended, carry = bit 31
/// - `ROR #0`: Encodes `RRX`, rotate right through carry by 1
pub fn shift(kind: ShiftKind, shift_amount: u32, rm: u32, carry: bool) -> ArithmeticOpResult {
    match kind {
        ShiftKind::Lsl => {
            match shift_amount {
                // LSL#0: No shift performed, ie. directly value=Rm, the C flag is NOT affected.
                0 => ArithmeticOpResult {
                    result: rm,
                    carry,
                    ..Default::default()
                },
                // LSL#1..32: Normal left logical shift
                1..=32 => {
                    // In Rust, when you use the << operator to shift a value to the left, the behavior is defined modulo the number of bits in the type.
                    // For a u32, there are 32 bits, so any left shift operation with a shift amount greater than or equal to 32 will wrap around and behave as
                    // if the shift amount is reduced modulo 32.
                    // So when you do 1 << 32 with a u32 in Rust, it is equivalent to 1 << (32 % 32), which is 1 << 0.
                    // Shifting a value 0 bits to the left is equivalent to the original value, so you get 1.
                    let rm = rm as u64;
                    let result = (rm << shift_amount) as u32;
                    ArithmeticOpResult {
                        result,
                        carry: rm.get_bit((32 - shift_amount).try_into().unwrap()),
                        ..Default::default()
                    }
                }
                // LSL#33...: Result is 0 and carry is 0
                _ => ArithmeticOpResult {
                    carry: false,
                    ..Default::default()
                },
            }
        }
        ShiftKind::Lsr => {
            match shift_amount {
                // LSR#0 is used to encode LSR#32, it has 0 result and carry equal to bit 31 of Rm
                0 => ArithmeticOpResult {
                    result: 0,
                    carry: rm.get_bit(31),
                    ..Default::default()
                },
                // LSR#1..32: Normal right logical shift
                1..=32 => {
                    // We do the shift in u64 for the same reason as above.
                    let rm = rm as u64;
                    let result = (rm >> shift_amount) as u32;

                    ArithmeticOpResult {
                        result,
                        carry: rm.get_bit((shift_amount - 1).try_into().unwrap()),
                        ..Default::default()
                    }
                }
                _ => ArithmeticOpResult {
                    result: 0,
                    carry: false,
                    ..Default::default()
                },
            }
        }
        ShiftKind::Asr => match shift_amount {
            1..=31 => ArithmeticOpResult {
                result: ((rm as i32) >> shift_amount) as u32,
                carry: rm.get_bit((shift_amount - 1).try_into().unwrap()),
                ..Default::default()
            },
            _ => ArithmeticOpResult {
                result: ((rm as i32) >> 31) as u32,
                carry: rm.get_bit(31),
                ..Default::default()
            },
        },
        ShiftKind::Ror => {
            // from documentation: ROR by n where n is greater than 32 will give the same
            // result and carry out as ROR by n-32; therefore repeatedly y subtract 32 from n until the amount is
            // in the range 1 to 32
            let mut new_shift_amount = shift_amount;

            if shift_amount > 32 {
                new_shift_amount %= 32;

                // if modulo operation yields 0 it means that shift_amount was a multiple of 32
                // so we should do ROR#32
                if new_shift_amount == 0 {
                    new_shift_amount = 32;
                }
            }

            match new_shift_amount {
                // ROR#0 is used to encode RRX (appending C to the left and shift right by 1)
                0 => {
                    let old_carry = carry as u32;

                    ArithmeticOpResult {
                        result: (rm >> 1) | (old_carry << 31),
                        carry: rm.get_bit(0),
                        ..Default::default()
                    }
                }

                // ROR#1..31: normal rotate right
                1..=31 => ArithmeticOpResult {
                    result: rm.rotate_right(new_shift_amount),
                    carry: rm.get_bit((new_shift_amount - 1).try_into().unwrap()),
                    ..Default::default()
                },

                // ROR#32 doesn't change rm but sets carry to bit 31 of rm
                32 => ArithmeticOpResult {
                    result: rm,
                    carry: rm.get_bit(31),
                    ..Default::default()
                },

                // ROR#i with i > 32 is the same of ROR#n where n = i % 32
                _ => unreachable!(),
            }
        }
    }
}

/// The type of PSR (Program Status Register) transfer operation.
///
/// These instructions allow reading and writing the CPSR (Current Program
/// Status Register) or SPSR (Saved Program Status Register).
///
/// ## Operations
///
/// - **MRS**: Read PSR into a general-purpose register
/// - **MSR**: Write a register to all PSR fields
/// - **MSR (flag bits only)**: Write to specific PSR fields using a mask
///
/// ## Field Mask (for MsrFlg)
///
/// The field mask selects which parts of the PSR to modify:
///
/// | Bit | Field  | PSR Bits | Description          |
/// |-----|--------|----------|----------------------|
/// | 3   | f      | 31-24    | Condition flags      |
/// | 2   | s      | 23-16    | Status (reserved)    |
/// | 1   | x      | 15-8     | Extension (reserved) |
/// | 0   | c      | 7-0      | Control bits         |
///
/// Example: `MSR CPSR_f, R0` only modifies the flag bits (N, Z, C, V).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsrOpKind {
    /// MRS: Transfer PSR contents to a register.
    /// `MRS Rd, CPSR` or `MRS Rd, SPSR`
    Mrs {
        /// The destination register to receive the PSR value.
        destination_register: u32,
    },
    /// MSR: Transfer register contents to PSR (all fields).
    /// `MSR CPSR, Rm` or `MSR SPSR, Rm`
    Msr {
        /// The source register containing the new PSR value.
        source_register: u32,
    },
    /// MSR with field mask: Transfer register/immediate to specific PSR fields.
    /// `MSR CPSR_f, Rm` or `MSR CPSR_fc, #imm`
    MsrFlg {
        /// The value to write (register or rotated immediate).
        operand: AluSecondOperandInfo,
        /// Mask selecting which PSR fields to modify (bits 19-16 of instruction).
        field_mask: u32,
    },
}

impl TryFrom<u32> for PsrOpKind {
    type Error = String;

    fn try_from(op_code: u32) -> Result<Self, Self::Error> {
        if op_code.get_bits(23..=27) == 0b0_0010
            && op_code.get_bits(16..=21) == 0b00_1111
            && op_code.get_bits(0..=11) == 0b0000_0000_0000
        {
            Ok(Self::Mrs {
                destination_register: op_code.get_bits(12..=15),
            })
        } else if op_code.get_bits(23..=27) == 0b00010
            && op_code.get_bits(12..=21) == 0b10_1001_1111
            && op_code.get_bits(4..=11) == 0b0000_0000
        {
            Ok(Self::Msr {
                source_register: op_code.get_bits(0..=3),
            })
        } else if op_code.get_bits(26..=27) == 0b00
            && op_code.get_bits(23..=24) == 0b10
            && op_code.get_bits(20..=21) == 0b10
            && op_code.get_bits(12..=15) == 0b1111
        {
            // MSR with field mask: can be immediate (bit 25=1) or register (bit 25=0)
            Ok(Self::MsrFlg {
                operand: if op_code.get_bit(25) {
                    // Immediate form
                    AluSecondOperandInfo::Immediate {
                        base: op_code.get_bits(0..=7),
                        shift: op_code.get_bits(8..=11) * 2,
                    }
                } else {
                    // Register form
                    AluSecondOperandInfo::Register {
                        shift_op: ShiftOperator::Immediate(0),
                        shift_kind: ShiftKind::Lsl,
                        register: op_code.get_bits(0..=3),
                    }
                },
                field_mask: op_code.get_bits(16..=19),
            })
        } else {
            Err(format!(
                "Invalid PSR operation opcode: 0x{:08X}\nBits 23-27: 0b{:05b}, Bits 16-21: 0b{:06b}, Bits 12-21: 0b{:010b}, Bits 0-11: 0b{:012b}",
                op_code,
                op_code.get_bits(23..=27),
                op_code.get_bits(16..=21),
                op_code.get_bits(12..=21),
                op_code.get_bits(0..=11)
            ))
        }
    }
}

/// Which Program Status Register to access.
///
/// - **CPSR**: Current Program Status Register (always accessible)
/// - **SPSR**: Saved Program Status Register (only in exception modes)
///
/// The SPSR holds the CPSR value from before the exception occurred,
/// allowing the original state to be restored on exception return.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsrKind {
    /// Current Program Status Register.
    Cpsr,
    /// Saved Program Status Register (banked per exception mode).
    Spsr,
}

impl From<bool> for PsrKind {
    fn from(value: bool) -> Self {
        if value { Self::Spsr } else { Self::Cpsr }
    }
}

impl std::fmt::Display for PsrKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpsr => write!(f, "CPSR"),
            Self::Spsr => write!(f, "SPSR"),
        }
    }
}

/// How the shift amount is specified for register operands.
///
/// In ARM data processing instructions with a register operand, the shift
/// amount can come from either:
/// - An immediate value encoded in the instruction (5 bits, 0-31)
/// - Another register (bottom 8 bits used, but only 0-255 meaningful)
///
/// ```text
/// Shift by immediate:  ADD R0, R1, R2, LSL #4   ; Shift R2 left by 4
/// Shift by register:   ADD R0, R1, R2, LSL R3   ; Shift R2 left by R3
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShiftOperator {
    /// Shift amount is an immediate value (0-31, or special encodings).
    Immediate(u32),
    /// Shift amount comes from a register (uses bottom 8 bits).
    Register(u32),
}

impl std::fmt::Display for ShiftOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Immediate(value) => write!(f, "#{value}"),
            Self::Register(register) => write!(f, "R{register}"),
        }
    }
}

/// The second operand for ALU data processing instructions.
///
/// ARM data processing instructions have a flexible second operand that can be:
/// 1. A register, optionally shifted by an immediate or register amount
/// 2. An 8-bit immediate, rotated right by an even amount (0-30)
///
/// ## Register Operand (I bit = 0)
///
/// ```text
/// ADD R0, R1, R2           ; R2 unshifted
/// ADD R0, R1, R2, LSL #3   ; R2 shifted left by 3
/// ADD R0, R1, R2, LSR R3   ; R2 shifted right by value in R3
/// ```
///
/// ## Immediate Operand (I bit = 1)
///
/// ```text
/// Encoded as: 8-bit immediate rotated right by (4-bit field * 2)
///
/// ADD R0, R1, #0xFF        ; base=0xFF, rotate=0
/// ADD R0, R1, #0xFF00      ; base=0xFF, rotate=24 (0xFF ROR 24 = 0xFF00)
/// ```
///
/// This encoding allows representing common constants like 0xFF, 0xFF00,
/// 0xFF0000, etc., but not all 32-bit values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AluSecondOperandInfo {
    /// Register operand with optional shift.
    Register {
        /// How the shift amount is specified.
        shift_op: ShiftOperator,
        /// The type of shift to apply.
        shift_kind: ShiftKind,
        /// The register number (0-15) containing the value to shift.
        register: u32,
    },
    /// Immediate operand with rotation.
    Immediate {
        /// The 8-bit immediate value.
        base: u32,
        /// The rotation amount (already multiplied by 2).
        shift: u32,
    },
}

impl std::fmt::Display for AluSecondOperandInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Register {
                shift_op,
                shift_kind,
                register,
            } => {
                if let ShiftOperator::Immediate(shift) = shift_op
                    && shift == 0
                {
                    return if shift_kind == ShiftKind::Lsl {
                        write!(f, "R{register}")
                    } else if shift_kind == ShiftKind::Ror {
                        write!(f, "R{register}, RRX")
                    } else {
                        write!(f, "R{register}, {shift_kind} #32")
                    };
                }

                write!(f, "R{register}, {shift_kind} {shift_op}")
            }
            Self::Immediate { base, shift } => {
                write!(f, "#{}", base.rotate_right(shift))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_logical_instruction() {
        let alu_op_code = 9;
        let instruction_kind = ArmModeAluInstr::from(alu_op_code).kind();

        assert_eq!(instruction_kind, AIKind::Logical);
    }

    #[test]
    fn test_arithmetic_instruction() {
        let alu_op_code = 2;
        let instruction_kind = ArmModeAluInstr::from(alu_op_code).kind();

        assert_eq!(instruction_kind, AIKind::Arithmetic);
    }
}
