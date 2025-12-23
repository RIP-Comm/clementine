//! # Banked Registers for Exception Modes
//!
//! Storage for registers that are swapped when changing CPU modes.
//! See [`cpu_modes`](super::cpu_modes) for the banking table and mode details.
//!
//! Each exception mode has its own R13 (SP), R14 (LR), and SPSR.
//! FIQ additionally banks R8-R12 for faster interrupt handling.

use serde::{Deserialize, Serialize};

use crate::cpu::psr::Psr;

/// Storage for banked registers across all CPU modes.
///
/// When the CPU switches modes, certain registers are "banked" - the current
/// values are saved here and mode-specific values are loaded into the main
/// register file. This happens automatically on mode switches.
///
/// See the [module-level documentation](self) for details on which registers
/// are banked in each mode.
#[derive(Default, Serialize, Deserialize)]
pub struct RegisterBank {
    // User/System mode R8-R14 saved here when in FIQ mode
    /// R8 value when not in FIQ mode (saved when entering FIQ).
    pub r8_old: u32,
    /// R9 value when not in FIQ mode.
    pub r9_old: u32,
    /// R10 value when not in FIQ mode.
    pub r10_old: u32,
    /// R11 value when not in FIQ mode.
    pub r11_old: u32,
    /// R12 value when not in FIQ mode.
    pub r12_old: u32,
    /// R13 (SP) value when not in FIQ mode.
    pub r13_old: u32,
    /// R14 (LR) value when not in FIQ mode.
    pub r14_old: u32,

    // FIQ mode banked registers
    /// R8 for FIQ mode.
    pub r8_fiq: u32,
    /// R9 for FIQ mode.
    pub r9_fiq: u32,
    /// R10 for FIQ mode.
    pub r10_fiq: u32,
    /// R11 for FIQ mode.
    pub r11_fiq: u32,
    /// R12 for FIQ mode.
    pub r12_fiq: u32,
    /// R13 (SP) for FIQ mode.
    pub r13_fiq: u32,
    /// R14 (LR) for FIQ mode.
    pub r14_fiq: u32,

    // Supervisor mode banked registers
    /// R13 (SP) for Supervisor mode (SWI handler stack).
    pub r13_svc: u32,
    /// R14 (LR) for Supervisor mode (return address from SWI).
    pub r14_svc: u32,

    // Abort mode banked registers
    /// R13 (SP) for Abort mode.
    pub r13_abt: u32,
    /// R14 (LR) for Abort mode.
    pub r14_abt: u32,

    // IRQ mode banked registers
    /// R13 (SP) for IRQ mode (interrupt handler stack).
    pub r13_irq: u32,
    /// R14 (LR) for IRQ mode (return address from interrupt).
    pub r14_irq: u32,

    // Undefined mode banked registers
    /// R13 (SP) for Undefined instruction mode.
    pub r13_und: u32,
    /// R14 (LR) for Undefined instruction mode.
    pub r14_und: u32,

    // Saved Program Status Registers (one per exception mode)
    /// SPSR for FIQ mode (saves CPSR when FIQ occurs).
    pub spsr_fiq: Psr,
    /// SPSR for Supervisor mode (saves CPSR when SWI occurs).
    pub spsr_svc: Psr,
    /// SPSR for Abort mode (saves CPSR when abort occurs).
    pub spsr_abt: Psr,
    /// SPSR for IRQ mode (saves CPSR when IRQ occurs).
    pub spsr_irq: Psr,
    /// SPSR for Undefined mode (saves CPSR when undefined instruction occurs).
    pub spsr_und: Psr,
}
