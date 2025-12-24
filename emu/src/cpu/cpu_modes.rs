//! # ARM7TDMI CPU Operating Modes
//!
//! The ARM7TDMI is the processor used in the GBA. It has **seven operating modes**, each designed
//! for different purposes. Understanding these modes is fundamental to understanding
//! how the GBA handles exceptions, interrupts, and privileged operations.
//!
//! ## Overview of the ARM7TDMI
//!
//! The ARM7TDMI is a 32-bit RISC processor with:
//! - **ARM state**: Executes 32-bit instructions
//! - **Thumb state**: Executes 16-bit instructions
//!
//! The "TDMI" stands for:
//! - **T** = Thumb instruction set support
//! - **D** = Debug extensions
//! - **M** = Enhanced multiplier
//! - **I** = Embedded ICE debug support
//!
//! ## The Seven Operating Modes
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        ARM7TDMI Operating Modes                        │
//! ├─────────────┬──────────┬───────────────────────────────────────────────┤
//! │    Mode     │  Binary  │                  Purpose                      │
//! ├─────────────┼──────────┼───────────────────────────────────────────────┤
//! │ User        │  10000   │ Normal program execution (unprivileged)       │
//! │ FIQ         │  10001   │ Fast interrupt handling                       │
//! │ IRQ         │  10010   │ General interrupt handling                    │
//! │ Supervisor  │  10011   │ Protected mode for OS (software interrupt)    │
//! │ Abort       │  10111   │ Memory access failures                        │
//! │ Undefined   │  11011   │ Undefined instruction handling                │
//! │ System      │  11111   │ Privileged mode sharing User registers        │
//! └─────────────┴──────────┴───────────────────────────────────────────────┘
//! ```
//!
//! ## Mode Categories
//!
//! ### Privileged vs Unprivileged
//!
//! - **User mode** is the only unprivileged mode, it cannot directly change
//!   CPU mode, disable interrupts, or access certain system registers.
//! - All other modes are **privileged** and can perform system-level operations.
//!
//! ### Exception Modes
//!
//! Five modes are entered automatically when exceptions occur:
//!
//! | Exception          | Mode       | Vector Address | Cause                          |
//! |--------------------|------------|----------------|--------------------------------|
//! | Reset              | Supervisor | 0x00000000     | Power on or reset              |
//! | Undefined          | Undefined  | 0x00000004     | Unknown instruction            |
//! | Software Interrupt | Supervisor | 0x00000008     | SWI instruction (BIOS calls)   |
//! | Prefetch Abort     | Abort      | 0x0000000C     | Failed instruction fetch       |
//! | Data Abort         | Abort      | 0x00000010     | Failed data access             |
//! | IRQ                | IRQ        | 0x00000018     | Hardware interrupt             |
//! | FIQ                | FIQ        | 0x0000001C     | Fast hardware interrupt        |
//!
//! ## Banked Registers
//!
//! Each exception mode has its own **banked registers**, private copies that
//! are swapped in when entering that mode. This allows the exception handler
//! to work without corrupting the interrupted program's registers.
//!
//! ```text
//! Register │ User/Sys │  FIQ   │  IRQ   │  SVC   │ Abort  │ Undef  │
//! ─────────┼──────────┼────────┼────────┼────────┼────────┼────────┤
//!   R0-R7  │  R0-R7   │ R0-R7  │ R0-R7  │ R0-R7  │ R0-R7  │ R0-R7  │
//!   R8     │   R8     │ R8_fiq │  R8    │   R8   │   R8   │   R8   │
//!   R9     │   R9     │ R9_fiq │  R9    │   R9   │   R9   │   R9   │
//!   R10    │   R10    │R10_fiq │  R10   │  R10   │  R10   │  R10   │
//!   R11    │   R11    │R11_fiq │  R11   │  R11   │  R11   │  R11   │
//!   R12    │   R12    │R12_fiq │  R12   │  R12   │  R12   │  R12   │
//!   R13/SP │   R13    │R13_fiq │R13_irq │R13_svc │R13_abt │R13_und │
//!   R14/LR │   R14    │R14_fiq │R14_irq │R14_svc │R14_abt │R14_und │
//!   R15/PC │   R15    │  R15   │  R15   │  R15   │  R15   │  R15   │
//!   CPSR   │   CPSR   │  CPSR  │  CPSR  │  CPSR  │  CPSR  │  CPSR  │
//!   SPSR   │   ---    │SPSR_fiq│SPSR_irq│SPSR_svc│SPSR_abt│SPSR_und│
//! ```
//!
//! Note: FIQ has the most banked registers (R8-R14) for fastest interrupt handling.
//!
//! ## How Exceptions Work
//!
//! When an exception occurs, the CPU:
//!
//! 1. **Saves the return address** to the new mode's R14 (LR)
//! 2. **Saves CPSR** to the new mode's SPSR (so flags can be restored later)
//! 3. **Changes to the exception's mode** (mode bits in CPSR)
//! 4. **Disables interrupts** (sets I bit, and F bit for FIQ/Reset)
//! 5. **Switches to ARM state** (clears T bit - exceptions always run ARM code)
//! 6. **Jumps to the exception vector** (fixed address in BIOS area)
//!
//! To return from an exception, the handler restores CPSR from SPSR and jumps
//! back using the saved LR value.
//!
//! ## GBA-Specific Usage
//!
//! On the GBA:
//!
//! - **User mode**: Where games run (after BIOS initialization)
//! - **Supervisor mode**: BIOS functions called via SWI
//! - **IRQ mode**: `VBlank`, `HBlank`, timer, DMA, keypad interrupts
//! - **FIQ mode**: Not typically used (no external FIQ source)
//! - **System mode**: Sometimes used by advanced games for privileged access
//!
//! The BIOS handles most exception setup. Games typically only need to:
//! 1. Set up an IRQ handler at 0x03007FFC (pointer to handler)
//! 2. Enable desired interrupts in IE (0x04000200)
//! 3. Set IME (0x04000208) to enable interrupt processing

