//! # GBA CPU Emulation
//!
//! This module implements the ARM7TDMI processor, the heart of the Game Boy Advance.
//! Understanding how this CPU works is key to understanding how GBA games execute.
//!
//! ## How a GBA Game Runs
//!
//! When you power on a GBA, here's what happens:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                          GBA Boot Sequence                                  │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │  1. CPU starts at address 0x00000000 (Reset vector in BIOS)                │
//! │  2. BIOS initializes hardware, displays logo, checks cartridge header      │
//! │  3. BIOS jumps to cartridge entry point at 0x08000000                      │
//! │  4. Game code runs, using BIOS functions via SWI when needed               │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```

mod arm;

pub mod arm7tdmi;
pub mod condition;
mod cpu_modes;

mod flags;

pub mod hardware;
pub mod psr;
mod register_bank;
mod registers;
mod thumb;

use arm::instructions::ArmModeInstruction;
use thumb::instruction::Instruction as ThumbModeInstruction;

// ============================================================================
// Disassembler Types
// ============================================================================

/// Default buffer capacity for the disassembler channel.
///
/// Large enough to handle bursts without blocking the CPU.
/// At ~16.78 MHz and 60fps, that's ~280k instructions per frame.
/// We use a large buffer to avoid any backpressure on the CPU.
pub const DISASM_BUFFER_CAPACITY: usize = 1024 * 1024;

/// An entry in the disassembler buffer.
///
/// Contains the program counter and instruction data for either ARM or Thumb mode.
/// The formatting is done lazily by the consumer (UI thread) to keep the CPU
/// hot path as fast as possible.
///
/// ## Design Goals
///
/// The disassembler is always enabled but designed to have
/// minimal impact on emulation performance:
///
/// 1. **No allocations in CPU hot path** - We send `Copy` types through the channel
/// 2. **No formatting in CPU hot path** - String formatting happens in the UI
/// 3. **Lock-free channel** - Uses [`rtrb`] for zero-contention message passing
/// 4. **Graceful overflow** - If the channel fills up, entries are dropped
#[derive(Debug, Clone, Copy)]
pub enum DisasmEntry {
    /// ARM mode instruction (32-bit)
    Arm {
        pc: u32,
        instruction: ArmModeInstruction,
    },
    /// Thumb mode instruction (16-bit)
    Thumb {
        pc: u32,
        instruction: ThumbModeInstruction,
    },
}

impl DisasmEntry {
    /// Format the entry as a disassembly string.
    ///
    /// This is called lazily by the UI thread, not in the CPU hot path.
    #[must_use]
    pub fn format(&self) -> String {
        match self {
            Self::Arm { pc, instruction } => {
                format!("{pc:#04X}: {}", instruction.disassembler())
            }
            Self::Thumb { pc, instruction } => {
                format!("{pc:#04X}: {}", instruction.disassembler())
            }
        }
    }
}
