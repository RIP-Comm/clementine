//! # ARM7TDMI Processor Implementation
//!
//! This module implements the ARM7TDMI processor core used in the Game Boy Advance.
//! The ARM7TDMI is a 32-bit RISC processor supporting two instruction sets.
//! The processor is running at 16.78 MHz.
//! It can run in either ARM mode or Thumb mode.
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                           ARM7TDMI Pipeline                               │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                  │
//! │   │   FETCH     │ ──▶ │   DECODE    │ ──▶ │   EXECUTE   │                  │
//! │   │  (PC + 8)   │     │  (PC + 4)   │     │    (PC)     │                  │
//! │   └─────────────┘     └─────────────┘     └─────────────┘                  │
//! │          │                                       │                          │
//! │          ▼                                       ▼                          │
//! │   ┌─────────────┐                        ┌─────────────┐                   │
//! │   │    Bus      │◀──────────────────────▶│  Registers  │                   │
//! │   │  (Memory)   │                        │   (R0-R15)  │                   │
//! │   └─────────────┘                        └─────────────┘                   │
//! │          │                                       │                          │
//! │          ▼                                       ▼                          │
//! │   ┌─────────────┐                        ┌─────────────┐                   │
//! │   │  Hardware   │                        │ CPSR/SPSR   │                   │
//! │   │ (LCD, DMA)  │                        │  (Flags)    │                   │
//! │   └─────────────┘                        └─────────────┘                   │
//! │                                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## The 3-Stage Pipeline
//!
//! The ARM7TDMI uses a 3-stage pipeline to increase throughput:
//!
//! | Stage   | What Happens                    | PC Offset (ARM) | PC Offset (Thumb) |
//! |---------|--------------------------------|-----------------|-------------------|
//! | Fetch   | Read instruction from memory   | PC + 8          | PC + 4            |
//! | Decode  | Parse instruction fields       | PC + 4          | PC + 2            |
//! | Execute | Perform the operation          | PC (current)    | PC (current)      |
//!
//! **Important**: When an instruction reads PC, it gets the address of the current
//! instruction **plus 8** (ARM) or **plus 4** (Thumb). This is because PC points
//! to what's being fetched, not what's being executed.
//!
//! ### Pipeline Flushing
//!
//! When a branch occurs, the pipeline must be flushed because the prefetched
//! instructions are from the wrong address. This costs cycles:
//!
//! ```text
//! Before branch:     After branch (BX 0x1000):
//! ┌────────────┐     ┌────────────┐
//! │ FETCH 0x108│     │ FETCH 0x1000│  ← New instruction
//! ├────────────┤     ├────────────┤
//! │ DECODE 0x104│ ──▶ │  (empty)   │  ← Flushed
//! ├────────────┤     ├────────────┤
//! │EXECUTE 0x100│     │  (empty)   │  ← Flushed
//! └────────────┘     └────────────┘
//! ```
//!
//! ## Exception Handling
//!
//! The CPU handles various exceptions (interrupts, errors) by:
//! 1. Saving CPSR to SPSR of the exception mode
//! 2. Setting LR to the return address
//! 3. Switching to the exception's CPU mode
//! 4. Jumping to the exception vector address
//!
//! | Exception   | Vector   | Mode       | Priority |
//! |-------------|----------|------------|----------|
//! | Reset       | 0x00     | Supervisor | 1 (high) |
//! | Undefined   | 0x04     | Undefined  | 6        |
//! | SWI         | 0x08     | Supervisor | 6        |
//! | Prefetch    | 0x0C     | Abort      | 5        |
//! | Data Abort  | 0x10     | Abort      | 2        |
//! | IRQ         | 0x18     | IRQ        | 4        |
//! | FIQ         | 0x1C     | FIQ        | 3        |
//!
//! ## HLE (High-Level Emulation)
//!
//! Some BIOS SWI calls are implemented directly in the emulator for speed:
//! - `SWI 0x06`: Div (signed division)
//! - `SWI 0x07`: `DivArm` (division with swapped args)
//! - `SWI 0x08`: Sqrt (square root)
//! - `SWI 0x0B`: `CpuSet` (memory copy/fill)
//! - `SWI 0x0C`: `CpuFastSet` (fast memory copy/fill)
//!
//! Other SWIs fall through to the actual BIOS code.

use serde::{Deserialize, Serialize};
use std::convert::TryInto;

use super::DisasmEntry;
use crate::bitwise::Bits;
use crate::bus::Bus;
use crate::cpu::arm;
use crate::cpu::arm::instructions::ArmModeInstruction;
use crate::cpu::arm::mode::ArmModeOpcode;
use crate::cpu::cpu_modes::Mode;
use crate::cpu::psr::{CpuState, Psr};
use crate::cpu::register_bank::RegisterBank;
use crate::cpu::thumb::instruction::Instruction;
use crate::cpu::thumb::mode::ThumbModeOpcode;

use super::registers::{REG_SP, Registers};
use super::thumb;

/// The ARM7TDMI CPU core.
///
/// This struct represents the complete state of the ARM7TDMI processor:
/// - 16 general-purpose registers (via `Registers`)
/// - Banked registers for exception modes (via `RegisterBank`)
/// - Current and Saved Program Status Registers ([`Psr`])
/// - Memory bus connection ([`Bus`])
/// - Pipeline state (fetched/decoded instructions)
///
/// ## Creating a CPU
///
/// The CPU is typically created via [`Gba::new`](crate::gba::Gba::new) which
/// sets up the bus and all hardware components. See that documentation
/// for a complete usage example.
///
/// ## The Execution Cycle
///
/// Each call to [`step()`](Self::step) performs one pipeline cycle:
///
/// 1. **Execute**: Run the decoded instruction (if any)
/// 2. **Decode**: Parse the fetched instruction
/// 3. **Fetch**: Read next instruction from memory at PC
/// 4. **Advance**: Increment PC (unless a branch occurred)
///
/// The CPU automatically handles mode switching, interrupts, and pipeline flushing.
#[derive(Serialize, Deserialize)]
pub struct Arm7tdmi {
    pub bus: Bus,

    pub cpsr: Psr,
    pub spsr: Psr,
    pub registers: Registers,

    pub register_bank: RegisterBank,

    /// Producer for the lock-free disassembler channel.
    #[serde(skip)]
    pub disasm_tx: Option<rtrb::Producer<DisasmEntry>>,

    fetched_arm: Option<u32>,
    decoded_arm: Option<ArmModeOpcode>,
    fetched_thumb: Option<u16>,
    decoded_thumb: Option<ThumbModeOpcode>,

    pub current_cycle: u128,
}

#[derive(Copy, Clone, Debug)]
enum ExceptionType {
    UndefinedInstruction,
    SoftwareInterrupt,
    Irq,
}

impl ExceptionType {
    pub const fn address(self) -> usize {
        match self {
            Self::UndefinedInstruction => 0x4,
            Self::SoftwareInterrupt => 0x8,
            Self::Irq => 0x18,
        }
    }

    pub const fn mode(self) -> Mode {
        match self {
            Self::SoftwareInterrupt => Mode::Supervisor,
            Self::UndefinedInstruction => Mode::Undefined,
            Self::Irq => Mode::Irq,
        }
    }

    pub fn next_instruction_func(
        self,
        current_state: CpuState,
        current_pc: usize,
    ) -> Box<dyn Fn() -> usize> {
        let current_executing_ins = match current_state {
            CpuState::Arm => current_pc - 8,
            CpuState::Thumb => current_pc - 4,
        };

        match (current_state, self) {
            (CpuState::Thumb, Self::SoftwareInterrupt | Self::UndefinedInstruction) => {
                Box::new(move || current_executing_ins + 2)
            }
            (CpuState::Arm, Self::SoftwareInterrupt | Self::UndefinedInstruction | Self::Irq) => {
                Box::new(move || current_executing_ins + 4)
            }
            (CpuState::Thumb, Self::Irq) => Box::new(move || current_executing_ins + 4),
        }
    }
}

impl Default for Arm7tdmi {
    fn default() -> Self {
        let mut s = Self {
            bus: Bus::default(),
            cpsr: Psr::from(Mode::Supervisor),
            spsr: Psr::from(Mode::Supervisor), // initialize SPSR to valid mode
            registers: Registers::default(),
            register_bank: RegisterBank::default(),
            disasm_tx: None,
            fetched_arm: None,
            decoded_arm: None,
            fetched_thumb: None,
            decoded_thumb: None,
            current_cycle: u128::default(),
        };

        // Setting ARM mode at startup
        s.cpsr.set_cpu_state(CpuState::Arm);
        s.cpsr.set_irq_disable(true);
        s.cpsr.set_fiq_disable(true);

        s
    }
}

impl Arm7tdmi {
    pub const fn flush_pipeline(&mut self) {
        self.decoded_arm = None;
        self.decoded_thumb = None;
        self.fetched_arm = None;
        self.fetched_thumb = None;
    }

    #[must_use]
    pub fn fetch_arm(&mut self) -> u32 {
        // Get PC and align it for ARM (word-aligned = clear bits 0-1)
        let mut pc = self.registers.program_counter() as u32;
        pc.set_bit_off(0);
        pc.set_bit_off(1);

        // Update current PC for BIOS read protection
        self.bus.set_current_pc(pc as usize);

        let opcode = self.bus.read_word(pc as usize);

        // If fetching from BIOS, save this opcode for read protection
        if (pc as usize) < 0x4000 {
            self.bus.set_last_bios_opcode(opcode);
        }

        opcode
    }

