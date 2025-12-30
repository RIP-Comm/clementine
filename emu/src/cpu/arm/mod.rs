//! # ARM Instruction Set (32-bit)
//!
//! Full-featured instruction set with conditional execution on every instruction.
//!
//! ## Format
//!
//! ```text
//! 31-28   27-25   24-0
//! [Cond] [Format] [Instruction-specific]
//! ```
//!
//! - **Condition (bits 28-31)**: See [`condition`](super::condition)
//! - **Format (bits 25-27)**: Determines instruction category
//!
//! ## Instruction Categories
//!
//! | Bits 27-25 | Category              | Examples                    |
//! |------------|-----------------------|-----------------------------|
//! | 00x        | Data Processing       | AND, ADD, CMP, MOV          |
//! | 000        | Multiply/Swap/BX      | MUL, SWP, BX                |
//! | 01x        | Single Data Transfer  | LDR, STR                    |
//! | 100        | Block Data Transfer   | LDM, STM                    |
//! | 101        | Branch                | B, BL                       |
//! | 1111       | Software Interrupt    | SWI                         |
//!
//! ## Barrel Shifter
//!
//! Operand2 can be shifted at no extra cost: LSL, LSR, ASR, ROR, RRX.
//!
//! ## Submodules
//!
//! - [`instructions`] - Decoding (`From<u32>`)
//! - [`operations`] - Execution
//! - [`alu_instruction`] - ALU ops and barrel shifter
//! - [`mode`] - Addressing modes

pub mod alu_instruction;

pub mod instructions;

pub mod mode;

pub mod operations;
