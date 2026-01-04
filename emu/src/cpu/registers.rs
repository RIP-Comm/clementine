//! # ARM7TDMI Register File
//!
//! The ARM7TDMI has 16 general-purpose registers visible at any time, but some of
//! these are actually **different physical registers** depending on the CPU mode.
//! This is called "register banking", see [`RegisterBank`](super::register_bank::RegisterBank).
//!
//! ## The 16 Visible Registers
//!
//! ```text
//! ┌──────────┬─────────────────────────────────────────────────────────────────┐
//! │ Register │ Purpose                                                         │
//! ├──────────┼─────────────────────────────────────────────────────────────────┤
//! │ R0-R7    │ General purpose. NEVER banked. Same physical register always.   │
//! │ R8-R12   │ General purpose. Banked ONLY in FIQ mode (for fast interrupts). │
//! │ R13 (SP) │ Stack Pointer (by convention). Banked in ALL exception modes.   │
//! │ R14 (LR) │ Link Register (return address). Banked in ALL exception modes.  │
//! │ R15 (PC) │ Program Counter. Never banked. Special read/write behavior.     │
//! └──────────┴─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Why Banking Matters
//!
//! When an interrupt fires (e.g., `VBlank`), the CPU switches to IRQ mode. If R13/R14
//! weren't banked, the interrupt handler would corrupt the game's stack pointer and
//! return address. Banking gives each mode its own SP/LR, so interrupts are transparent.
//!
//! FIQ banks R8-R12 as well, allowing the FIQ handler to use these registers freely
//! without saving/restoring them - hence "Fast" Interrupt Request.
//!
//! ## Program Counter (R15) Quirks
//!
//! Due to the 3-stage pipeline, reading R15 returns a value **ahead** of the current
//! instruction:
//! - **ARM mode**: PC reads as current instruction address + 8
//! - **Thumb mode**: PC reads as current instruction address + 4
//!
//! Writing to R15 causes a pipeline flush and branches to the new address.

use serde::{Deserialize, Serialize};

/// Stack Pointer register index.
pub const REG_SP: usize = 0xD;

/// Link Register index (return address for subroutines).
pub const REG_LR: usize = 0xE;

/// Program Counter register index.
pub const REG_PC: u32 = 0xF;

#[derive(Default, Serialize, Deserialize)]
pub struct Registers([u32; 16]);

impl Registers {
    pub fn program_counter(&self) -> usize {
        self.0[15].try_into().unwrap()
    }

    pub const fn set_program_counter(&mut self, new_value: u32) {
        self.0[15] = new_value;
    }

    pub fn set_register_at(&mut self, reg: usize, new_value: u32) {
        assert!(reg <= 15, "Invalid register index: {reg} (0x{reg:X})");
        self.0[reg] = new_value;
    }

    pub const fn register_at(&self, reg: usize) -> u32 {
        self.0[reg]
    }
}
