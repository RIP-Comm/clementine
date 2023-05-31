use std::convert::TryInto;
use std::sync::{Arc, Mutex};

use logger::log;
use vecfixed::VecFixed;

use crate::bitwise::Bits;
use crate::cpu::arm;
use crate::cpu::arm::instructions::ArmModeInstruction;
use crate::cpu::arm::mode::ArmModeOpcode;
use crate::cpu::cpu_modes::Mode;
use crate::cpu::psr::{CpuState, Psr};
use crate::cpu::register_bank::RegisterBank;
use crate::cpu::thumb::instruction::ThumbModeInstruction;
use crate::cpu::thumb::mode::ThumbModeOpcode;
use crate::memory::internal_memory::InternalMemory;

use super::registers::Registers;
use super::thumb;

pub struct Arm7tdmi {
    pub(crate) memory: Arc<Mutex<InternalMemory>>,

    pub cpsr: Psr,
    pub spsr: Psr,
    pub registers: Registers,

    pub register_bank: RegisterBank,

    pub disassembler_buffer: VecFixed<1000, String>,

    fetched_arm: Option<u32>,
    decoded_arm: Option<ArmModeOpcode>,
    fetched_thumb: Option<u16>,
    decoded_thumb: Option<ThumbModeOpcode>,
}

impl Default for Arm7tdmi {
    fn default() -> Self {
        let mut s = Self {
            memory: Arc::new(Mutex::new(InternalMemory::default())),
            cpsr: Psr::from(Mode::Supervisor), // FIXME: Starting as Supervisor? Not sure
            spsr: Psr::default(),
            registers: Registers::default(),
            register_bank: RegisterBank::default(),
            disassembler_buffer: VecFixed::new(),
            fetched_arm: None,
            decoded_arm: None,
            fetched_thumb: None,
            decoded_thumb: None,
        };

        // Setting ARM mode at startup
        s.cpsr.set_cpu_state(CpuState::Arm);
        s.cpsr.set_irq_disable(true);
        s.cpsr.set_fiq_disable(true);

        s
    }
}

impl Arm7tdmi {
    pub fn flush_pipeline(&mut self) {
        self.decoded_arm = None;
        self.decoded_thumb = None;
        self.fetched_arm = None;
        self.fetched_thumb = None;
    }

    pub fn fetch_arm(&mut self) -> u32 {
        let mut pc = self.registers.program_counter() as u32;
        pc.set_bit_off(0);
        pc.set_bit_off(1);
        self.registers.set_program_counter(pc);

        self.memory.lock().unwrap().read_word(pc as usize)
    }

    pub fn fetch_thumb(&mut self) -> u16 {
        let mut pc = self.registers.program_counter() as u32;
        pc.set_bit_off(0);
        self.registers.set_program_counter(pc);

        self.memory.lock().unwrap().read_half_word(pc as usize)
    }

    pub fn decode<T, V>(op_code: V) -> T
    where
        T: std::fmt::Display + TryFrom<V>,
        <T as TryFrom<V>>::Error: std::fmt::Debug,
    {
        T::try_from(op_code).unwrap()
    }