    #[must_use]
    pub fn fetch_thumb(&mut self) -> u16 {
        // Get PC and align it for Thumb (halfword-aligned = clear bit 0)
        let pc_raw = self.registers.program_counter() as u32;
        let mut pc = pc_raw;
        pc.set_bit_off(0);

        // Update current PC for BIOS read protection
        self.bus.set_current_pc(pc as usize);

        let opcode = self.bus.read_half_word(pc as usize);

        // If fetching from BIOS, save this opcode for read protection (extended to 32-bit)
        if (pc as usize) < 0x4000 {
            self.bus.set_last_bios_opcode(u32::from(opcode));
        }

        opcode
    }

    /// This function is used to execute the Data Processing instruction.
    ///
    /// # Panics
    /// It can panics if `op_code` is not a `DataProcessing` instruction.
    pub fn decode<T, V>(op_code: V) -> T
    where
        T: std::fmt::Display + TryFrom<V>,
        <T as TryFrom<V>>::Error: std::fmt::Debug,
    {
        T::try_from(op_code).unwrap()
    }

    /// This function is used to execute the Data Processing instruction.
    ///
    /// # Panics
    /// It can panics if destination register is None.
    pub fn execute_arm(&mut self, op_code: ArmModeOpcode) {
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        let can_execute = self.cpsr.can_execute(op_code.condition);

        if !can_execute {
            return;
        }

        // push dis-ASM to the channel
        if let Some(tx) = &mut self.disasm_tx {
            let pc = self.registers.program_counter() as u32;
            let _ = tx.push(DisasmEntry::Arm {
                pc,
                instruction: op_code.instruction,
            });
        }

        match op_code.instruction {
            ArmModeInstruction::DataProcessing {
                condition: _,
                alu_instruction,
                set_conditions,
                op_kind,
                rn,
                destination,
                op2: _,
            } => self.data_processing(
                op_code,
                alu_instruction,
                set_conditions,
                op_kind,
                rn,
                destination,
            ),
            ArmModeInstruction::PSRTransfer { psr_kind, kind, .. } => {
                self.psr_transfer(kind, psr_kind);
            }
            ArmModeInstruction::Multiply {
                variant,
                should_set_codes,
                rd_destination_register,
                rn_accumulate_register,
                rm_operand_register,
                rs_operand_register,
                ..
            } => self.multiply(
                variant,
                should_set_codes,
                rd_destination_register,
                rn_accumulate_register,
                rm_operand_register,
                rs_operand_register,
            ),
            ArmModeInstruction::MultiplyLong {
                variant,
                should_set_codes,
                rdhi_destination_register,
                rdlo_destination_register,
                rm_operand_register,
                rs_operand_register,
                ..
            } => self.multiply_long(
                variant,
                should_set_codes,
                rdhi_destination_register,
                rdlo_destination_register,
                rm_operand_register,
                rs_operand_register,
            ),
            ArmModeInstruction::SingleDataSwap {
                condition: _,
                byte,
                rn,
                rd,
                rm,
            } => {
                let address: usize = self.registers.register_at(rn as usize).try_into().unwrap();
                let rm_value = self.registers.register_at(rm as usize);

                if byte {
                    // byte swap (SWPB)
                    let old_value = self.bus.read_byte(address) as u32;
                    self.bus.write_byte(address, rm_value as u8);
                    self.registers.set_register_at(rd as usize, old_value);
                } else {
                    // word swap (SWP)
                    let old_value = self.read_word(address);
                    self.bus.write_word(address, rm_value);
                    self.registers.set_register_at(rd as usize, old_value);
                }
            }
            ArmModeInstruction::BranchAndExchange {
                condition: _,
                register,
            } => self.branch_and_exchange(register),
            ArmModeInstruction::HalfwordDataTransfer {
                condition: _,
                indexing,
                offsetting,
                write_back,
                load_store_kind,
                offset_kind,
                base_register,
                source_destination_register,
                transfer_kind,
            } => self.half_word_data_transfer(
                indexing,
                offsetting,
                write_back,
                load_store_kind,
                offset_kind,
                base_register,
                source_destination_register,
                transfer_kind,
            ),
            ArmModeInstruction::SingleDataTransfer {
                condition: _,
                kind,
                quantity,
                write_back,
                indexing,
                rd,
                base_register,
                offset_info,
                offsetting,
            } => self.single_data_transfer(
                kind,
                quantity,
                write_back,
                indexing,
                rd,
                base_register,
                offset_info,
                offsetting,
            ),
            ArmModeInstruction::Undefined => {
                // Undefined instruction exception
                let pc = self.registers.program_counter();
                let pcode = op_code.raw;
                tracing::warn!(
                    "!!! UNDEFINED INSTRUCTION EXCEPTION !!!\n  \
                     PC=0x{pc:08X}, opcode=0x{:08X}, mode=ARM\n  \
                     Binary: {:032b}\n  \
                     R0=0x{:08X} R1=0x{:08X} R2=0x{:08X} R3=0x{:08X}\n  \
                     R4=0x{:08X} R5=0x{:08X} R6=0x{:08X} R7=0x{:08X}",
                    pcode,
                    pcode,
                    self.registers.register_at(0),
                    self.registers.register_at(1),
                    self.registers.register_at(2),
                    self.registers.register_at(3),
                    self.registers.register_at(4),
                    self.registers.register_at(5),
                    self.registers.register_at(6),
                    self.registers.register_at(7),
                );
                self.handle_exception(ExceptionType::UndefinedInstruction);
            }
            ArmModeInstruction::BlockDataTransfer {
                condition: _,
                indexing,
                offsetting,
                load_psr,
                write_back,
                load_store,
                rn,
                register_list: reg_list,
            } => self.block_data_transfer(
                indexing, offsetting, load_psr, write_back, load_store, rn, reg_list,
            ),
            ArmModeInstruction::Branch {
                condition: _,
                link,
                offset,
            } => self.branch(link, offset),
            ArmModeInstruction::CoprocessorDataTransfer { .. } => {
                tracing::warn!(
                    "!!! UNIMPLEMENTED: CoprocessorDataTransfer at PC=0x{:08X}",
                    self.registers.program_counter() - 8
                );
                // Coprocessor instructions are typically ignored on GBA (no coprocessor)
            }
            ArmModeInstruction::CoprocessorDataOperation => {
                tracing::warn!(
                    "!!! UNIMPLEMENTED: CoprocessorDataOperation at PC=0x{:08X}",
                    self.registers.program_counter() - 8
                );
                // Coprocessor instructions are typically ignored on GBA (no coprocessor)
            }
            ArmModeInstruction::CoprocessorRegisterTransfer => {
                tracing::warn!(
                    "!!! UNIMPLEMENTED: CoprocessorRegisterTransfer at PC=0x{:08X}",
                    self.registers.program_counter() - 8
                );
                // Coprocessor instructions are typically ignored on GBA (no coprocessor)
            }
            ArmModeInstruction::SoftwareInterrupt => {
                self.handle_exception(ExceptionType::SoftwareInterrupt);
            }
        }
    }

    /// This function is used to execute the Data Processing instruction.
    ///
    /// # Panics
    /// It can panics if destination register is None.
    pub fn execute_thumb(&mut self, op_code: ThumbModeOpcode) {
        // push dis-ASM to the channel
        if let Some(tx) = &mut self.disasm_tx {
            let pc = self.registers.program_counter() as u32;
            let _ = tx.push(DisasmEntry::Thumb {
                pc,
                instruction: op_code.instruction,
            });
        }

        match op_code.instruction {
            Instruction::MoveShiftedRegister {
                shift_operation: op,
                offset5,
                source_register,
                destination_register,
            } => self.move_shifted_reg(op, offset5, source_register, destination_register),
            Instruction::AddSubtract {
                operation_kind,
                op,
                rn_offset3,
                source_register: rs,
                destination_register: rd,
            } => self.add_subtract(operation_kind, op, rn_offset3, rs, rd),
            Instruction::MoveCompareAddSubtractImm {
                operation: op,
                destination_register: r_destination,
                offset,
            } => self.move_compare_add_sub_imm(op, r_destination, offset),
            Instruction::AluOp {
                alu_operation: op,
                source_register: rs,
                destination_register: rd,
            } => self.alu_op(op, rs, rd),
            Instruction::HiRegisterOpBX {
                register_operation: op,
                source_register,
                destination_register,
            } => self.hi_reg_operation_branch_ex(op, source_register, destination_register),
            Instruction::PCRelativeLoad {
                destination_register: r_destination,
                immediate_value,
            } => self.pc_relative_load(r_destination, immediate_value),
            Instruction::LoadStoreRegisterOffset {
                load_store,
                byte_word,
                ro,
                base_register: rb,
                destination_register: rd,
            } => self.load_store_register_offset(load_store, byte_word, ro, rb, rd),
            Instruction::LoadStoreSignExtByteHalfword {
                h: h_flag,
                sign_extend_flag,
                offset_register: r_offset,
                base_register: r_base,
                destination_register: r_destination,
            } => self.load_store_sign_extend_byte_halfword(
                h_flag,
                sign_extend_flag,
                r_offset,
                r_base,
                r_destination,
            ),
            Instruction::LoadStoreImmOffset {
                load_store,
                byte_word,
                offset,
                base_register,
                destination_register,
            } => self.load_store_immediate_offset(
                load_store,
                byte_word,
                offset,
                base_register,
                destination_register,
            ),
            Instruction::LoadStoreHalfword {
                load_store,
                offset,
                base_register,
                source_destination_register,
            } => self.load_store_halfword(
                load_store,
                offset,
                base_register,
                source_destination_register,
            ),
            Instruction::SPRelativeLoadStore {
                load_store,
                destination_register: r_destination,
                word8,
            } => self.sp_relative_load_store(load_store, r_destination, word8),
            Instruction::LoadAddress {
                sp,
                destination_register: r_destination,
                offset,
            } => self.load_address(sp, r_destination.try_into().unwrap(), offset),
            Instruction::AddOffsetSP { s, word7 } => self.add_offset_sp(s, word7),
            Instruction::PushPopReg {
                load_store,
                pc_lr,
                register_list,
            } => self.push_pop_register(load_store, pc_lr, register_list),
            Instruction::MultipleLoadStore {
                load_store,
                base_register,
                register_list,
            } => self.multiple_load_store(load_store, base_register as usize, register_list),
            Instruction::CondBranch {
                condition,
                immediate_offset,
            } => self.cond_branch(condition, immediate_offset),
            Instruction::Swi => {
                self.handle_exception(ExceptionType::SoftwareInterrupt);
            }
            Instruction::UncondBranch { offset } => self.uncond_branch(offset),
            Instruction::LongBranchLink { h, offset } => self.long_branch_link(h, offset),
            Instruction::Nop => {
                // NOP - do nothing
            }
        }
    }