use serde::{Deserialize, Serialize};

/// The CPU operating mode, stored in bits 0-4 of the CPSR/SPSR.
///
/// Each mode determines:
/// - Which banked registers are active
/// - Whether the code has privileged access
/// - How the CPU got to this state (for exception modes)
///
/// See the [module-level documentation](self) for details on how modes work.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Mode {
    /// Normal program execution state (unprivileged).
    ///
    /// This is where user code (games) runs. Cannot directly change CPU mode
    /// or access privileged registers.
    User = 0b10000,

    /// Fast Interrupt Request mode.
    ///
    /// Entered when an FIQ exception occurs. Has the most banked registers
    /// (R8-R14) for minimal context-save overhead. On GBA, FIQ is not
    /// typically used since there's no external FIQ source.
    Fiq = 0b10001,

    /// Interrupt Request mode.
    ///
    /// Entered when an IRQ exception occurs (`VBlank`, `HBlank`, timers, etc.).
    /// Most common exception mode used by GBA games.
    Irq = 0b10010,

    /// Supervisor mode (privileged).
    ///
    /// Entered via Reset or SWI (Software Interrupt) instruction.
    /// The BIOS runs in this mode and handles SWI calls for common
    /// functions like division, decompression, and audio mixing.
    Supervisor = 0b10011,

    /// Abort mode.
    ///
    /// Entered after a data abort (failed memory access) or prefetch abort
    /// (failed instruction fetch). On GBA, typically indicates a bug
    /// since there's no virtual memory.
    Abort = 0b10111,

    /// Undefined instruction mode.
    ///
    /// Entered when the CPU encounters an instruction it doesn't recognize.
    /// On GBA, this usually means a bug or corrupted code.
    Undefined = 0b11011,

    /// System mode (privileged, but shares User registers).
    ///
    /// A privileged mode that uses the same registers as User mode
    /// (no banked SP/LR). Useful for OS code that needs to manipulate
    /// user-mode stack/link register directly.
    System = 0b11111,
}

impl From<Mode> for u32 {
    fn from(m: Mode) -> Self {
        m as Self
    }
}

impl TryFrom<u32> for Mode {
    type Error = String;

    fn try_from(n: u32) -> Result<Self, Self::Error> {
        match n {
            0b10000 => Ok(Self::User),
            0b10001 => Ok(Self::Fiq),
            0b10010 => Ok(Self::Irq),
            0b10011 => Ok(Self::Supervisor),
            0b10111 => Ok(Self::Abort),
            0b11011 => Ok(Self::Undefined),
            0b11111 => Ok(Self::System),
            _ => Err(String::from("Unexpected value for Mode")),
        }
    }
}
