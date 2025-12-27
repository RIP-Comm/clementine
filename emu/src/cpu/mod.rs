//! # GBA CPU Emulation - ARM7TDMI
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
//!
//! ## ARM vs Thumb: Two Instruction Sets
//!
//! The ARM7TDMI can run in two different states:
//!
//! ### ARM State (32-bit instructions)
//!
//! - Each instruction is 4 bytes, word-aligned (address ends in 0x0, 0x4, 0x8, 0xC)
//! - Full access to all 16 registers
//! - **Conditional execution**: Every instruction has a 4-bit condition code
//! - More powerful but uses more memory
//!
//! ```text
//! ARM Instruction Format (32 bits):
//! ┌──────────┬─────────────────────────────────────────────────────┐
//! │ 31    28 │ 27                                               0 │
//! ├──────────┼─────────────────────────────────────────────────────┤
//! │   Cond   │              Instruction-specific bits             │
//! └──────────┴─────────────────────────────────────────────────────┘
//!     ↑
//!     Condition code (EQ, NE, GT, LT, AL, etc.)
//! ```
//!
//! ### Thumb State (16-bit instructions)
//!
//! - Each instruction is 2 bytes, halfword-aligned
//! - Only 8 registers easily accessible (R0-R7), special access to R8-R15
//! - **No conditional execution** (except for branches)
//! - More compact code - important for GBA's limited memory
//!
//! ```text
//! Thumb Instruction Format (16 bits):
//! ┌─────────────────────────────────────────────────────┐
//! │ 15                                               0 │
//! ├─────────────────────────────────────────────────────┤
//! │              Instruction-specific bits             │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! ### Switching Between States
//!
//! The CPU switches states using the `BX` (Branch and Exchange) instruction:
//! - `BX Rn` - Jump to address in Rn; if bit 0 is 1, switch to Thumb; if 0, switch to ARM
//!
//! The current state is stored in the T bit (bit 5) of the CPSR register.
//!
//! ## The Execution Pipeline
//!
//! The ARM7TDMI has a 3-stage pipeline:
//!
//! ```text
//! ┌─────────┐    ┌─────────┐    ┌─────────┐
//! │  FETCH  │ → │ DECODE  │ → │ EXECUTE │
//! └─────────┘    └─────────┘    └─────────┘
//!      ↑              ↑              ↑
//!    PC+8          PC+4            PC     (ARM state)
//!    PC+4          PC+2            PC     (Thumb state)
//! ```
//!
//! This means when an instruction executes, PC points 2 instructions ahead!
//! This is crucial for understanding branch offsets and PC-relative addressing.
//!
//! ## Registers
//!
//! The CPU has 16 general-purpose registers plus status registers:
//!
//! | Register | Alias | Purpose                                    |
//! |----------|-------|-------------------------------------------|
//! | R0-R12   | -     | General purpose                           |
//! | R13      | SP    | Stack Pointer (by convention)             |
//! | R14      | LR    | Link Register (return address for calls)  |
//! | R15      | PC    | Program Counter (current instruction + 8) |
//! | CPSR     | -     | Current Program Status Register           |
//! | SPSR     | -     | Saved Program Status Register (per mode)  |
//!
//! ## Module Structure
//!
//! - [`arm7tdmi`] - Main CPU struct with fetch/decode/execute cycle
//! - `cpu_modes` - Operating modes (User, IRQ, Supervisor, etc.)
//! - [`psr`] - Program Status Register (flags, mode bits, state bit)
//! - [`condition`] - Condition codes for conditional execution
//! - `registers` - Register file implementation
//! - `register_bank` - Banked registers for different modes
//! - `arm` - ARM instruction set implementation
//! - `thumb` - Thumb instruction set implementation
//! - [`hardware`] - Memory-mapped hardware (LCD, DMA, timers, etc.)

mod arm;

#[allow(clippy::cast_lossless)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::large_stack_frames)]
#[allow(clippy::module_name_repetitions)]
pub mod arm7tdmi;
pub mod condition;
mod cpu_modes;

#[allow(clippy::cast_possible_truncation)]
mod flags;

#[allow(clippy::cast_possible_truncation)]
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