    pub fn execute_arm(&mut self, op_code: ArmModeOpcode) {
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        if !self.cpsr.can_execute(op_code.condition) {
            return;
        }

        let decimal_value = self.registers.program_counter();
        let padded_hex_value = format!("{decimal_value:#04X}");
        self.disassembler_buffer.push(format!(
            "{}: {}",
            padded_hex_value,
            op_code.instruction.disassembler()
        ));

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
            ArmModeInstruction::PSRTransfer {
                condition: _,
                psr_kind,
                kind,
            } => self.psr_transfer(kind, psr_kind),
            ArmModeInstruction::Multiply => todo!(),
            ArmModeInstruction::MultiplyLong => todo!(),
            ArmModeInstruction::SingleDataSwap => todo!(),
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
            ArmModeInstruction::Undefined => todo!(),
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
            ArmModeInstruction::CoprocessorDataTransfer { .. } => todo!(),
            ArmModeInstruction::CoprocessorDataOperation => todo!(),
            ArmModeInstruction::CoprocessorRegisterTransfer => todo!(),
            ArmModeInstruction::SoftwareInterrupt => todo!(),
        };
    }

    pub fn execute_thumb(&mut self, op_code: ThumbModeOpcode) {
        let decimal_value = self.registers.program_counter();
        let padded_hex_value = format!("{decimal_value:#04X}");
        self.disassembler_buffer.push(format!(
            "{padded_hex_value}: {}",
            op_code.instruction.disassembler()
        ));

        match op_code.instruction {
            ThumbModeInstruction::MoveShiftedRegister {
                shift_operation: op,
                offset5,
                source_register,
                destination_register,
            } => self.move_shifted_reg(op, offset5, source_register, destination_register),
            ThumbModeInstruction::AddSubtract {
                operation_kind,
                op,
                rn_offset3,
                source_register: rs,
                destination_register: rd,
            } => self.add_subtract(operation_kind, op, rn_offset3, rs, rd),
            ThumbModeInstruction::MoveCompareAddSubtractImm {
                operation: op,
                destination_register: r_destination,
                offset,
            } => self.move_compare_add_sub_imm(op, r_destination, offset),
            ThumbModeInstruction::AluOp {
                alu_operation: op,
                source_register: rs,
                destination_register: rd,
            } => self.alu_op(op, rs, rd),
            ThumbModeInstruction::HiRegisterOpBX {
                register_operation: op,
                source_register,
                destination_register,
            } => self.hi_reg_operation_branch_ex(op, source_register, destination_register),
            ThumbModeInstruction::PCRelativeLoad {
                destination_register: r_destination,
                immediate_value,
            } => self.pc_relative_load(r_destination, immediate_value),
            ThumbModeInstruction::LoadStoreRegisterOffset {
                load_store,
                byte_word,
                ro,
                base_register: rb,
                destination_register: rd,
            } => self.load_store_register_offset(load_store, byte_word, ro, rb, rd),
            ThumbModeInstruction::LoadStoreSignExtByteHalfword {
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
            ThumbModeInstruction::LoadStoreImmOffset => self.load_store_immediate_offset(op_code),
            ThumbModeInstruction::LoadStoreHalfword {
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
            ThumbModeInstruction::SPRelativeLoadStore {
                load_store,
                destination_register: r_destination,
                word8,
            } => self.sp_relative_load_store(load_store, r_destination, word8),
            ThumbModeInstruction::LoadAddress {
                sp,
                destination_register: r_destination,
                offset,
            } => self.load_address(sp, r_destination.try_into().unwrap(), offset),
            ThumbModeInstruction::AddOffsetSP { s, word7 } => self.add_offset_sp(s, word7),
            ThumbModeInstruction::PushPopReg {
                load_store,
                pc_lr,
                register_list,
            } => self.push_pop_register(load_store, pc_lr, register_list),
            ThumbModeInstruction::MultipleLoadStore {
                load_store,
                base_register,
                register_list,
            } => self.multiple_load_store(load_store, base_register as usize, register_list),
            ThumbModeInstruction::CondBranch {
                condition,
                immediate_offset,
            } => self.cond_branch(condition, immediate_offset),
            ThumbModeInstruction::Swi => unimplemented!(),
            ThumbModeInstruction::UncondBranch { offset } => self.uncond_branch(offset),
            ThumbModeInstruction::LongBranchLink { h, offset } => self.long_branch_link(h, offset),
        };
    }

    pub fn step(&mut self) {
        match self.cpsr.cpu_state() {
            CpuState::Thumb => {
                if let Some(decoded) = self.decoded_thumb {
                    let current_ins = self.registers.program_counter() - 4;

                    log(format!("PC: 0x{current_ins:X} {decoded}"));

                    self.execute_thumb(decoded);
                }

                if !matches!(self.cpsr.cpu_state(), CpuState::Thumb) {
                    return;
                }

                self.decoded_thumb = self.fetched_thumb.map(Self::decode);
                self.fetched_thumb = Some(self.fetch_thumb());

                self.registers.set_program_counter(
                    self.registers.program_counter() as u32
                        + thumb::operations::SIZE_OF_INSTRUCTION,
                );
            }
            CpuState::Arm => {
                if let Some(decoded) = self.decoded_arm {
                    let current_ins = self.registers.program_counter() - 4;

                    log(format!("PC: 0x{current_ins:X} {decoded}"));

                    self.execute_arm(decoded);
                }

                if !matches!(self.cpsr.cpu_state(), CpuState::Arm) {
                    return;
                }

                self.decoded_arm = self.fetched_arm.map(Self::decode);
                self.fetched_arm = Some(self.fetch_arm());

                self.registers.set_program_counter(
                    self.registers.program_counter() as u32 + arm::operations::SIZE_OF_INSTRUCTION,
                );
            }
        }
    }

    pub fn new(memory: Arc<Mutex<InternalMemory>>) -> Self {
        Self {
            memory,
            ..Default::default()
        }
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
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
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
    use crate::cpu::condition::Condition;
    use crate::cpu::flags::{HalfwordDataTransferOffsetKind, Indexing, LoadStoreKind, Offsetting};
    use crate::cpu::registers::{REG_LR, REG_PROGRAM_COUNTER, REG_SP};
    use crate::cpu::thumb::instruction::ThumbModeInstruction;
    use crate::memory::io_device::IoDevice;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn check_branch() {
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
    fn check_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::default();

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute_arm(op_code);
    }

    #[test]
    fn check_block_data_transfer() {
        {
            // LDM with post-increment
            let op_code = 0b1110_100_0_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x1000, 1);
                memory.write_at(0x1004, 5);
                memory.write_at(0x1008, 7);
            }
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // LDM with pre-increment
            let op_code = 0b1110_100_1_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x1004, 1);
                memory.write_at(0x1008, 5);
                memory.write_at(0x100C, 7);
            }
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // LDM with post-decrement
            let op_code = 0b1110_100_0_0_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x1000, 7);
                memory.write_at(0x0FFC, 5);
                memory.write_at(0x0FF8, 1);
            }
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x0FF4);
        }
        {
            // LDM with pre-decrement
            let op_code = 0b1110_100_1_0_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x0FFC, 7);
                memory.write_at(0x0FF8, 5);
                memory.write_at(0x0FF4, 1);
            }
            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x0FF4);
        }
        {
            // STM with post-increment
            let op_code = 0b1110_100_0_1_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 1);
            assert_eq!(memory.read_at(0x1004), 5);
            assert_eq!(memory.read_at(0x1008), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // STM with pre-increment
            let op_code = 0b1110_100_1_1_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 0);
            assert_eq!(memory.read_at(0x1004), 1);
            assert_eq!(memory.read_at(0x1008), 5);
            assert_eq!(memory.read_at(0x100C), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // STM with post-decrement
            let op_code = 0b1110_100_0_0_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 7);
            assert_eq!(memory.read_at(0x0FFC), 5);
            assert_eq!(memory.read_at(0x0FF8), 1);
            assert_eq!(cpu.registers.register_at(13), 0x0FF4);
        }
        {
            // STM with pre-decrement and storing R15

            let op_code = 0b1110_100_1_0_0_1_0_1101_1000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 0);
            assert_eq!(memory.read_at(0x0FFC), 15 + 4);
            assert_eq!(memory.read_at(0x0FF8), 7);
            assert_eq!(memory.read_at(0x0FF4), 5);
            assert_eq!(memory.read_at(0x0FF0), 1);
            assert_eq!(cpu.registers.register_at(13), 0x0FF0);
        }
    }

    #[test]
    fn check_half_word_data_transfer() {
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
            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();
            assert_eq!(memory.read_at(0), 1);
            assert_eq!(memory.read_at(1), 1);
            // because we store halfword = 16bit
            assert_eq!(memory.read_at(2), 0);
            assert_eq!(memory.read_at(3), 0);
        }
        {
            // Immediate offset, pre-index, down, no wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_0_1_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory
                .lock()
                .unwrap()
                .write_word(100 - 0b11111, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), 100);
        }
        {
            // Immediate offset, pre-index, down, wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_1_1_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory
                .lock()
                .unwrap()
                .write_word(100 - 0b11111, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, pre-index, up, wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_1_1_1_1_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory
                .lock()
                .unwrap()
                .write_word(100 + 0b11111, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), 100 + 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_1_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory.lock().unwrap().write_word(100, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), 0x1234);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), load, signed byte
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_1_0000_0001_0001_1_10_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory.lock().unwrap().write_at(100, -5_i8 as u8);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), -5_i32 as u32);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), load, signed halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_1_0000_0001_0001_1_11_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory
                .lock()
                .unwrap()
                .write_half_word(100, -300_i16 as u16);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.registers.register_at(1), -300_i32 as u32);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), store, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_0_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 0xFFFF1234);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100), 0x1234);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, post-index, down, no wb (but implicit), store PC, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_1_0_0_0000_1111_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_program_counter(500);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100), 504);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, pre-index, down, no wb, store PC, unsigned halfword, base PC
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_0_0_1111_1111_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_program_counter(500);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(500 - 0b11111), 504);
            assert_eq!(cpu.registers.program_counter(), 500);
        }
        {
            // Register offset, post-index, down, no wb (but implicit), store PC, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_0_0_0_0000_1111_0000_1_01_1_0010;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_program_counter(500);
            cpu.registers.set_register_at(2, 0b11111);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100), 504);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
    }

    #[test]
    fn check_pc_relative_load() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b0100_1001_0101_1000_u16;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

        cpu.registers.set_register_at(1, 10);
        cpu.memory.lock().unwrap().write_at(352, 1);
        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.register_at(1), 1);
    }

    #[test]
    fn check_load_store_register_offset() {
        // Checks Store Word
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_00_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 100);
            cpu.registers.set_register_at(2, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(200), 0xFEEFAC1F);
        }
        // Checks Store Byte
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_01_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 100);
            cpu.registers.set_register_at(2, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_at(200), 0x1F);
            assert_eq!(cpu.memory.lock().unwrap().read_at(201), 0);
            assert_eq!(cpu.memory.lock().unwrap().read_at(202), 0);
            assert_eq!(cpu.memory.lock().unwrap().read_at(203), 0);
        }
        // Checks Load Word
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_10_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 100);
            cpu.memory.lock().unwrap().write_word(200, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(2), 0xFEEFAC1F);
        }
        // Checks Load Byte
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_11_0_000_001_010;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 100);
            cpu.memory.lock().unwrap().write_word(200, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(2), 0x1F);
        }
    }

    #[test]
    fn check_load_store_immediate_offset() {
        {
            // Store Word
            let op_code = 0b0110_0011_0111_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::LoadStoreImmOffset
            );

            cpu.registers.set_register_at(7, 2);
            cpu.registers.set_register_at(0, 0xFFFFFFFF);
            cpu.execute_thumb(op_code);

            let mem = cpu.memory.lock().unwrap();
            assert_eq!(mem.read_word(54), 0xFFFFFFFF);
        }
        {
            // Load Word
            let op_code = 0b0110_1011_0000_1111;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::LoadStoreImmOffset
            );
            {
                let mut mem = cpu.memory.lock().unwrap();
                mem.write_word(1048, 0xFFFFFFFF);
            }
            cpu.registers.set_register_at(1, 1000);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(7), 0xFFFFFFFF);
        }
        {
            // Store Byte
            let op_code = 0b0111_0010_0011_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::LoadStoreImmOffset
            );

            cpu.registers.set_register_at(7, 2);
            cpu.registers.set_register_at(0, 0xFFFFFFFF);
            cpu.execute_thumb(op_code);

            let mem = cpu.memory.lock().unwrap();
            assert_eq!(mem.read_at(10), 0xFF);
        }
    }

    #[test]
    fn check_add_subtract() {
        // Check sub
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b00011_1_1_111_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 0b110);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
            assert!(!cpu.cpsr.zero_flag());
            assert!(cpu.cpsr.carry_flag());
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
    fn check_cond_branch() {
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
    fn check_uncond_branch() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1110_0001_0010_1111;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

        cpu.registers.set_program_counter(1000);

        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.program_counter(), 1606);
    }

    #[test]
    fn check_hi_reg_operation_branch_ex() {
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
            assert!(!cpu.cpsr.carry_flag());
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
            assert!(cpu.cpsr.carry_flag());
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
            assert!(!cpu.cpsr.carry_flag());
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
    fn check_push_pop_register() {
        {
            // Store + save LR
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1011_0101_1111_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_program_counter(1000);
            cpu.registers.set_register_at(REG_LR, 1000);
            cpu.registers.set_register_at(REG_SP, 1000);

            for r in 0..8 {
                cpu.registers.set_register_at(r, r.try_into().unwrap());
            }

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(1000 - 4), 1000);
            assert_eq!(cpu.memory.lock().unwrap().read_word(1000 - 4 - 4), 7);
            assert_eq!(cpu.memory.lock().unwrap().read_word(1000 - 4 - 4 - 4), 6);
            assert_eq!(
                cpu.memory.lock().unwrap().read_word(1000 - 4 - 4 - 4 - 4),
                5
            );
            assert_eq!(
                cpu.memory
                    .lock()
                    .unwrap()
                    .read_word(1000 - 4 - 4 - 4 - 4 - 4),
                4
            );
        }
        {
            // Load + restore PC
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1011_1_10_1_1111_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, 1000);

            cpu.memory.lock().unwrap().write_word(1000, 100);
            cpu.memory.lock().unwrap().write_word(1004, 200);
            cpu.memory.lock().unwrap().write_word(1008, 300);
            cpu.memory.lock().unwrap().write_word(1012, 400);
            cpu.memory.lock().unwrap().write_word(1016, 500);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(4), 100);
            assert_eq!(cpu.registers.register_at(5), 200);
            assert_eq!(cpu.registers.register_at(6), 300);
            assert_eq!(cpu.registers.register_at(7), 400);
            assert_eq!(
                cpu.registers
                    .register_at(REG_PROGRAM_COUNTER.try_into().unwrap()),
                500
            );
            assert_eq!(cpu.registers.register_at(REG_SP), 1020);
        }
    }

    #[test]
    fn check_add_offset_sp() {
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
    fn check_sp_relative_load() {
        {
            // Load
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1001_1_000_00000111;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, 100);
            cpu.memory.lock().unwrap().write_word(100 + 0b11100, 999);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 999);
        }
        {
            // Store
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1001_0_000_00000111;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(REG_SP, 100);
            cpu.registers.set_register_at(0, 999);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100 + 0b11100), 999);
        }
    }

    #[test]
    fn check_alu_op() {
        {
            // mul
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_0110_0000;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.cpsr.set_sign_flag(true);
            cpu.cpsr.set_zero_flag(true);
            cpu.registers.set_register_at(0, 0xFFFFFFFF);
            cpu.registers.set_register_at(4, 1);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 0xFFFFFFFF);
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
    }

    #[test]
    fn check_long_branch_link() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1111_1000_0100_0000;
        let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ThumbModeInstruction::LongBranchLink {
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
    fn check_load_store_halfword() {
        {
            // Load
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_1_00001_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.memory.lock().unwrap().write_half_word(102, 0xFF);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 0xFF);
        }
        {
            // Store
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_0_00001_000_001;
            let op_code: ThumbModeOpcode = Arm7tdmi::decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 0xFF);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_half_word(102), 0xFF);
        }
    }

    #[test]
    fn check_load_store_sign_extend_byte_halfword() {
        struct Test {
            opcode: u16,
            expected_decode: ThumbModeInstruction,
            prepare_fn: Box<dyn Fn(&mut Arm7tdmi)>,
            check_fn: Box<dyn Fn(Arm7tdmi)>,
        }

        let cases = vec![
            Test {
                opcode: 0b0101_0_0_1_000_001_010,
                expected_decode: ThumbModeInstruction::LoadStoreSignExtByteHalfword {
                    h: false,
                    sign_extend_flag: false,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, 100);
                    cpu.registers.set_register_at(2, 0xF000_F0FF);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.memory.lock().unwrap().read_half_word(110), 0xF0FF);
                }),
            },
            Test {
                opcode: 0b0101_1_0_1_000_001_010,
                expected_decode: ThumbModeInstruction::LoadStoreSignExtByteHalfword {
                    h: true,
                    sign_extend_flag: false,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, 100);
                    cpu.memory.lock().unwrap().write_half_word(110, 0xF0FF);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(2), 0xF0FF);
                }),
            },
            Test {
                opcode: 0b0101_0_1_1_000_001_010,
                expected_decode: ThumbModeInstruction::LoadStoreSignExtByteHalfword {
                    h: false,
                    sign_extend_flag: true,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, 100);
                    cpu.memory.lock().unwrap().write_at(110, 0x80);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(2), 0xFFFF_FF80);
                }),
            },
            Test {
                opcode: 0b0101_1_1_1_000_001_010,
                expected_decode: ThumbModeInstruction::LoadStoreSignExtByteHalfword {
                    h: true,
                    sign_extend_flag: true,
                    offset_register: 0,
                    base_register: 1,
                    destination_register: 2,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(0, 10);
                    cpu.registers.set_register_at(1, 100);
                    cpu.memory.lock().unwrap().write_half_word(110, 0x8030);
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
    fn check_load_address() {
        struct Test {
            opcode: u16,
            expected_decode: ThumbModeInstruction,
            prepare_fn: Box<dyn Fn(&mut Arm7tdmi)>,
            check_fn: Box<dyn Fn(Arm7tdmi)>,
        }

        for case in [
            Test {
                opcode: 0b1010_1_001_00000010,
                expected_decode: ThumbModeInstruction::LoadAddress {
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
                expected_decode: ThumbModeInstruction::LoadAddress {
                    sp: false,
                    destination_register: 1,
                    offset: 8,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_program_counter(10);
                }),
                check_fn: Box::new(|cpu| {
                    let v = cpu.registers.register_at(1);
                    assert_eq!(v, 18);
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
    fn check_multiple_load_store() {
        struct Test {
            opcode: u16,
            expected_decode: ThumbModeInstruction,
            prepare_fn: Box<dyn Fn(&mut Arm7tdmi)>,
            check_fn: Box<dyn Fn(Arm7tdmi)>,
        }

        let cases = vec![
            Test {
                opcode: 0b1100_1_001_10100000,
                expected_decode: ThumbModeInstruction::MultipleLoadStore {
                    load_store: LoadStoreKind::Load,
                    base_register: 1,
                    register_list: 160,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(1, 100);
                    cpu.memory.lock().unwrap().write_word(100, 0xFF);
                    cpu.memory.lock().unwrap().write_word(104, 0xFF);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.registers.register_at(5), 0xFF);
                    assert_eq!(cpu.registers.register_at(7), 0xFF);
                    assert_eq!(cpu.registers.register_at(1), 108);
                }),
            },
            Test {
                opcode: 0b1100_0_001_10100000,
                expected_decode: ThumbModeInstruction::MultipleLoadStore {
                    load_store: LoadStoreKind::Store,
                    base_register: 1,
                    register_list: 160,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_register_at(1, 100);
                    cpu.registers.set_register_at(5, 10);
                    cpu.registers.set_register_at(7, 20);
                }),
                check_fn: Box::new(|cpu| {
                    assert_eq!(cpu.memory.lock().unwrap().read_word(100), 10);
                    assert_eq!(cpu.memory.lock().unwrap().read_word(104), 20);
                    assert_eq!(cpu.registers.register_at(1), 108);
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
