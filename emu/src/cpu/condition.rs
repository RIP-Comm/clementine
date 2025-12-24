//! # ARM Conditional Execution
//!
//! One of ARM's most distinctive features is **conditional execution**: almost every
//! instruction can be conditionally executed based on the CPU flags. This is encoded
//! in the top 4 bits (31-28) of every ARM instruction.
//!
//! ## Why Conditional Execution?
//!
//! Conditional execution reduces branching, which is expensive on pipelined CPUs:
//!
//! ```text
//! Traditional approach (with branches):     ARM approach (conditional):
//! ─────────────────────────────────────     ─────────────────────────────
//!     CMP R0, #0                                CMP R0, #0
//!     BEQ skip                                  MOVNE R1, #1    ← Only executes if Z=0
//!     MOV R1, #1                                MOVEQ R1, #0    ← Only executes if Z=1
//!     B done
//! skip:
//!     MOV R1, #0
//! done:
//! ```
//!
//! The ARM approach:
//! - Avoids pipeline flushes from branches
//! - Uses less code space
//! - Executes faster for simple conditionals
//!
//! ## The CPU Flags (CPSR bits 28-31)
//!
//! Conditions are based on four flags in the CPSR:
//!
//! | Flag | Bit | Name     | Set When                                    |
//! |------|-----|----------|---------------------------------------------|
//! | N    | 31  | Negative | Result has bit 31 set (is negative)         |
//! | Z    | 30  | Zero     | Result is zero                              |
//! | C    | 29  | Carry    | Addition overflowed, or subtraction didn't  |
//! | V    | 28  | Overflow | Signed arithmetic overflowed                |
//!
//! ## Condition Codes
//!
//! The 4-bit condition field encodes 16 conditions (though one is reserved):
//!
//! ```text
//! ┌───────┬────────┬─────────────────────┬─────────────────────────────────┐
//! │ Code  │ Suffix │     Meaning         │          Flags Tested           │
//! ├───────┼────────┼─────────────────────┼─────────────────────────────────┤
//! │ 0000  │   EQ   │ Equal               │ Z=1                             │
//! │ 0001  │   NE   │ Not equal           │ Z=0                             │
//! │ 0010  │   CS   │ Carry set / ≥ (uns) │ C=1                             │
//! │ 0011  │   CC   │ Carry clear / < (u) │ C=0                             │
//! │ 0100  │   MI   │ Minus / negative    │ N=1                             │
//! │ 0101  │   PL   │ Plus / non-negative │ N=0                             │
//! │ 0110  │   VS   │ Overflow set        │ V=1                             │
//! │ 0111  │   VC   │ Overflow clear      │ V=0                             │
//! │ 1000  │   HI   │ Higher (unsigned)   │ C=1 AND Z=0                     │
//! │ 1001  │   LS   │ Lower/same (unsig)  │ C=0 OR Z=1                      │
//! │ 1010  │   GE   │ ≥ (signed)          │ N=V                             │
//! │ 1011  │   LT   │ < (signed)          │ N≠V                             │
//! │ 1100  │   GT   │ > (signed)          │ Z=0 AND N=V                     │
//! │ 1101  │   LE   │ ≤ (signed)          │ Z=1 OR N≠V                      │
//! │ 1110  │   AL   │ Always              │ (unconditional)                 │
//! │ 1111  │   NV   │ Never (reserved)    │ (don't use)                     │
//! └───────┴────────┴─────────────────────┴─────────────────────────────────┘
//! ```
//!
//! ## Instruction Encoding Example
//!
//! ```text
//! Instruction: MOVEQ R0, #1    (Move 1 to R0 if equal)
//!
//! Binary: 0000 00 1 1101 0 0000 0000 000000000001
//!         ↑         ↑         ↑
//!         │         │         └─ Immediate value: 1
//!         │         └─ MOV opcode
//!         └─ Condition: 0000 = EQ (execute if Z=1)
//! ```
//!
//! ## Thumb Mode Difference
//!
//! In Thumb state, only branch instructions have condition codes. Other
//! instructions always execute (equivalent to AL condition). This is why
//! Thumb code often uses more branches than ARM code.
//!
//! ## Common Patterns
//!
//! ```text
//! ; Check if R0 == R1
//! CMP R0, R1          ; Sets flags: Z=1 if equal
//! BEQ equal_case      ; Branch if Z=1
//!
//! ; Set R0 = abs(R0) using conditional
//! CMP R0, #0          ; Compare R0 to 0
//! RSBLT R0, R0, #0    ; If less than 0: R0 = 0 - R0
//!
//! ; Max of R0 and R1, result in R0
//! CMP R0, R1
//! MOVLT R0, R1        ; If R0 < R1: R0 = R1
//! ```

use serde::{Deserialize, Serialize};

/// Condition codes for ARM conditional execution.
///
/// In ARM state, all instructions are conditionally executed according to
/// the state of the CPSR condition codes and the instruction's condition field.
/// If the flags satisfy the condition, the instruction executes; otherwise
/// it is skipped (acting as a NOP).
///
/// See the [module-level documentation](self) for details on how conditions work.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// Equal (Z=1)
    ///
    /// True when the previous comparison found the values equal,
    /// or when the previous operation resulted in zero.
    EQ = 0x0,

    /// Not equal (Z=0)
    ///
    /// True when the previous comparison found the values different,
    /// or when the previous operation resulted in non-zero.
    NE = 0x1,

    /// Carry set / unsigned higher or same (C=1)
    ///
    /// For comparisons: true when the first operand is >= second (unsigned).
    /// Also known as HS (Higher or Same).
    CS = 0x2,

    /// Carry clear / unsigned lower (C=0)
    ///
    /// For comparisons: true when the first operand is < second (unsigned).
    /// Also known as LO (Lower).
    CC = 0x3,

    /// Minus / negative (N=1)
    ///
    /// True when the result is negative (bit 31 is set).
    MI = 0x4,

    /// Plus / positive or zero (N=0)
    ///
    /// True when the result is positive or zero (bit 31 is clear).
    PL = 0x5,

    /// Overflow set (V=1)
    ///
    /// True when signed arithmetic caused overflow.
    VS = 0x6,

    /// Overflow clear (V=0)
    ///
    /// True when signed arithmetic did not overflow.
    VC = 0x7,

    /// Unsigned higher (C=1 AND Z=0)
    ///
    /// For comparisons: true when the first operand is > second (unsigned).
    HI = 0x8,

    /// Unsigned lower or same (C=0 OR Z=1)
    ///
    /// For comparisons: true when the first operand is <= second (unsigned).
    LS = 0x9,

    /// Signed greater or equal (N=V)
    ///
    /// For comparisons: true when the first operand is >= second (signed).
    GE = 0xA,

    /// Signed less than (N≠V)
    ///
    /// For comparisons: true when the first operand is < second (signed).
    LT = 0xB,

    /// Signed greater than (Z=0 AND N=V)
    ///
    /// For comparisons: true when the first operand is > second (signed).
    GT = 0xC,

    /// Signed less than or equal (Z=1 OR N≠V)
    ///
    /// For comparisons: true when the first operand is <= second (signed).
    LE = 0xD,

    /// Always (unconditional)
    ///
    /// The instruction always executes. This is the default when no
    /// condition suffix is specified in assembly (e.g., `MOV` = `MOVAL`).
    AL = 0xE,

    /// Never (reserved, do not use)
    ///
    /// In ARMv1/v2 this meant "never execute". In `ARMv3+` it's reserved
    /// and should not be used by normal code.
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
