//! # ARM7TDMI Register File
//!
//! The 16 general-purpose registers visible at any time.
//!
//! - **R0-R12**: General purpose
//! - **R13 (SP)**: Stack pointer (by convention)
//! - **R14 (LR)**: Link register (return address)
//! - **R15 (PC)**: Program counter (+8 ARM, +4 Thumb due to pipeline)
//!
//! For register banking by mode, see [`cpu_modes`](super::cpu_modes).
//! For Thumb register restrictions, see [`thumb`](super::thumb).

use serde::{Deserialize, Serialize};

/// Stack Pointer register index.
pub const REG_SP: usize = 0xD;

/// Link Register index (return address for subroutines).
pub const REG_LR: usize = 0xE;

/// Program Counter register index.
pub const REG_PROGRAM_COUNTER: u32 = 0xF;

/// The 16 general-purpose registers visible to the CPU.
///
/// This struct holds the currently-visible register values. Note that some
/// registers are banked - when the CPU mode changes, different physical
/// registers are swapped in. The [`RegisterBank`](super::register_bank::RegisterBank)
/// holds the banked registers.
///
/// R15 (index 15) is the Program Counter and is special:
/// - Reading it returns the current instruction address + 8 (ARM) or + 4 (Thumb)
/// - Writing it causes a pipeline flush and branch to the new address
///
/// See the [module-level documentation](self) for details on register usage.
#[derive(Default, Serialize, Deserialize)]
pub struct Registers([u32; 16]);

impl Registers {
    pub fn program_counter(&self) -> usize {
        self.0[15].try_into().unwrap()
    }

    pub const fn set_program_counter(&mut self, new_value: u32) {
        self.0[15] = new_value;
    }

    pub const fn advance_program_counter(&mut self, bytes: u32) {
        self.0[15] = self.0[15].wrapping_add(bytes);
    }

    pub fn set_register_at(&mut self, reg: usize, new_value: u32) {
        assert!(reg <= 15, "Invalid register index: {reg} (0x{reg:X})");
        self.0[reg] = new_value;
    }

    pub const fn register_at(&self, reg: usize) -> u32 {
        self.0[reg]
    }

    pub fn to_vec(&self) -> Vec<u32> {
        self.0.as_slice().to_vec()
    }
}
