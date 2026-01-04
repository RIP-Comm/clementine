//! # Banked Registers for Exception Modes
//!
//! When the CPU switches modes (e.g., from User to IRQ because a timer fired),
//! specific registers are **physically swapped out** for different ones. This is
//! called "register banking" and is crucial for exception handling.
//!
//! ## Why Banking Exists
//!
//! Imagine `VBlank` fires while a game is in the middle of a function:
//! - The game's return address is in R14 (LR)
//! - The game's stack is at the address in R13 (SP)
//!
//! If the IRQ handler used the same R13/R14, it would destroy the game's state!
//! Banking solves this by giving IRQ mode its **own private R13 and R14**.
//!
//! ## What Gets Banked
//!
//! ```text
//! ┌───────────┬───────────────────────────────────────────────────────────────┐
//! │ Registers │ Banking Behavior                                              │
//! ├───────────┼───────────────────────────────────────────────────────────────┤
//! │ R0 - R7   │ NEVER banked. Same physical registers in ALL modes.           │
//! │           │ Exception handlers must save these if they use them.          │
//! ├───────────┼───────────────────────────────────────────────────────────────┤
//! │ R8 - R12  │ Banked ONLY in FIQ mode. This is why FIQ is "fast" - the      │
//! │           │ handler gets 5 free scratch registers without saving.         │
//! ├───────────┼───────────────────────────────────────────────────────────────┤
//! │ R13 (SP)  │ Banked in EVERY exception mode. Each mode has its own stack.  │
//! │           │ User_SP, IRQ_SP, FIQ_SP, SVC_SP, ABT_SP, UND_SP all exist.    │
//! ├───────────┼───────────────────────────────────────────────────────────────┤
//! │ R14 (LR)  │ Banked in EVERY exception mode. Holds return address.         │
//! │           │ When IRQ fires, User_LR is preserved, IRQ_LR gets return addr.│
//! ├───────────┼───────────────────────────────────────────────────────────────┤
//! │ R15 (PC)  │ NEVER banked. There's only one program counter.               │
//! ├───────────┼───────────────────────────────────────────────────────────────┤
//! │ SPSR      │ One per exception mode. Saves CPSR when exception occurs.     │
//! │           │ User/System modes have NO SPSR (nothing to restore to).       │
//! └───────────┴───────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Physical Register Count
//!
//! The ARM7TDMI has **37 total registers** (not 16!):
//!
//! - 16 visible at any time (R0-R15)
//! - 1 CPSR (always visible)
//! - 5 SPSRs (one per exception mode: FIQ, IRQ, SVC, ABT, UND)
//! - 10 banked for FIQ (`R8_fiq`..`R14_fiq` + `SPSR_fiq` = 7 already counted)
//! - 2 banked for IRQ (`R13_irq`, `R14_irq`)
//! - 2 banked for SVC (`R13_svc`, `R14_svc`)
//! - 2 banked for ABT (`R13_abt`, `R14_abt`)
//! - 2 banked for UND (`R13_und`, `R14_und`)
//!
//! ## Mode Switch Example
//!
//! When switching from User to IRQ mode:
//!
//! 1. Current CPSR → `SPSR_irq` (save flags so we can restore them later)
//! 2. Current R14 stays in User's R14 (preserved)
//! 3. `R14_irq` becomes visible as R14 (holds return address)
//! 4. `R13_irq` becomes visible as R13 (IRQ has its own stack)
//! 5. R0-R12 stay the same (handler must save any it uses)
//!
//! When returning (via `MOVS PC, LR` or similar):
//!
//! 1. `SPSR_irq` → CPSR (restore flags and mode bits)
//! 2. Mode changes back to User
//! 3. User's R13/R14 become visible again (never corrupted!)

use serde::{Deserialize, Serialize};

use crate::cpu::psr::Psr;

/// Storage for banked registers across all CPU modes.
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