    fn handle_exception(&mut self, exception_type: ExceptionType) {
        let next_ins = exception_type
            .next_instruction_func(self.cpsr.cpu_state(), self.registers.program_counter())(
        );

        let old_cpsr = self.cpsr;

        // IRQ handling: Always use BIOS exception vector (no HLE for now)
        // This is more accurate to real hardware and avoids HLE bugs
        if matches!(exception_type, ExceptionType::Irq) {
            let _handler_addr = self.bus.read_word(0x0300_7FFC);
            // Fall through to normal exception handling
        }

        // HLE: For SWI, implement common BIOS functions directly
        if matches!(exception_type, ExceptionType::SoftwareInterrupt) {
            // Get the SWI number from the instruction
            // For Thumb: bits 0-7 of the instruction
            // For ARM: bits 0-23 of the instruction (but GBA BIOS only uses 0-7)
            let swi_num = if matches!(old_cpsr.cpu_state(), CpuState::Thumb) {
                // Thumb SWI: read the byte before the return address
                let swi_pc = (next_ins as u32).wrapping_sub(2);
                self.bus.read_byte(swi_pc as usize) as u32
            } else {
                // ARM SWI: read the word at PC-8 and extract bits 0-23
                let swi_pc = (next_ins as u32).wrapping_sub(4);
                let swi_instr = self.bus.read_word(swi_pc as usize);
                swi_instr & 0xFF // GBA BIOS only uses lower 8 bits
            };

            // Try to handle the SWI with HLE
            if self.handle_swi_hle(swi_num, old_cpsr, next_ins as u32) {
                return; // HLE handled it
            }
            // Otherwise fall through to normal BIOS handling
        }

        // Every exception is handled in ARM
        self.cpsr.set_cpu_state(CpuState::Arm);

        self.swap_mode(exception_type.mode());

        // Set LR to return address
        // Note: LR should NOT have bit 0 set for exceptions - the return mode is stored in SPSR
        // The Thumb bit convention (bit 0 set for Thumb addresses) is only used with BX instruction
        self.registers.set_register_at(14, next_ins as u32);
        self.spsr = old_cpsr;

        self.cpsr.set_irq_disable(true);

        let new_pc = exception_type.address() as u32;
        tracing::debug!(
            "  Exception vector: {:?} -> address 0x{:08X}, mode {:?}",
            exception_type,
            new_pc,
            exception_type.mode()
        );
        self.registers.set_program_counter(new_pc);

        // flush pipeline, the next step() will refill it naturally
        self.flush_pipeline();
    }

    /// Execute one CPU cycle.
    ///
    /// Returns `true` if `VBlank` just started (a new frame is ready to display).
    pub fn step(&mut self) -> bool {
        self.current_cycle += 1;
        let initial_mode = self.cpsr.cpu_state();

        match initial_mode {
            CpuState::Thumb => {
                let to_execute = self.decoded_thumb;

                // Move fetched instruction to decode stage
                self.decoded_thumb = self.fetched_thumb.map(Self::decode);

                // Fetch new instruction into pipeline
                self.fetched_thumb = Some(self.fetch_thumb());

                if let Some(decoded) = to_execute {
                    if !self.cpsr.irq_disable() && self.bus.is_irq_pending() {
                        self.handle_exception(ExceptionType::Irq);

                        // Still need to step peripherals even on IRQ
                        return self.bus.step();
                    }

                    self.execute_thumb(decoded);

                    // If execution changed CPU mode, clear the ENTIRE pipeline
                    // since we're switching between ARM/Thumb modes
                    if self.cpsr.cpu_state() != initial_mode {
                        tracing::debug!(
                            "MODE CHANGE DETECTED (Thumb): Clearing pipeline (PC=0x{:08X})",
                            self.registers.program_counter()
                        );
                        self.flush_pipeline();
                    }

                    // If execution flushed the pipeline, clear decoded too
                    if self.fetched_thumb.is_none() {
                        self.decoded_thumb = None;
                    }
                }

                // Advance PC to point to the next instruction to fetch
                // This happens every cycle to maintain the pipeline
                // BUT: If the pipeline was flushed (by a branch), don't advance PC
                // because the branch already set the correct target address
                if self.fetched_thumb.is_some() {
                    let old_pc = self.registers.program_counter() as u32;
                    let mut new_pc = old_pc + thumb::operations::SIZE_OF_INSTRUCTION;

                    // Ensure PC stays halfword-aligned in Thumb mode
                    new_pc.set_bit_off(0);

                    // Log PC advancement for debugging the loop
                    if old_pc == 0x081DCA90 || old_pc == 0x081DCA92 || old_pc == 0x081DCCE8 {
                        tracing::debug!("ADVANCING PC (Thumb): 0x{old_pc:08X} -> 0x{new_pc:08X}");
                    }

                    // Detect PC going to invalid address (not ROM 0x08000000+, not RAM 0x02000000+ or 0x03000000+, not BIOS 0x0-0x4000)
                    if new_pc > 0x00010000 && new_pc < 0x02000000 {
                        tracing::warn!(
                            "!!! SUSPICIOUS PC JUMP (Thumb) !!!\n  PC advancing to 0x{new_pc:08X} (invalid address range!)"
                        );
                    }

                    self.registers.set_program_counter(new_pc);
                }
            }
            CpuState::Arm => {
                let to_execute = self.decoded_arm;

                // Move fetched instruction to decode stage
                self.decoded_arm = self.fetched_arm.map(Self::decode);

                // Fetch new instruction into pipeline
                self.fetched_arm = Some(self.fetch_arm());

                if let Some(decoded) = to_execute {
                    if !self.cpsr.irq_disable() && self.bus.is_irq_pending() {
                        self.handle_exception(ExceptionType::Irq);

                        // Still need to step peripherals even on IRQ
                        return self.bus.step();
                    }

                    self.execute_arm(decoded);

                    // If execution changed CPU mode, clear the ENTIRE pipeline
                    // since we're switching between ARM/Thumb modes
                    if self.cpsr.cpu_state() != initial_mode {
                        tracing::debug!(
                            "MODE CHANGE DETECTED (ARM): Clearing pipeline (PC=0x{:08X})",
                            self.registers.program_counter()
                        );
                        self.flush_pipeline();
                    }

                    // If execution flushed the pipeline (e.g. BX without mode change),
                    // the flush_pipeline() already set fetched_arm and decoded_arm to None.
                    // We need to also clear the decoded instruction we set BEFORE execute.
                    // But since we set fetched_arm to Some() before execute, we can detect
                    // a flush by checking if fetched_arm is now None.
                    if self.fetched_arm.is_none() {
                        self.decoded_arm = None;
                    }
                }

                // Advance PC to point to the next instruction to fetch
                // This happens every cycle to maintain the pipeline
                // BUT: If the pipeline was flushed (by a branch), don't advance PC
                // because the branch already set the correct target address
                if self.fetched_arm.is_some() {
                    let mut new_pc = self.registers.program_counter() as u32
                        + arm::operations::SIZE_OF_INSTRUCTION;

                    // Ensure PC stays word-aligned in ARM mode
                    new_pc.set_bit_off(0);
                    new_pc.set_bit_off(1);

                    // Detect PC going to invalid address
                    if new_pc > 0x00010000 && new_pc < 0x02000000 {
                        tracing::warn!(
                            "!!! SUSPICIOUS PC JUMP (ARM) !!!\n  PC advancing to 0x{new_pc:08X} (invalid address range!)"
                        );
                    }

                    self.registers.set_program_counter(new_pc);
                }
            }
        }

        // Step the bus (LCD, timers, DMA, etc.) after CPU instruction completes
        // This ensures peripherals advance in sync with CPU cycles
        self.bus.step()
    }

