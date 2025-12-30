//! # Thumb Instruction Set (16-bit)
//!
//! Compact instruction set for better code density on GBA's 16-bit ROM bus.
//!
//! Key differences from ARM (see [`arm`](super::arm)):
//! - 16-bit instructions (vs 32-bit)
//! - Only R0-R7 directly accessible (R8-R15 via special instructions)
//! - No conditional execution (except branches)
//! - Separate shift instructions (no barrel shifter in ALU ops)
//!
//! ## Register Access
//!
//! - **R0-R7**: Full access
//! - **R8-R15**: Only via `HiRegisterOpBX` (MOV, ADD, CMP, BX)
//! - **SP/LR**: Implicit in PUSH/POP, SP-relative loads
//!
//! ## Long Branch (BL)
//!
//! BL uses two instructions for ±4MB range:
//! 1. `1111 0 <offset_hi>` - Sets up LR
//! 2. `1111 1 <offset_lo>` - Completes branch, sets LR to return address
//!
//! ## Submodules
//!
//! - [`instruction`] - Decoding (`TryFrom<u16>`)
//! - [`operations`] - Execution
//! - [`alu_instructions`] - ALU ops
//! - [`mode`] - Addressing modes

pub mod alu_instructions;

pub mod instruction;
pub mod mode;

pub mod operations;