    #[must_use]
    pub fn new(bus: Bus) -> Self {
        let mut cpu = Self {
            bus,
            ..Default::default()
        };

        // Initialize stack pointers for different modes
        // These values match what the GBA BIOS sets up
        cpu.initialize_stack_pointers();

        cpu
    }

    /// Initialize stack pointers for all CPU modes to match GBA BIOS behavior
    fn initialize_stack_pointers(&mut self) {
        // Save current mode
        let current_mode = self.cpsr.mode();

        // Set up IRQ mode stack pointer
        self.swap_mode(Mode::Irq);
        self.registers.set_register_at(REG_SP, 0x0300_7FA0);

        // Set up Supervisor mode stack pointer
        self.swap_mode(Mode::Supervisor);
        self.registers.set_register_at(REG_SP, 0x0300_7FE0);

        // Set up System mode stack pointer
        self.swap_mode(Mode::System);
        self.registers.set_register_at(REG_SP, 0x0300_7F00);

        // Restore original mode (Supervisor)
        self.swap_mode(current_mode);
    }

    pub fn swap_mode(&mut self, new_mode: Mode) {
        if self.cpsr.mode() == new_mode {
            return;
        }

        match self.cpsr.mode() {
            // If we leave Fiq we store r8-14 and spsr.
            // We should also restore r8-r12 since other modes do not have it banked
            Mode::Fiq => {
                self.register_bank.r8_fiq = self.registers.register_at(8);
                self.register_bank.r9_fiq = self.registers.register_at(9);
                self.register_bank.r10_fiq = self.registers.register_at(10);
                self.register_bank.r11_fiq = self.registers.register_at(11);
                self.register_bank.r12_fiq = self.registers.register_at(12);
                self.register_bank.r13_fiq = self.registers.register_at(13);
                self.register_bank.r14_fiq = self.registers.register_at(14);
                self.register_bank.spsr_fiq = self.spsr;

                self.registers.set_register_at(8, self.register_bank.r8_old);
                self.registers.set_register_at(9, self.register_bank.r9_old);
                self.registers
                    .set_register_at(10, self.register_bank.r10_old);
                self.registers
                    .set_register_at(11, self.register_bank.r11_old);
                self.registers
                    .set_register_at(12, self.register_bank.r12_old);
            }
            // If we leave System or User we store r13-14
            Mode::System | Mode::User => {
                self.register_bank.r13_old = self.registers.register_at(13);
                self.register_bank.r14_old = self.registers.register_at(14);
            }
            // Otherwise we store r13-14 and spsr
            Mode::Supervisor => {
                self.register_bank.r13_svc = self.registers.register_at(13);
                self.register_bank.r14_svc = self.registers.register_at(14);
                self.register_bank.spsr_svc = self.spsr;
            }
            Mode::Abort => {
                self.register_bank.r13_abt = self.registers.register_at(13);
                self.register_bank.r14_abt = self.registers.register_at(14);
                self.register_bank.spsr_abt = self.spsr;
            }
            Mode::Irq => {
                self.register_bank.r13_irq = self.registers.register_at(13);
                self.register_bank.r14_irq = self.registers.register_at(14);
                self.register_bank.spsr_irq = self.spsr;
            }
            Mode::Undefined => {
                self.register_bank.r13_und = self.registers.register_at(13);
                self.register_bank.r14_und = self.registers.register_at(14);
                self.register_bank.spsr_und = self.spsr;
            }
        }

        match new_mode {
            // If we enter Fiq we restore r8-14 and spsr.
            // We should also store r8-12 otherwise we lose them.
            Mode::Fiq => {
                self.register_bank.r8_old = self.registers.register_at(8);
                self.register_bank.r9_old = self.registers.register_at(9);
                self.register_bank.r10_old = self.registers.register_at(10);
                self.register_bank.r11_old = self.registers.register_at(11);
                self.register_bank.r12_old = self.registers.register_at(12);

                self.registers.set_register_at(8, self.register_bank.r8_fiq);
                self.registers.set_register_at(9, self.register_bank.r9_fiq);
                self.registers
                    .set_register_at(10, self.register_bank.r10_fiq);
                self.registers
                    .set_register_at(11, self.register_bank.r11_fiq);
                self.registers
                    .set_register_at(12, self.register_bank.r12_fiq);
                self.registers
                    .set_register_at(13, self.register_bank.r13_fiq);
                self.registers
                    .set_register_at(14, self.register_bank.r14_fiq);

                self.spsr = self.register_bank.spsr_fiq;
            }
            // If we enter System or User we restore r13-14
            Mode::System | Mode::User => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_old);
                self.registers
                    .set_register_at(14, self.register_bank.r14_old);
            }
            // Otherwise we restore r13-14 and spsr
            Mode::Supervisor => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_svc);
                self.registers
                    .set_register_at(14, self.register_bank.r14_svc);
                self.spsr = self.register_bank.spsr_svc;
            }
            Mode::Abort => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_abt);
                self.registers
                    .set_register_at(14, self.register_bank.r14_abt);
                self.spsr = self.register_bank.spsr_abt;
            }
            Mode::Irq => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_irq);
                self.registers
                    .set_register_at(14, self.register_bank.r14_irq);
                self.spsr = self.register_bank.spsr_irq;
            }
            Mode::Undefined => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_und);
                self.registers
                    .set_register_at(14, self.register_bank.r14_und);
                self.spsr = self.register_bank.spsr_und;
            }
        }

        self.cpsr.set_mode(new_mode);
    }

    /// Read a register value from user mode register bank
    /// Used for LDM/STM with S bit when R15 is not in register list
    pub(crate) fn read_user_register(&self, reg: usize) -> u32 {
        match reg {
            0..=7 => self.registers.register_at(reg), // r0-r7 are never banked
            8 => self.register_bank.r8_old,
            9 => self.register_bank.r9_old,
            10 => self.register_bank.r10_old,
            11 => self.register_bank.r11_old,
            12 => self.register_bank.r12_old,
            13 => self.register_bank.r13_old,
            14 => self.register_bank.r14_old,
            15 => self.registers.register_at(15), // PC is never banked
            _ => panic!("Invalid register: {reg}"),
        }
    }

    /// Write a register value to user mode register bank
    /// Used for LDM/STM with S bit when R15 is not in register list
    pub(crate) fn write_user_register(&mut self, reg: usize, value: u32) {
        match reg {
            0..=7 => self.registers.set_register_at(reg, value), // r0-r7 are never banked
            8 => self.register_bank.r8_old = value,
            9 => self.register_bank.r9_old = value,
            10 => self.register_bank.r10_old = value,
            11 => self.register_bank.r11_old = value,
            12 => self.register_bank.r12_old = value,
            13 => self.register_bank.r13_old = value,
            14 => self.register_bank.r14_old = value,
            15 => self.registers.set_register_at(15, value), // PC is never banked
            _ => panic!("Invalid register: {reg}"),
        }
    }

    pub fn read_half_word(&mut self, address: usize, sign_extended: bool) -> u32 {
        // Misaligned reads are unsupported in ARMv4.
        // When reading an half-word from a misaligned halfword address (even address)
        // the CPU will read at the aligned halfword address and will put the selected
        // byte to the lower byte of the address. That's why we rotate right by 8 if the lowest
        // in the address is 1.

        let rotation = ((address & 0b1) * 8) as u32;
        let mut value = (self.bus.read_half_word(address) as u32).rotate_right(rotation);

        if sign_extended {
            let is_halfword_aligned: bool = address & 0b1 == 0;

            // If the address is halfword aligned then we didn't rotate it so we can extend the entire 16 bits.
            // If the address was not halfword aligned we rotated it so that the selected halfword in now
            // in the lower 8 bits. We should extend only these 8 bits, making this operation equal to
            // a Load sign-extended Byte.
            value = value.sign_extended(if is_halfword_aligned { 16 } else { 8 });
        }

        value
    }

    pub fn read_word(&mut self, address: usize) -> u32 {
        // From documentation: An address offset from a word boundary will cause the data to be rotated
        // into the register so that the addressed byte occupies bits 0 to 7.
        // So if the last 2 bits of the address are 01, we still word-align the address but the byte 1 of the
        // read word will be in the lower 0-7 bits of the register. That's why we rotate it.
        let rotation = ((address & 0b11) * 8) as u32;
        self.bus.read_word(address).rotate_right(rotation)
    }

    /// Handle SWI calls with High-Level Emulation
    /// Returns true if handled, false if BIOS should handle it
    fn handle_swi_hle(&mut self, swi_num: u32, old_cpsr: Psr, return_addr: u32) -> bool {
        match swi_num {
            // SWI 0x00: SoftReset - Reset the GBA
            0x00 => {
                // Clear 200h bytes of IWRAM work area (03007E00h-03007FFFh)
                for addr in 0x03007E00..=0x03007FFF {
                    self.bus.write_byte(addr, 0);
                }

                // Check flag at 03007FFAh to determine entry point
                let flag = self.bus.read_byte(0x03007FFA);
                let entry_point = if flag == 0 { 0x08000000 } else { 0x02000000 };

                // Set R0-R12 to 0
                for i in 0..=12 {
                    self.registers.set_register_at(i, 0);
                }

                // Set up stack pointers (same as BIOS init)
                // Supervisor mode SP = 0x03007FE0
                // IRQ mode SP = 0x03007FA0
                // User/System mode SP = 0x03007F00

                // Switch to Supervisor mode temporarily to set its SP
                self.cpsr.set_mode(Mode::Supervisor);
                self.registers.set_register_at(13, 0x03007FE0);

                // Switch to IRQ mode to set its SP
                self.cpsr.set_mode(Mode::Irq);
                self.registers.set_register_at(13, 0x03007FA0);

                // Switch to System mode for entry
                self.cpsr.set_mode(Mode::System);
                self.registers.set_register_at(13, 0x03007F00);

                // Clear IRQ disable flag
                self.cpsr.set_irq_disable(false);

                // Clear state bit (ARM mode)
                self.cpsr.set_state_bit(false);

                // Jump to entry point
                self.registers.set_program_counter(entry_point);

                // Flush pipeline
                self.flush_pipeline();

                tracing::debug!("SoftReset: Jumping to 0x{entry_point:08X} (flag was {flag})");

                true
            }
            // SWI 0x01: RegisterRamReset - Clear memory and I/O
            0x01 => {
                let flags = self.registers.register_at(0) as u8;
                // Clear memory regions based on flags
                // Bit 0: Clear 256K EWRAM (0x02000000-0x0203FFFF), excluding last 0x200 bytes
                if flags & 0x01 != 0 {
                    for addr in 0x02000000..0x0203FE00 {
                        self.bus.write_byte(addr, 0);
                    }
                }

                // Bit 1: Clear 32K IWRAM (0x03000000-0x03007FFF), excluding last 0x200 bytes
                // NOTE: Skipping IWRAM clear for now as it breaks IRQ handlers that games set up before calling this
                // The real BIOS might have special logic to preserve certain areas
                // TODO: Investigate proper BIOS behavior
                if flags & 0x02 != 0 {
                    // Don't clear IWRAM for now - needs more investigation
                    tracing::debug!(
                        "RegisterRamReset: Skipping IWRAM clear (would break IRQ handlers)"
                    );
                }

                // Bit 2: Clear Palette RAM (0x05000000-0x050003FF)
                if flags & 0x04 != 0 {
                    for addr in 0x05000000..0x05000400 {
                        self.bus.write_byte(addr, 0);
                    }
                }

                // Bit 3: Clear VRAM (0x06000000-0x06017FFF)
                if flags & 0x08 != 0 {
                    for addr in 0x06000000..0x06018000 {
                        self.bus.write_byte(addr, 0);
                    }
                }

                // Bit 4: Clear OAM (0x07000000-0x070003FF)
                if flags & 0x10 != 0 {
                    for addr in 0x07000000..0x07000400 {
                        self.bus.write_byte(addr, 0);
                    }
                }

                // Bits 5-7: Reset I/O registers (not fully implemented)
                // Bit 5: SIO registers
                // Bit 6: Sound registers
                // Bit 7: All other registers

                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x02: Halt - Low power mode until interrupt
            0x02 => {
                tracing::debug!("HLE SWI 0x02: Halt");
                // Just return - the main loop will handle waiting for interrupts
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x03: Stop - Very low power mode
            0x03 => {
                tracing::debug!("HLE SWI 0x03: Stop");
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x04: IntrWait - Wait for interrupt
            0x04 => {
                tracing::debug!(
                    "HLE SWI 0x04: IntrWait - discard={}, flags=0x{:08X}",
                    self.registers.register_at(0),
                    self.registers.register_at(1)
                );
                // Just return for now - proper implementation would wait for specific interrupts
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x05: VBlankIntrWait - Wait for VBlank interrupt
            0x05 => {
                tracing::debug!("HLE SWI 0x05: VBlankIntrWait");
                // Just return for now
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x06: Div - Signed division
            // R0 = numerator, R1 = denominator
            // Returns: R0 = result, R1 = remainder, R3 = abs(result)
            0x06 => {
                let numerator = self.registers.register_at(0) as i32;
                let denominator = self.registers.register_at(1) as i32;
                if denominator == 0 {
                    tracing::warn!("HLE SWI 0x06: Div by zero!");
                    // On real hardware, division by zero causes weird behavior
                    self.registers.set_register_at(0, 0);
                    self.registers.set_register_at(1, 0);
                    self.registers.set_register_at(3, 0);
                } else {
                    let result = numerator / denominator;
                    let modulo = numerator % denominator;
                    self.registers.set_register_at(0, result as u32);
                    self.registers.set_register_at(1, modulo as u32);
                    self.registers.set_register_at(3, result.unsigned_abs());
                }
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x07: DivArm - Same as Div but with swapped arguments
            // R0 = denominator, R1 = numerator (swapped from SWI 0x06)
            // Returns: R0 = result, R1 = remainder, R3 = abs(result)
            0x07 => {
                let denominator = self.registers.register_at(0) as i32;
                let numerator = self.registers.register_at(1) as i32;
                if denominator == 0 {
                    tracing::warn!("HLE SWI 0x07: DivArm by zero!");
                    self.registers.set_register_at(0, 0);
                    self.registers.set_register_at(1, 0);
                    self.registers.set_register_at(3, 0);
                } else {
                    let result = numerator / denominator;
                    let modulo = numerator % denominator;
                    self.registers.set_register_at(0, result as u32);
                    self.registers.set_register_at(1, modulo as u32);
                    self.registers.set_register_at(3, result.unsigned_abs());
                }
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x08: Sqrt - Square root
            // R0 = input (32-bit unsigned)
            // Returns: R0 = sqrt(input)
            0x08 => {
                let input = self.registers.register_at(0);
                // Integer square root using Newton's method
                let result = if input == 0 {
                    0
                } else {
                    let mut x = input;
                    let mut y = x.div_ceil(2);
                    while y < x {
                        x = y;
                        y = u32::midpoint(x, input / x);
                    }
                    x
                };
                self.registers.set_register_at(0, result);
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x09: ArcTan - Arc tangent
            // R0 = tan (16-bit, range -1.0 to 1.0 scaled to -0x4000 to 0x4000)
            // Returns: R0 = angle (-pi/4 to pi/4 scaled to -0x2000 to 0x2000)
            0x09 => {
                // Simplified implementation - just return a reasonable approximation
                let tan = self.registers.register_at(0) as i16 as i32;
                // Simple linear approximation for small angles: arctan(x) ≈ x
                // Scale from tan range to angle range (divide by 2)
                let result = (tan / 2) as u32;
                self.registers.set_register_at(0, result);
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x0A: ArcTan2 - Arc tangent of y/x
            // R0 = x, R1 = y
            // Returns: R0 = angle (0 to 2*pi scaled to 0x0000 to 0xFFFF)
            0x0A => {
                let x = self.registers.register_at(0) as i16 as f64;
                let y = self.registers.register_at(1) as i16 as f64;
                let angle = y.atan2(x);
                // Convert from radians (-pi to pi) to GBA format (0 to 0xFFFF)
                let result = ((angle + std::f64::consts::PI) / (2.0 * std::f64::consts::PI)
                    * 65536.0) as u32
                    & 0xFFFF;
                self.registers.set_register_at(0, result);
                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x0B: CpuSet - Memory copy
            0x0B => {
                let src = self.registers.register_at(0);
                let dest = self.registers.register_at(1);
                let control = self.registers.register_at(2);

                let count = control & 0x1FFFFF;
                let is_32bit = (control & (1 << 26)) != 0;
                let is_fill = (control & (1 << 24)) != 0;

                if is_fill {
                    // Fill mode: read one value from src and write it count times to dest
                    let value = if is_32bit {
                        self.bus.read_word(src as usize)
                    } else {
                        self.bus.read_half_word(src as usize) as u32
                    };

                    for i in 0..count {
                        let offset = if is_32bit { i * 4 } else { i * 2 };
                        if is_32bit {
                            self.bus.write_word((dest + offset) as usize, value);
                        } else {
                            self.bus
                                .write_half_word((dest + offset) as usize, value as u16);
                        }
                    }
                } else {
                    // Copy mode
                    for i in 0..count {
                        let offset = if is_32bit { i * 4 } else { i * 2 };
                        let value = if is_32bit {
                            self.bus.read_word((src + offset) as usize)
                        } else {
                            self.bus.read_half_word((src + offset) as usize) as u32
                        };

                        if is_32bit {
                            self.bus.write_word((dest + offset) as usize, value);
                        } else {
                            self.bus
                                .write_half_word((dest + offset) as usize, value as u16);
                        }
                    }
                }

                self.swi_return(old_cpsr, return_addr);
                true
            }
            // SWI 0x0C: CpuFastSet - Fast memory copy (32-bit only)
            0x0C => {
                let src = self.registers.register_at(0);
                let dest = self.registers.register_at(1);
                let control = self.registers.register_at(2);

                let count = control & 0x1FFFFF;
                let is_fill = (control & (1 << 24)) != 0;

                if is_fill {
                    let value = self.bus.read_word(src as usize);
                    for i in 0..count {
                        self.bus.write_word((dest + i * 4) as usize, value);
                    }
                } else {
                    for i in 0..count {
                        let value = self.bus.read_word((src + i * 4) as usize);
                        self.bus.write_word((dest + i * 4) as usize, value);
                    }
                }

                self.swi_return(old_cpsr, return_addr);
                true
            }
            _ => {
                // Not implemented - let BIOS handle it
                // TODO: Add tracing log
                false
            }
        }
    }

    /// Helper to return from a SWI HLE implementation
    const fn swi_return(&mut self, old_cpsr: Psr, return_addr: u32) {
        // Restore old CPU state
        self.cpsr = old_cpsr;

        // Set PC to return address
        self.registers.set_program_counter(return_addr);

        // Flush pipeline
        self.flush_pipeline();
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HalfwordTransferKind {
    UnsignedHalfwords,
    SignedByte,
    SignedHalfwords,
}

impl From<u8> for HalfwordTransferKind {
    fn from(value: u8) -> Self {
        match value.get_bits(0..=1) {
            0b01 => Self::UnsignedHalfwords,
            0b10 => Self::SignedByte,
            0b11 => Self::SignedHalfwords,
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for HalfwordTransferKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsignedHalfwords => write!(f, "H"),
            Self::SignedByte => write!(f, "SB"),
            Self::SignedHalfwords => write!(f, "SH"),
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::cpu::condition::Condition;
    use crate::cpu::flags::{HalfwordDataTransferOffsetKind, Indexing, LoadStoreKind, Offsetting};
    use crate::cpu::registers::{REG_LR, REG_PC, REG_SP};
    use crate::cpu::thumb::instruction::Instruction;

    use super::*;

    #[test]
    fn arm_branch() {
        // Covers a positive offset

        // 15(1111b) << 2 = 60 bytes
        let op_code = 0b1110_1010_0000_0000_0000_0000_0000_1111;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.program_counter(), 60);

        // Covers a negative offset

        // -9 << 2 = -36 bytes
        let op_code = 0b1110_1010_1111_1111_1111_1111_1111_0111;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.program_counter(), 60 - 36);

        // Covers link

        let op_code = 0b1110_1011_0000_0000_0000_0000_0000_1111;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);

        // -4 because of pipelining (pc is at +8 so we've to do -4 to get the next instruction)
        assert_eq!(cpu.registers.register_at(14), 24 - 4);
    }

    #[test]
    #[should_panic]
    fn arm_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::default();

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute_arm(op_code);
    }

    #[test]
    fn arm_block_data_transfer() {
        // Use EWRAM base address for tests (0x02000000)
        const BASE: u32 = 0x0200_1000;
        {
            // LDM with post-increment
            let op_code = 0b1110_100_0_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, BASE);
            cpu.bus.write_byte(BASE as usize, 1);
            cpu.bus.write_byte((BASE + 4) as usize, 5);
            cpu.bus.write_byte((BASE + 8) as usize, 7);
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), BASE + 0xC);
        }
        {
            // LDM with pre-increment
            let op_code = 0b1110_100_1_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, BASE);
            cpu.bus.write_byte((BASE + 4) as usize, 1);
            cpu.bus.write_byte((BASE + 8) as usize, 5);
            cpu.bus.write_byte((BASE + 0xC) as usize, 7);
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), BASE + 0xC);
        }
        {
            // LDM with post-decrement
            let op_code = 0b1110_100_0_0_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, BASE);
            cpu.bus.write_byte(BASE as usize, 7);
            cpu.bus.write_byte((BASE - 4) as usize, 5);
            cpu.bus.write_byte((BASE - 8) as usize, 1);
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), BASE - 0xC);
        }
        {
            // LDM with pre-decrement
            let op_code = 0b1110_100_1_0_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, BASE);
            cpu.bus.write_byte((BASE - 4) as usize, 7);
            cpu.bus.write_byte((BASE - 8) as usize, 5);
            cpu.bus.write_byte((BASE - 0xC) as usize, 1);
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), BASE - 0xC);
        }
        {
            // STM with post-increment
            let op_code = 0b1110_100_0_1_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, BASE);

            cpu.execute_arm(op_code);

            let mut bus = cpu.bus;

            assert_eq!(bus.read_byte(BASE as usize), 1);
            assert_eq!(bus.read_byte((BASE + 4) as usize), 5);
            assert_eq!(bus.read_byte((BASE + 8) as usize), 7);
            assert_eq!(cpu.registers.register_at(13), BASE + 0xC);
        }
        {
            // STM with pre-increment
            let op_code = 0b1110_100_1_1_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, BASE);

            cpu.execute_arm(op_code);

            let mut bus = cpu.bus;

            assert_eq!(bus.read_byte(BASE as usize), 0);
            assert_eq!(bus.read_byte((BASE + 4) as usize), 1);
            assert_eq!(bus.read_byte((BASE + 8) as usize), 5);
            assert_eq!(bus.read_byte((BASE + 0xC) as usize), 7);
            assert_eq!(cpu.registers.register_at(13), BASE + 0xC);
        }
        {
            // STM with post-decrement
            let op_code = 0b1110_100_0_0_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, BASE);

            cpu.execute_arm(op_code);

            let mut bus = cpu.bus;

            assert_eq!(bus.read_byte(BASE as usize), 7);
            assert_eq!(bus.read_byte((BASE - 4) as usize), 5);
            assert_eq!(bus.read_byte((BASE - 8) as usize), 1);
            assert_eq!(cpu.registers.register_at(13), BASE - 0xC);
        }
        {
            // STM with pre-decrement and storing R15

            let op_code = 0b1110_100_1_0_0_1_0_1101_1000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, BASE);

            cpu.execute_arm(op_code);

            let mut bus = cpu.bus;

            assert_eq!(bus.read_byte(BASE as usize), 0);
            assert_eq!(bus.read_byte((BASE - 4) as usize), 15 + 4);
            assert_eq!(bus.read_byte((BASE - 8) as usize), 7);
            assert_eq!(bus.read_byte((BASE - 0xC) as usize), 5);
            assert_eq!(bus.read_byte((BASE - 0x10) as usize), 1);
            assert_eq!(cpu.registers.register_at(13), BASE - 0x10);
        }
    }

    #[test]
    fn arm_half_word_data_transfer() {
        // Use EWRAM base address for tests
        const EWRAM: u32 = 0x0200_0000;
        {
            // Register offset
            let op_code = 0b1110_0001_1000_0010_0000_0000_1011_0001;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::HalfwordDataTransfer {
                    condition: Condition::AL,
                    indexing: Indexing::Pre,
                    offsetting: Offsetting::Up,
                    write_back: false,
                    load_store_kind: LoadStoreKind::Store,
                    offset_kind: HalfwordDataTransferOffsetKind::Register { register: 1 },
                    base_register: 2,
                    source_destination_register: 0,
                    transfer_kind: HalfwordTransferKind::UnsignedHalfwords,
                }
            );

            cpu.registers.set_register_at(0, 16843009);
            cpu.registers.set_register_at(2, EWRAM); // set base register
            cpu.execute_arm(op_code);

            let mut bus = cpu.bus;

            assert_eq!(bus.read_byte(EWRAM as usize), 1);
            assert_eq!(bus.read_byte((EWRAM + 1) as usize), 1);
            // because we store halfword = 16bit
            assert_eq!(bus.read_byte((EWRAM + 2) as usize), 0);
            assert_eq!(bus.read_byte((EWRAM + 3) as usize), 0);
        }
        {
            // Immediate offset, pre-index, down, no wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_0_1_0000_0001_0001_1_01_1_1100;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.bus
                .write_word((EWRAM + 100 - 0b11100) as usize, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100);
        }
        {
            // Immediate offset, pre-index, down, wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_1_1_0000_0001_0001_1_01_1_1100;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.bus
                .write_word((EWRAM + 100 - 0b11100) as usize, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11100);
        }
        {
            // Immediate offset, pre-index, up, wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_1_1_1_1_0000_0001_0001_1_01_1_1100;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.bus
                .write_word((EWRAM + 100 + 0b11100) as usize, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 + 0b11100);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_1_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.bus.write_word((EWRAM + 100) as usize, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), load, signed byte
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_1_0000_0001_0001_1_10_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.bus.write_byte((EWRAM + 100) as usize, -5_i8 as u8);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), -5_i32 as u32);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), load, signed halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_1_0000_0001_0001_1_11_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.bus
                .write_half_word((EWRAM + 100) as usize, -300_i16 as u16);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), -300_i32 as u32);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), store, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_0_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_register_at(1, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.bus.read_word((EWRAM + 100) as usize), 0x1234);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), store PC, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_0_0000_1111_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_program_counter(500);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.bus.read_word((EWRAM + 100) as usize), 504);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11111);
        }
        {
            // Immediate offset, pre-index, down, no wb, store PC, unsigned halfword, base PC
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_0_0_1111_1111_0001_1_01_1_1100;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_program_counter(EWRAM + 500);

            cpu.execute_arm(op_code);

            // STRH stores only 16 bits, so only the lower 16 bits of PC+4 are stored
            // PC + 4 = EWRAM + 504 = 0x020001F8, lower 16 bits = 0x01F8 = 504
            assert_eq!(
                cpu.bus.read_half_word((EWRAM + 500 - 0b11100) as usize),
                ((EWRAM + 504) & 0xFFFF) as u16
            );
            assert_eq!(cpu.registers.program_counter(), (EWRAM + 500) as usize);
        }
        {
            // Register offset, post-index, down, no wb (but implicit), store PC, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_0_0_0_0000_1111_0000_1_01_1_0010;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_program_counter(500);
            cpu.registers.set_register_at(2, 0b11111);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.bus.read_word((EWRAM + 100) as usize), 504);
            assert_eq!(cpu.registers.register_at(0), EWRAM + 100 - 0b11111);
        }
    }

    #[test]
    fn thumb_pc_relative_load() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b0100_1001_0101_1000_u16;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

        // PC-relative load: address = (PC & ~3) + offset * 4
        // offset = 0x58 = 88 (352 when shifted), so address = (PC & ~3) + 352
        // Set PC to EWRAM + 4 to simulate pipeline-adjusted value
        cpu.registers.set_program_counter(EWRAM + 4);
        cpu.registers.set_register_at(1, 10);
        // Address = (EWRAM + 4 & ~3) + 352 = EWRAM + 4 + 352 = EWRAM + 356
        cpu.bus.write_word((EWRAM + 356) as usize, 1);
        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.register_at(1), 1);
    }

    #[test]
    fn thumb_load_store_register_offset() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        // Checks Store Word
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_00_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_register_at(1, 100);
            cpu.registers.set_register_at(2, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.bus.read_word((EWRAM + 200) as usize), 0xFEEFAC1F);
        }
        // Checks Store Byte
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_01_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_register_at(1, 100);
            cpu.registers.set_register_at(2, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.bus.read_byte((EWRAM + 200) as usize), 0x1F);
            assert_eq!(cpu.bus.read_byte((EWRAM + 201) as usize), 0);
            assert_eq!(cpu.bus.read_byte((EWRAM + 202) as usize), 0);
            assert_eq!(cpu.bus.read_byte((EWRAM + 203) as usize), 0);
        }
        // Checks Load Word
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_10_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_register_at(1, 100);
            cpu.bus.write_word((EWRAM + 200) as usize, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(2), 0xFEEFAC1F);
        }
        // Checks Load Byte
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_11_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, EWRAM + 100);
            cpu.registers.set_register_at(1, 100);
            cpu.bus.write_word((EWRAM + 200) as usize, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(2), 0x1F);
        }
    }

    #[test]
    fn thumb_load_store_immediate_offset() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        {
            // Store Word
            let op_code = 0b0110_0011_0111_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert!(matches!(
                op_code.instruction,
                Instruction::LoadStoreImmOffset { .. }
            ));

            cpu.registers.set_register_at(7, EWRAM + 4);
            cpu.registers.set_register_at(0, 0xFFFF_FFFF);
            cpu.execute_thumb(op_code);

            let mut bus = cpu.bus;
            assert_eq!(bus.read_word((EWRAM + 56) as usize), 0xFFFF_FFFF);
        }
        {
            // Store Word misaligned
            let op_code = 0b0110_0011_0111_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert!(matches!(
                op_code.instruction,
                Instruction::LoadStoreImmOffset { .. }
            ));

            cpu.registers.set_register_at(7, EWRAM + 2);
            cpu.registers.set_register_at(0, 0xFFFF_FFFF);
            cpu.execute_thumb(op_code);

            let mut bus = cpu.bus;
            assert_eq!(bus.read_word((EWRAM + 52) as usize), 0xFFFF_FFFF);
        }
        {
            // Load Word
            let op_code = 0b0110_1011_0000_1111;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert!(matches!(
                op_code.instruction,
                Instruction::LoadStoreImmOffset { .. }
            ));
            cpu.bus.write_word((EWRAM + 1048) as usize, 0xFFFF_FFFF);
            cpu.registers.set_register_at(1, EWRAM + 1000);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(7), 0xFFFF_FFFF);
        }
        {
            // Store Byte
            let op_code = 0b0111_0010_0011_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert!(matches!(
                op_code.instruction,
                Instruction::LoadStoreImmOffset { .. }
            ));

            cpu.registers.set_register_at(7, EWRAM + 2);
            cpu.registers.set_register_at(0, 0xFFFF_FFFF);
            cpu.execute_thumb(op_code);

            let mut bus = cpu.bus;
            assert_eq!(bus.read_byte((EWRAM + 10) as usize), 0xFF);
        }
    }

    #[test]
    fn thumb_add_subtract() {
        // Check sub
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b00011_1_1_111_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 0b110);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }

        // Check add
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b00011_1_0_001_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, u32::MAX);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 0);
            assert!(cpu.cpsr.zero_flag());
            assert!(cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
    }

    #[test]
    fn thumb_cond_branch() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1101_1011_11111100;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

        cpu.registers.set_program_counter(1000);

        cpu.execute_thumb(op_code);

        // Asserting no branch since condition is not satisfied
        assert_eq!(cpu.registers.program_counter(), 1000);

        cpu.cpsr.set_sign_flag(true);

        let op_code = 0b1101_1011_11111100;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_thumb(op_code);

        // Asserting branch now that we set the condition
        assert_eq!(cpu.registers.program_counter(), 1000 - 8);
    }

    #[test]
    fn thumb_uncond_branch() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1110_0001_0010_1111;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

        cpu.registers.set_program_counter(1000);

        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.program_counter(), 1606);
    }

    #[test]
    fn thumb_hi_reg_operation_branch_ex() {
        {
            // BX Hs
            let mut cpu = Arm7tdmi::default();
            let op_code: u16 = 0b0100_0111_0111_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(14, 123);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.program_counter(), 122);
        }
        {
            // Add Rd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_00_0_1_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(1, 10);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 20);
        }
        {
            // Add Hd, Rs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_00_1_0_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 10);
            cpu.registers.set_register_at(9, 10);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 20);
        }
        {
            // Add Hd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_00_1_1_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(9, 10);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 20);
        }
        {
            // Cmp Rd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_01_0_1_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(1, 10);

            cpu.execute_thumb(op_code);

            assert!(cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.overflow_flag());
            assert!(cpu.cpsr.carry_flag());
        }
        {
            // Cmp Hd, Rs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_01_1_0_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 11);
            cpu.registers.set_register_at(9, 10);

            cpu.execute_thumb(op_code);

            assert!(!cpu.cpsr.zero_flag());
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
        {
            // Cmp Hd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_01_1_1_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(9, 11);

            cpu.execute_thumb(op_code);

            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.sign_flag());
            assert!(cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
        {
            // Mov Rd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_10_0_1_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(1, 11);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 10);
        }
        {
            // Mov Hd, Rs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_10_1_0_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 10);
            cpu.registers.set_register_at(9, 11);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 10);
        }
        {
            // Mov Hd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_10_1_1_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(9, 11);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 10);
        }
    }

    #[test]
    fn check_swap_mode() {
        // Cpu starts in Supervisor
        let mut cpu = Arm7tdmi::default();

        for i in 0..=15 {
            cpu.registers.set_register_at(i, i as u32);
        }

        cpu.spsr.set_carry_flag(true);

        // Simulating a mode swap from Supervisor to System
        cpu.swap_mode(Mode::System);

        assert_eq!(cpu.registers.register_at(13), 0);
        assert_eq!(cpu.registers.register_at(14), 0);

        cpu.registers.set_register_at(13, 100);
        cpu.registers.set_register_at(14, 200);

        // Simulating a mode swap from System to IRQ
        cpu.swap_mode(Mode::Irq);

        assert_eq!(cpu.registers.register_at(13), 0);
        assert_eq!(cpu.registers.register_at(14), 0);
        assert!(!cpu.spsr.carry_flag());

        // Simulating a mode swap back from IRQ to Supervisor
        cpu.swap_mode(Mode::Supervisor);

        assert_eq!(cpu.registers.register_at(13), 13);
        assert_eq!(cpu.registers.register_at(14), 14);
        assert!(cpu.spsr.carry_flag());

        // Simulating a mode swap to FIQ
        cpu.swap_mode(Mode::Fiq);
        assert_eq!(cpu.registers.register_at(8), 0);
        assert_eq!(cpu.registers.register_at(9), 0);
        assert_eq!(cpu.registers.register_at(10), 0);
        assert_eq!(cpu.registers.register_at(11), 0);
        assert_eq!(cpu.registers.register_at(12), 0);
        assert_eq!(cpu.registers.register_at(13), 0);
        assert_eq!(cpu.registers.register_at(14), 0);
        assert!(!cpu.spsr.carry_flag());

        // Simulating a mode swap to System
        cpu.swap_mode(Mode::System);
        assert_eq!(cpu.registers.register_at(8), 8);
        assert_eq!(cpu.registers.register_at(9), 9);
        assert_eq!(cpu.registers.register_at(10), 10);
        assert_eq!(cpu.registers.register_at(11), 11);
        assert_eq!(cpu.registers.register_at(12), 12);
        assert_eq!(cpu.registers.register_at(13), 100);
        assert_eq!(cpu.registers.register_at(14), 200);
    }

    #[test]
    fn thumb_push_pop_register() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        const SP_BASE: u32 = EWRAM + 1000;
        {
            // Store + save LR
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1011_0101_1111_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_program_counter(1000);
            cpu.registers.set_register_at(REG_LR, 1000);
            cpu.registers.set_register_at(REG_SP, SP_BASE);

            for r in 0..8 {
                cpu.registers.set_register_at(r, r.try_into().unwrap());
            }

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.bus.read_word((SP_BASE - 4) as usize), 1000);
            assert_eq!(cpu.bus.read_word((SP_BASE - 4 - 4) as usize), 7);
            assert_eq!(cpu.bus.read_word((SP_BASE - 4 - 4 - 4) as usize), 6);
            assert_eq!(cpu.bus.read_word((SP_BASE - 4 - 4 - 4 - 4) as usize), 5);
            assert_eq!(cpu.bus.read_word((SP_BASE - 4 - 4 - 4 - 4 - 4) as usize), 4);
        }
        {
            // Load + restore PC
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1011_1_10_1_1111_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, SP_BASE);

            cpu.bus.write_word(SP_BASE as usize, 100);
            cpu.bus.write_word((SP_BASE + 4) as usize, 200);
            cpu.bus.write_word((SP_BASE + 8) as usize, 300);
            cpu.bus.write_word((SP_BASE + 12) as usize, 400);
            cpu.bus.write_word((SP_BASE + 16) as usize, 500);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(4), 100);
            assert_eq!(cpu.registers.register_at(5), 200);
            assert_eq!(cpu.registers.register_at(6), 300);
            assert_eq!(cpu.registers.register_at(7), 400);
            assert_eq!(cpu.registers.register_at(REG_PC.try_into().unwrap()), 500);
            assert_eq!(cpu.registers.register_at(REG_SP), SP_BASE + 20);
        }
    }

    #[test]
    fn thumb_add_offset_sp() {
        {
            // Positive offset
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b10110000_0_0000111;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, 1000);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(REG_SP), 1000 + (7 << 2));
        }
        // Negative offset
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b10110000_1_0000111;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

        cpu.registers.set_register_at(REG_SP, 1000);

        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.register_at(REG_SP), 1000 - (7 << 2));
    }

    #[test]
    fn thumb_sp_relative_load() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        const SP_BASE: u32 = EWRAM + 100;
        {
            // Load
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1001_1_000_00000111;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, SP_BASE);
            cpu.bus.write_word((SP_BASE + 0b11100) as usize, 999);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 999);
        }
        {
            // Store
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1001_0_000_00000111;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, SP_BASE);
            cpu.registers.set_register_at(0, 999);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.bus.read_word((SP_BASE + 0b11100) as usize), 999);
        }
    }

    #[test]
    fn thumb_alu_op() {
        {
            // mul
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_0110_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.cpsr.set_sign_flag(true);
            cpu.cpsr.set_zero_flag(true);
            cpu.registers.set_register_at(0, 0xFFFF_FFFF);
            cpu.registers.set_register_at(4, 1);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 0xFFFF_FFFF);
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
        }
        {
            // and
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0000_0001_1000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            cpu.cpsr.set_sign_flag(true);
            cpu.cpsr.set_zero_flag(true);
            cpu.registers.set_register_at(0, 1000);
            cpu.registers.set_register_at(3, 8);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 8);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
        }
        {
            // tst
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0010_0011_1110;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.execute_thumb(op_code);

            assert!(!cpu.cpsr.sign_flag());
            assert!(cpu.cpsr.zero_flag());
        }
        {
            // orr
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_0010_1010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(2, 90);
            cpu.registers.set_register_at(5, 97);
            cpu.cpsr.set_sign_flag(true);
            cpu.cpsr.set_zero_flag(true);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(2), 123);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
        }
        {
            // mvn
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_1100_1111;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(7), !0);
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
        }
        {
            // lsl
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0000_1000_1000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 0x1);
            cpu.registers.set_register_at(1, 0x20);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 0);
            assert_eq!(cpu.registers.register_at(1), 32);
        }
    }

    #[test]
    fn thumb_long_branch_link() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1111_1000_0100_0000;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            Instruction::LongBranchLink {
                h: true,
                offset: 64
            }
        );

        cpu.registers.set_program_counter(100);
        cpu.registers.set_register_at(REG_LR, 200);
        cpu.execute_thumb(op_code);

        // 98 | 1
        assert_eq!(cpu.registers.register_at(REG_LR), 99);
        assert_eq!(cpu.registers.program_counter(), 328);
    }

    #[test]
    fn thumb_load_store_halfword() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        const BASE: u32 = EWRAM + 100;
        {
            // Load
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_1_00001_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, BASE);
            cpu.bus.write_half_word((BASE + 2) as usize, 0xFF);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 0xFF);
        }
        {
            // Store
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_0_00001_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, BASE);
            cpu.registers.set_register_at(1, 0xFF);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.bus.read_half_word((BASE + 2) as usize), 0xFF);
        }
    }

    #[test]
    fn thumb_load_store_sign_extend_byte_halfword() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        struct Test {
            opcode: u16,
            expected_decode: Instruction,
            prepare_fn: Box<dyn Fn(&mut Arm7tdmi)>,
            check_fn: Box<dyn Fn(Arm7tdmi)>,
        }

        let cases = vec![
            Test {
                opcode: 0b0101_0_0_1_000_001_010,
                expected_decode: Instruction::LoadStoreSignExtByteHalfword {
                    h: false,
                    sign_extend_flag: false,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, EWRAM + 100);
                    cpu.registers.set_register_at(2, 0xF000_F0FF);
                }),
                check_fn: Box::new(|mut cpu| {
                    assert_eq!(cpu.bus.read_half_word((EWRAM + 110) as usize), 0xF0FF);
                }),
            },
            Test {
                opcode: 0b0101_1_0_1_000_001_010,
                expected_decode: Instruction::LoadStoreSignExtByteHalfword {
                    h: true,
                    sign_extend_flag: false,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, EWRAM + 100);
                    cpu.bus.write_half_word((EWRAM + 110) as usize, 0xF0FF);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(2), 0xF0FF);
                }),
            },
            Test {
                opcode: 0b0101_0_1_1_000_001_010,
                expected_decode: Instruction::LoadStoreSignExtByteHalfword {
                    h: false,
                    sign_extend_flag: true,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, EWRAM + 100);
                    cpu.bus.write_byte((EWRAM + 110) as usize, 0x80);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(2), 0xFFFF_FF80);
                }),
            },
            Test {
                opcode: 0b0101_1_1_1_000_001_010,
                expected_decode: Instruction::LoadStoreSignExtByteHalfword {
                    h: true,
                    sign_extend_flag: true,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, EWRAM + 100);
                    cpu.bus.write_half_word((EWRAM + 110) as usize, 0x8030);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(2), 0xFFFF_8030);
                }),
            },
        ];

        for case in cases {
            let mut cpu = Arm7tdmi::default();
            let op_code = case.opcode;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(op_code.instruction, case.expected_decode);

            (*case.prepare_fn)(&mut cpu);

            cpu.execute_thumb(op_code);

            (*case.check_fn)(cpu);
        }
    }

    #[test]
    fn thumb_load_address() {
        struct Test {
            opcode: u16,
            expected_decode: Instruction,
            prepare_fn: Box<dyn Fn(&mut Arm7tdmi)>,
            check_fn: Box<dyn Fn(Arm7tdmi)>,
        }

        for case in [
            Test {
                opcode: 0b1010_1_001_00000010,
                expected_decode: Instruction::LoadAddress {
                    sp: true,
                    destination_register: 1,
                    offset: 8,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(REG_SP, 10);
                }),
                check_fn: Box::new(|cpu| {
                    let v = cpu.registers.register_at(1);
                    assert_eq!(v, 18);
                }),
            },
            Test {
                opcode: 0b1010_0_001_00000010,
                expected_decode: Instruction::LoadAddress {
                    sp: false,
                    destination_register: 1,
                    offset: 8,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_program_counter(10);
                }),
                check_fn: Box::new(|cpu| {
                    let v = cpu.registers.register_at(1);
                    assert_eq!(v, 16);
                }),
            },
        ] {
            let mut cpu = Arm7tdmi::default();
            let op_code = case.opcode;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(op_code.instruction, case.expected_decode);

            (*case.prepare_fn)(&mut cpu);

            cpu.execute_thumb(op_code);

            (*case.check_fn)(cpu);
        }
    }

    #[test]
    fn thumb_multiple_load_store() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
        const BASE: u32 = EWRAM + 100;
        struct Test {
            opcode: u16,
            expected_decode: Instruction,
            prepare_fn: Box<dyn Fn(&mut Arm7tdmi)>,
            check_fn: Box<dyn Fn(Arm7tdmi)>,
        }

        let cases = vec![
            Test {
                opcode: 0b1100_1_001_10100000,
                expected_decode: Instruction::MultipleLoadStore {
                    load_store: LoadStoreKind::Load,
                    base_register: 1,
                    register_list: 160,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(1, BASE);
                    cpu.bus.write_word(BASE as usize, 0xFF);
                    cpu.bus.write_word((BASE + 4) as usize, 0xFF);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(5), 0xFF);
                    assert_eq!(cpu.registers.register_at(7), 0xFF);
                    assert_eq!(cpu.registers.register_at(1), BASE + 8);
                }),
            },
            Test {
                opcode: 0b1100_0_001_10100000,
                expected_decode: Instruction::MultipleLoadStore {
                    load_store: LoadStoreKind::Store,
                    base_register: 1,
                    register_list: 160,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(1, BASE);
                    cpu.registers.set_register_at(5, 10);
                    cpu.registers.set_register_at(7, 20);
                }),
                check_fn: Box::new(|mut cpu| {
                    assert_eq!(cpu.bus.read_word(BASE as usize), 10);
                    assert_eq!(cpu.bus.read_word((BASE + 4) as usize), 20);
                    assert_eq!(cpu.registers.register_at(1), BASE + 8);
                }),
            },
        ];

        for case in cases {
            let mut cpu = Arm7tdmi::default();
            let op_code = case.opcode;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(op_code.instruction, case.expected_decode);

            (*case.prepare_fn)(&mut cpu);

            cpu.execute_thumb(op_code);

            (*case.check_fn)(cpu);
        }
    }
}
