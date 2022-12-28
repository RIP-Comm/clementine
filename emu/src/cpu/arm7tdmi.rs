use std::convert::TryInto;
use std::ops::Mul;
use std::sync::{Arc, Mutex};

use logger::log;

use crate::bitwise::Bits;
use crate::cpu::cpu_modes::Mode;
use crate::cpu::instruction::{ArmModeInstruction, ThumbModeInstruction};
use crate::cpu::opcode::ArmModeOpcode;
use crate::cpu::psr::Psr;
use crate::cpu::register_bank::RegisterBank;
use crate::memory::internal_memory::InternalMemory;
use crate::memory::io_device::IoDevice;

use super::alu_instruction::{self, ThumbModeAluInstruction};
use super::flags::{Indexing, LoadStoreKind, Offsetting, OperandKind, ReadWriteKind};
use super::opcode::ThumbModeOpcode;
use super::psr::CpuState;
use super::registers::Registers;

pub const REG_SP: usize = 0xD;
pub const REG_LR: usize = 0xE;
pub const REG_PROGRAM_COUNTER: u32 = 0xF;
pub const SIZE_OF_ARM_INSTRUCTION: u32 = 4;
pub const SIZE_OF_THUMB_INSTRUCTION: u32 = 2;

pub struct Arm7tdmi {
    pub(crate) memory: Arc<Mutex<InternalMemory>>,

    pub cpsr: Psr,
    pub spsr: Psr,
    pub registers: Registers,

    pub register_bank: RegisterBank,
}

impl Default for Arm7tdmi {
    fn default() -> Self {
        let mut s = Self {
            memory: Arc::new(Mutex::new(InternalMemory::default())),
            cpsr: Psr::from(Mode::Supervisor), // FIXME: Starting as Supervisor? Not sure
            spsr: Psr::default(),
            registers: Registers::default(),
            register_bank: RegisterBank::default(),
        };

        // Setting ARM mode at startup
        s.cpsr.set_cpu_state(CpuState::Arm);

        s
    }
}

impl Arm7tdmi {
    pub fn fetch_arm(&self) -> u32 {
        self.memory
            .lock()
            .unwrap()
            .read_word(self.registers.program_counter())
    }

    pub fn fetch_thumb(&self) -> u16 {
        self.memory
            .lock()
            .unwrap()
            .read_half_word(self.registers.program_counter())
    }

    pub fn decode<T, V>(&self, op_code: V) -> T
    where
        T: std::fmt::Display + TryFrom<V>,
        <T as TryFrom<V>>::Error: std::fmt::Debug,
    {
        let code = T::try_from(op_code).unwrap();
        log(format!("{code}"));
        code
    }

    pub fn execute_arm(&mut self, op_code: ArmModeOpcode) {
        use ArmModeInstruction::*;
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        let bytes_to_advance = if !self.cpsr.can_execute(op_code.condition) {
            Some(SIZE_OF_ARM_INSTRUCTION)
        } else {
            match op_code.instruction {
                DataProcessing => self.data_processing(op_code),
                Multiply => todo!(),
                MultiplyLong => todo!(),
                SingleDataSwap => todo!(),
                BranchAndExchange => self.branch_and_exchange(op_code),
                HalfwordDataTransferRegisterOffset => self.data_transfer_register_offset(op_code),
                HalfwordDataTransferImmediateOffset => self.data_transfer_immediate_offset(op_code),
                SingleDataTransfer => self.single_data_transfer(op_code),
                Undefined => todo!(),
                BlockDataTransfer => self.block_data_transfer(op_code),
                Branch => self.branch(op_code),
                CoprocessorDataTransfer => self.coprocessor_data_transfer(op_code),
                CoprocessorDataOperation => todo!(),
                CoprocessorRegisterTrasfer => todo!(),
                SoftwareInterrupt => todo!(),
            }
        };

        self.registers
            .advance_program_counter(bytes_to_advance.unwrap_or(0));
    }

    pub fn execute_thumb(&mut self, op_code: ThumbModeOpcode) {
        use ThumbModeInstruction::*;
        let bytes_to_advance: Option<u32> = match op_code.instruction {
            MoveShiftedRegister => self.move_shifted_reg(op_code),
            AddSubtract => self.add_subtract(op_code),
            MoveCompareAddSubtractImm => self.move_compare_add_sub_imm(op_code),
            AluOp => self.alu_op(op_code),
            HiRegisterOpBX => self.hi_reg_operation_branch_ex(op_code),
            PCRelativeLoad => self.pc_relative_load(op_code),
            LoadStoreRegisterOffset => self.load_store_register_offset(op_code),
            LoadStoreSignExtByteHalfword => unimplemented!(),
            LoadStoreImmOffset => unimplemented!(),
            LoadStoreHalfword => self.load_store_halfword(op_code),
            SPRelativeLoadStore => self.sp_relative_load_store(op_code),
            LoadAddress => unimplemented!(),
            AddOffsetSP => self.add_offset_sp(op_code),
            PushPopReg => self.push_pop_register(op_code),
            MultipleLoadStore => unimplemented!(),
            CondBranch => self.cond_branch(op_code),
            Swi => unimplemented!(),
            UncondBranch => unimplemented!(),
            LongBranchLink => self.long_branch_link(op_code),
        };

        self.registers
            .advance_program_counter(bytes_to_advance.unwrap_or(0));
    }

    pub fn step(&mut self) {
        // We set pc lowest bits to 0. In ARM we set the 2 lsb to 0 because instructions are word aligned.
        // In THUMB we set only the lsb to 0 because instructions are halfword aligned.

        match self.cpsr.cpu_state() {
            CpuState::Thumb => {
                let mut pc = self.registers.program_counter() as u32;
                pc.set_bit_off(0);
                self.registers.set_program_counter(pc);

                let op = self.fetch_thumb();
                let op = self.decode(op);
                self.execute_thumb(op);
            }
            CpuState::Arm => {
                let mut pc = self.registers.program_counter() as u32;
                pc.set_bit_off(0);
                pc.set_bit_off(1);
                self.registers.set_program_counter(pc);

                let op = self.fetch_arm();
                let op = self.decode(op);
                self.execute_arm(op);
            }
        }
    }
}

impl Arm7tdmi {
    pub fn new(memory: Arc<Mutex<InternalMemory>>) -> Self {
        Self {
            memory,
            ..Default::default()
        }
    }

    pub fn get_spsr(&self) -> Psr {
        match self.cpsr.mode() {
            Mode::User | Mode::System => panic!("Trying to access a SPSR in either User or System state which do not have banked SPSR."),
            Mode::Fiq => self.register_bank.spsr_fiq,
            Mode::Irq => self.register_bank.spsr_irq,
            Mode::Abort => self.register_bank.spsr_abt,
            Mode::Supervisor => self.register_bank.spsr_svc,
            Mode::Undefined => self.register_bank.spsr_und
        }
    }

    fn load_store_halfword(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let load_store: LoadStoreKind = op_code.get_bit(11).into();
        let offset = op_code.get_bits(6..=10) << 1;
        let rb = op_code.get_bits(3..=5);
        let rb = self.registers.register_at(rb.try_into().unwrap());
        let rd: usize = op_code.get_bits(0..=2).try_into().unwrap();

        let address: usize = rb.wrapping_add(offset as u32).try_into().unwrap();

        let mut mem = self.memory.lock().unwrap();

        match load_store {
            LoadStoreKind::Load => {
                self.registers
                    .set_register_at(rd, mem.read_half_word(address) as u32);
            }
            LoadStoreKind::Store => {
                mem.write_half_word(address, self.registers.register_at(rd) as u16);
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn cond_branch(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        // Encodes the condition
        let cond = op_code.get_bits(8..=11) as u8;

        // 9 bits signed offset (assembler puts `label` >> 1 in this field so we should <<1)
        let offset = op_code.get_bits(0..=7) << 1;

        let mask = 1 << 8;
        let offset = (offset as i32 ^ mask) - mask;

        if self.cpsr.can_execute(cond.into()) {
            let pc = self.registers.program_counter() as i32;
            let new_pc = pc + 4 + offset;

            self.registers.set_program_counter(new_pc as u32);

            None
        } else {
            Some(SIZE_OF_THUMB_INSTRUCTION)
        }
    }

    fn add_subtract(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let i: OperandKind = op_code.get_bit(10).into();

        // 0 - Add, 1 - Sub
        let op = op_code.get_bit(9);

        let rn_offset3 = op_code.get_bits(6..=8);
        let rs = op_code.get_bits(3..=5);
        let rs = self.registers.register_at(rs.try_into().unwrap());

        let rd: usize = op_code.get_bits(0..=2).try_into().unwrap();

        let offset = match i {
            OperandKind::Immediate => rn_offset3 as u32,
            OperandKind::Register => self.registers.register_at(rn_offset3.try_into().unwrap()),
        };

        match op {
            // Add
            false => {
                let add_result = Self::add_inner_op(rs, offset);
                self.registers.set_register_at(rd, add_result.result);
                self.cpsr.set_flags(add_result);
            }
            // Sub
            true => {
                let sub_result = Self::sub_inner_op(rs, offset);
                self.registers.set_register_at(rd, sub_result.result);
                self.cpsr.set_flags(sub_result);
            }
        };

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn load_store_register_offset(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let load_store: LoadStoreKind = op_code.get_bit(11).into();
        let byte_word: ReadWriteKind = op_code.get_bit(10).into();
        let ro = op_code.get_bits(6..=8);
        let ro = self.registers.register_at(ro.try_into().unwrap());

        let rb = op_code.get_bits(3..=5);
        let rb = self.registers.register_at(rb.try_into().unwrap());

        let rd: usize = op_code.get_bits(0..=2).try_into().unwrap();

        let address: usize = rb.wrapping_add(ro).try_into().unwrap();

        let mut mem = self.memory.lock().unwrap();

        match (load_store, byte_word) {
            (LoadStoreKind::Store, ReadWriteKind::Byte) => {
                let rd = (self.registers.register_at(rd) & 0xFF) as u8;
                mem.write_at(address, rd);
            }
            (LoadStoreKind::Store, ReadWriteKind::Word) => {
                let rd = self.registers.register_at(rd);
                mem.write_word(address, rd);
            }
            (LoadStoreKind::Load, ReadWriteKind::Byte) => {
                let value = mem.read_at(address);
                self.registers.set_register_at(rd, value as u32);
            }
            (LoadStoreKind::Load, ReadWriteKind::Word) => {
                let value = mem.read_word(address);
                self.registers.set_register_at(rd, value);
            }
        };

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn branch_and_exchange(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let rn = op_code.get_bits(0..=3);
        let rn = self.registers.register_at(rn.try_into().unwrap());
        let state: CpuState = rn.get_bit(0).into();
        self.cpsr.set_cpu_state(state);
        self.registers.set_program_counter(rn);

        None
    }

    fn data_transfer_register_offset(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let indexing: Indexing = op_code.get_bit(24).into();
        let offsetting: Offsetting = op_code.get_bit(23).into();
        let _write_back = op_code.get_bit(21);
        let load_store = op_code.get_bit(20);
        let rn_base_register = op_code.get_bits(16..=19);
        let rd_source_destination_register = op_code.get_bits(12..=15);
        let transfer_type = HalfwordTransferType::from(op_code.get_bits(5..=6) as u8);
        let offset_register = op_code.get_bits(0..=3);

        let offset = self.registers.register_at(offset_register as usize);
        let mut address = self
            .registers
            .register_at(rn_base_register.try_into().unwrap());

        if rn_base_register == REG_PROGRAM_COUNTER {
            // prefetching
            let v: u32 = self.registers.program_counter().try_into().unwrap();
            address = address.wrapping_add(v + 8);
        }

        let effective = match offsetting {
            Offsetting::Down => address.wrapping_sub(offset),
            Offsetting::Up => address.wrapping_add(offset),
        };

        let address: usize = match indexing {
            Indexing::Pre => effective.try_into().unwrap(),
            Indexing::Post => address.try_into().unwrap(),
        };

        if load_store {
            todo!()
        } else {
            let value = if rd_source_destination_register == 0xF {
                let v: u32 = self.registers.program_counter().try_into().unwrap();
                v + 12
            } else {
                self.registers
                    .register_at(rd_source_destination_register.try_into().unwrap())
            };

            match transfer_type {
                HalfwordTransferType::UnsignedHalfwords => {
                    if let Ok(mut mem) = self.memory.lock() {
                        mem.write_at(address, value.get_bits(0..=7) as u8);
                        mem.write_at(address + 1, value.get_bits(8..=15) as u8);
                    }
                }
                _ => unreachable!("HS flags invalid for STORE (L=0)"),
            };
        }

        if indexing == Indexing::Post {
            todo!()
        }

        if !(load_store && rd_source_destination_register == REG_PROGRAM_COUNTER) {
            Some(SIZE_OF_ARM_INSTRUCTION)
        } else {
            None
        }
    }

    fn data_transfer_immediate_offset(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let indexing: Indexing = op_code.get_bit(24).into();
        let offsetting: Offsetting = op_code.get_bit(23).into();
        let _write_back = op_code.get_bit(21); // TODO: Handle write back.
        let load_store = op_code.get_bit(20);
        let rn_base_register = op_code.get_bits(16..=19);
        let rd_source_destination_register = op_code.get_bits(12..=15);
        let transfer_type = HalfwordTransferType::from(op_code.get_bits(5..=6) as u8);
        let immediate_offset_high = op_code.get_bits(8..=11);
        let immediate_offset_low = op_code.get_bits(0..=3);

        let offset = (immediate_offset_high << 4) | immediate_offset_low;
        let mut address = self
            .registers
            .register_at(rn_base_register.try_into().unwrap());

        if rn_base_register == REG_PROGRAM_COUNTER {
            // prefetching
            let v: u32 = self.registers.program_counter().try_into().unwrap();
            address = address.wrapping_add(v + 8);
        }

        let effective = match offsetting {
            Offsetting::Down => address.wrapping_sub(offset),
            Offsetting::Up => address.wrapping_add(offset),
        };

        let address: usize = match indexing {
            Indexing::Pre => effective.try_into().unwrap(),
            Indexing::Post => address.try_into().unwrap(), // TODO: ignore write back (should be 0 in this case but...)
        };

        if load_store {
            todo!("load from mem")
        } else {
            let value = if rd_source_destination_register == REG_PROGRAM_COUNTER {
                let pc: u32 = self.registers.program_counter().try_into().unwrap();
                pc + 12
            } else {
                self.registers
                    .register_at(rd_source_destination_register as usize)
            };

            match transfer_type {
                HalfwordTransferType::UnsignedHalfwords => {
                    if let Ok(mut mem) = self.memory.lock() {
                        mem.write_at(address, value.get_bits(0..=7) as u8);
                        mem.write_at(address + 1, value.get_bits(8..=15) as u8);
                    }
                }
                _ => unreachable!("HS flags can't be != from 01 for STORE (L=0)"),
            };
        }

        if indexing == Indexing::Post {
            // TODO: ignore write back (should be 0 in this case but...)
            todo!()
        }

        if !(load_store && rd_source_destination_register == REG_PROGRAM_COUNTER) {
            Some(SIZE_OF_ARM_INSTRUCTION)
        } else {
            None
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

    fn branch(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let offset = op_code.get_bits(0..=23) << 2;

        // We need to sign-extend the 26 bit number into a 32 bit.
        // We can't just do `offset as i32` since it would just do a
        // zero extension.

        let mask = 1 << 25;
        let offset = (offset as i32 ^ mask) - mask;

        let old_pc: u32 = self.registers.program_counter().try_into().unwrap();
        let is_link = op_code.get_bit(24);
        if is_link {
            self.registers
                .set_register_at(14, old_pc.wrapping_add(SIZE_OF_ARM_INSTRUCTION));
        }

        // 8 is for the prefetch
        let new_pc = self.registers.program_counter() as i32 + offset + 8;
        self.registers.set_program_counter(new_pc as u32);

        // Never advance PC after B
        None
    }

    fn block_data_transfer(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let indexing: Indexing = op_code.get_bit(24).into();
        let offsetting: Offsetting = op_code.get_bit(23).into();
        let s = op_code.get_bit(22);
        if s {
            todo!()
        }
        let write_back = op_code.get_bit(21);
        let load_store = op_code.get_bit(20);
        let rn = op_code.get_bits(16..=19);
        let reg_list = op_code.get_bits(0..=15);

        let memory_base = self.registers.register_at(rn.try_into().unwrap());
        let mut address = memory_base.try_into().unwrap();

        if load_store {
            let transfer = |arm: &mut Self, address: usize, reg_destination: usize| {
                let memory = arm.memory.lock().unwrap();

                let part_0: u32 = memory.read_at(address).try_into().unwrap();
                let part_1: u32 = memory.read_at(address + 1).try_into().unwrap();
                let part_2: u32 = memory.read_at(address + 2).try_into().unwrap();
                let part_3: u32 = memory.read_at(address + 3).try_into().unwrap();
                let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                arm.registers.set_register_at(reg_destination, v);
            };

            self.exec_data_trasfer(reg_list, indexing, &mut address, offsetting, transfer);
        } else {
            let transfer = |arm: &mut Self, address: usize, reg_source: usize| {
                let mut value = arm.registers.register_at(reg_source);

                // If R15 we get the value of the current instruction + 12
                if reg_source == REG_PROGRAM_COUNTER.try_into().unwrap() {
                    value += 12;
                }
                let mut memory = arm.memory.lock().unwrap();

                memory.write_at(address, value.get_bits(0..=7) as u8);
                memory.write_at(address + 1, value.get_bits(8..=15) as u8);
                memory.write_at(address + 2, value.get_bits(16..=23) as u8);
                memory.write_at(address + 3, value.get_bits(24..=31) as u8);
            };

            self.exec_data_trasfer(reg_list, indexing, &mut address, offsetting, transfer);
        }

        if write_back {
            self.registers
                .set_register_at(rn.try_into().unwrap(), address.try_into().unwrap());
        };

        // If LDM and R15 is in register list we don't advance PC
        if !(load_store && reg_list.is_bit_on(15)) {
            Some(SIZE_OF_ARM_INSTRUCTION)
        } else {
            None
        }
    }

    fn coprocessor_data_transfer(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let indexing: Indexing = op_code.get_bit(24).into();
        let offsetting: Offsetting = op_code.get_bit(23).into();
        let _transfer_len = op_code.get_bit(22);
        let _write_back = op_code.get_bit(21);
        let _load_store = op_code.get_bit(20);

        let rn_base_register = op_code.get_bits(16..=19);
        let _crd = op_code.get_bits(12..=15);
        let _cp_number = op_code.get_bits(8..=11);
        let offset = op_code.get_bits(0..=7);

        let mut _address = self
            .registers
            .register_at(rn_base_register.try_into().unwrap());

        let effective = match offsetting {
            Offsetting::Down => _address.wrapping_sub(offset),
            Offsetting::Up => _address.wrapping_add(offset),
        };

        let _address = match indexing {
            Indexing::Pre => effective,
            Indexing::Post => _address,
        };

        // TODO: take a look if we need to finish this for real.
        Some(SIZE_OF_ARM_INSTRUCTION)
    }

    fn exec_data_trasfer<F>(
        &mut self,
        reg_list: u32,
        indexing: Indexing,
        address: &mut usize,
        offsetting: Offsetting,
        trasfer: F,
    ) where
        F: Fn(&mut Self, usize, usize),
    {
        let alignment = 4; // Since are word, the alignment is 4.

        let change_address = |address: usize| match offsetting {
            Offsetting::Down => address.wrapping_sub(alignment),
            Offsetting::Up => address.wrapping_add(alignment),
        };

        // If we are decreasing we still want to store the lowest reg to the lowest
        // memory address. For this reason we reverse the loop order.
        let range_registers: Box<dyn Iterator<Item = u8>> = match offsetting {
            Offsetting::Down => Box::new((0..=15).rev()),
            Offsetting::Up => Box::new(0..=15),
        };

        for reg_source in range_registers {
            if reg_list.is_bit_on(reg_source) {
                if indexing == Indexing::Pre {
                    *address = change_address(*address);
                }

                trasfer(self, *address, reg_source.into());

                if indexing == Indexing::Post {
                    *address = change_address(*address);
                }
            }
        }
    }

    pub fn pc_relative_load(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let rd = op_code.get_bits(8..=10);
        let address = op_code.get_bits(0..=7) as usize;
        let mut pc = self.registers.program_counter() as u32;
        pc.set_bit_off(1);
        let address = pc as usize + 4_usize + (address << 2);
        let value = self.memory.lock().unwrap().read_word(address);
        self.registers
            .set_register_at(rd.try_into().unwrap(), value);

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    pub(crate) fn hi_reg_operation_branch_ex(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let op = op_code.get_bits(8..=9);
        let h1 = op_code.get_bit(7);
        let h2 = op_code.get_bit(6);
        let rs_hs = op_code.get_bits(3..=5);
        let rd_hd = op_code.get_bits(0..=2);

        if !h1 && !h2 && (op == 0b00 || op == 0b01 || op == 0b10) {
            panic!("H1=0 H2=0 is not supported with ADD, CMP, MOV");
        } else if h1 && op == 0b11 {
            panic!("H1=1 is not supported with BX")
        }

        let destination_register: usize = (h1 as u16 * 8 + rd_hd).try_into().unwrap();
        let source_register: usize = (h2 as u16 * 8 + rs_hs).try_into().unwrap();

        let first_op = self.registers.register_at(destination_register)
            + match destination_register as u32 {
                REG_PROGRAM_COUNTER => 4,
                _ => 0,
            };

        let second_op = self.registers.register_at(source_register)
            + match source_register as u32 {
                REG_PROGRAM_COUNTER => 4,
                _ => 0,
            };

        match op {
            // Add
            0b00 => {
                self.registers
                    .set_register_at(destination_register, first_op + second_op);

                if destination_register == REG_PROGRAM_COUNTER as usize {
                    None
                } else {
                    Some(SIZE_OF_THUMB_INSTRUCTION)
                }
            }
            // Cmp
            0b01 => {
                let sub_result = Self::sub_inner_op(first_op, second_op);

                self.cpsr.set_flags(sub_result);

                Some(SIZE_OF_THUMB_INSTRUCTION)
            }
            // Mov
            0b10 => {
                self.registers
                    .set_register_at(destination_register, second_op);

                if destination_register == REG_PROGRAM_COUNTER as usize {
                    None
                } else {
                    Some(SIZE_OF_THUMB_INSTRUCTION)
                }
            }
            // Bx
            0b11 => {
                self.cpsr.set_cpu_state(second_op.get_bit(0).into());

                self.registers.set_program_counter(second_op);

                None
            }
            _ => unreachable!(),
        }
    }

    pub(crate) fn push_pop_register(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let load_store: LoadStoreKind = op_code.get_bit(11).into();
        let store_or_not_reg_lr = op_code.get_bit(8);
        let rlist = op_code.get_bits(0..=7);
        let mut reg_sp = self.registers.register_at(REG_SP);

        let mut memory = self.memory.lock().unwrap();

        match load_store {
            LoadStoreKind::Store => {
                if store_or_not_reg_lr {
                    reg_sp -= 4;
                    memory.write_word(
                        reg_sp.try_into().unwrap(),
                        self.registers.register_at(REG_LR),
                    );
                }

                for r in (0..=7).rev() {
                    if rlist.get_bit(r) {
                        reg_sp -= 4;
                        memory.write_word(
                            reg_sp.try_into().unwrap(),
                            self.registers.register_at(r.into()),
                        );
                    }
                }
            }
            LoadStoreKind::Load => {
                for r in 0..=7 {
                    if rlist.get_bit(r) {
                        self.registers.set_register_at(
                            r.try_into().unwrap(),
                            memory.read_word(reg_sp.try_into().unwrap()),
                        );

                        reg_sp += 4;
                    }
                }

                if store_or_not_reg_lr {
                    self.registers
                        .set_program_counter(memory.read_word(reg_sp.try_into().unwrap()));

                    reg_sp += 4;
                }
            }
        }

        self.registers.set_register_at(REG_SP, reg_sp);

        if load_store == LoadStoreKind::Load && store_or_not_reg_lr {
            None
        } else {
            Some(SIZE_OF_THUMB_INSTRUCTION)
        }
    }

    fn add_offset_sp(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        // 0 - positive, 1 - negative
        let s = op_code.get_bit(7);
        let word7 = op_code.get_bits(0..=6);

        let value = ((word7 as i32) << 2).mul(if s { -1 } else { 1 });
        let old_sp = self.registers.register_at(REG_SP) as i32;
        let new_sp = old_sp + value;

        self.registers.set_register_at(REG_SP, new_sp as u32);

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn sp_relative_load_store(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let load_store: LoadStoreKind = op_code.get_bit(11).into();
        let rd: usize = op_code.get_bits(8..=10).try_into().unwrap();
        let word8 = op_code.get_bits(0..=7);

        let address = self.registers.register_at(REG_SP) + ((word8 as u32) << 2);

        match load_store {
            LoadStoreKind::Load => {
                self.registers.set_register_at(
                    rd,
                    self.memory
                        .lock()
                        .unwrap()
                        .read_word(address.try_into().unwrap()),
                );
            }
            LoadStoreKind::Store => {
                self.memory
                    .lock()
                    .unwrap()
                    .write_word(address.try_into().unwrap(), self.registers.register_at(rd));
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn alu_op(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let op: ThumbModeAluInstruction = op_code.get_bits(6..=9).into();
        let rs = op_code.get_bits(3..=5);
        let rs = self.registers.register_at(rs.try_into().unwrap());

        let rd = op_code.get_bits(0..=2);

        match op {
            ThumbModeAluInstruction::And => todo!(),
            ThumbModeAluInstruction::Eor => todo!(),
            ThumbModeAluInstruction::Lsl => todo!(),
            ThumbModeAluInstruction::Lsr => todo!(),
            ThumbModeAluInstruction::Asr => todo!(),
            ThumbModeAluInstruction::Adc => todo!(),
            ThumbModeAluInstruction::Sbc => todo!(),
            ThumbModeAluInstruction::Ror => todo!(),
            ThumbModeAluInstruction::Tst => self.tst(rd.into(), rs),
            ThumbModeAluInstruction::Neg => todo!(),
            ThumbModeAluInstruction::Cmp => todo!(),
            ThumbModeAluInstruction::Cmn => todo!(),
            ThumbModeAluInstruction::Orr => self.orr(
                rd.into(),
                self.registers.register_at(rd.try_into().unwrap()),
                rs,
                true,
            ),
            ThumbModeAluInstruction::Mul => todo!(),
            ThumbModeAluInstruction::Bic => todo!(),
            ThumbModeAluInstruction::Mvn => {
                self.mvn(rd.try_into().unwrap(), rs, true);
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn long_branch_link(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let h = op_code.get_bit(11);
        let offset = op_code.get_bits(0..=10) as u32;

        if h {
            let next_instruction =
                self.registers.program_counter() as u32 + SIZE_OF_THUMB_INSTRUCTION;
            let lr = self.registers.register_at(REG_LR);
            let offset = offset << 1;
            self.registers.set_program_counter(lr.wrapping_add(offset));
            self.registers.set_register_at(REG_LR, next_instruction | 1);
        } else {
            let offset = offset << 12;
            let mask = 1 << 22;
            let offset = (offset as i32 ^ mask) - mask;

            let pc = self.registers.program_counter() as u32
                + SIZE_OF_THUMB_INSTRUCTION
                + SIZE_OF_THUMB_INSTRUCTION;
            self.registers
                .set_register_at(REG_LR, ((pc as i32) + offset) as u32);
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    pub(crate) fn move_shifted_reg(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let op = op_code.get_bits(11..=12);
        let offset = op_code.get_bits(6..=10);
        let rs = op_code.get_bits(3..=5);
        let rd = op_code.get_bits(0..=2);
        let source = self.registers.register_at(rs.try_into().unwrap());

        let r = alu_instruction::shift(op.into(), offset.into(), source, self.cpsr.carry_flag());
        self.registers
            .set_register_at(rd.try_into().unwrap(), r.result);
        self.cpsr.set_flags(r);

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }
}

pub enum HalfwordTransferType {
    UnsignedHalfwords,
    SignedByte,
    SignedHalfwords,
}

impl From<u8> for HalfwordTransferType {
    fn from(value: u8) -> Self {
        match value.get_bits(0..=1) {
            0b01 => Self::UnsignedHalfwords,
            0b10 => Self::SignedByte,
            0b11 => Self::SignedHalfwords,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::condition::Condition;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_branch() {
        // Covers a positive offset

        // 15(1111b) << 2 = 60 bytes
        let op_code = 0b1110_1010_0000_0000_0000_0000_0000_1111;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.program_counter(), 68);

        // Covers a negative offset

        // -9 << 2 = -36 bytes
        let op_code = 0b1110_1010_1111_1111_1111_1111_1111_0111;
        let op_code = cpu.decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.program_counter(), 68 + 8 - 36);

        // Covers link

        let op_code = 0b1110_1011_0000_0000_0000_0000_0000_1111;
        let op_code = cpu.decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(14), 44);
    }

    #[test]
    #[should_panic]
    fn check_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::default();

        let op_code: ArmModeOpcode = cpu.decode(op_code);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute_arm(op_code);
    }

    #[test]
    fn check_block_data_transfer() {
        {
            // LDM with post-increment
            let op_code = 0b1110_100_0_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

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
            let op_code = cpu.decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 0);
            assert_eq!(memory.read_at(0x0FFC), 15 + 12);
            assert_eq!(memory.read_at(0x0FF8), 7);
            assert_eq!(memory.read_at(0x0FF4), 5);
            assert_eq!(memory.read_at(0x0FF0), 1);
            assert_eq!(cpu.registers.register_at(13), 0x0FF0);
        }
    }

    #[test]
    fn check_data_transfer_register_offset() {
        {
            let op_code = 0b1110_0001_1000_0010_0000_0000_1011_0001;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::HalfwordDataTransferRegisterOffset
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
    }

    #[test]
    fn check_data_transfer_immediate_offset() {
        {
            // Store halfword
            let op_code = 0b1110_0001_1100_0001_0000_0000_1011_0000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);

            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::HalfwordDataTransferImmediateOffset
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
    }

    #[test]
    fn check_pc_relative_load() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b0100_1001_0101_1000_u16;
        let op_code: ThumbModeOpcode = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ThumbModeInstruction::PCRelativeLoad);

        cpu.registers.set_register_at(1, 10);
        cpu.memory.lock().unwrap().write_at(356, 1);
        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.register_at(1), 1);
    }

    #[test]
    fn check_load_store_register_offset() {
        // Checks Store Word
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0101_00_0_000_001_010;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 100);
            cpu.memory.lock().unwrap().write_word(200, 0xFEEFAC1F);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(2), 0x1F);
        }
    }

    #[test]
    fn check_add_subtract() {
        // Check sub
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b00011_1_1_111_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
        let op_code: ThumbModeOpcode = cpu.decode(op_code);

        cpu.registers.set_program_counter(1000);

        cpu.execute_thumb(op_code);

        // Asserting no branch since condition is not satisfied
        assert_eq!(cpu.registers.program_counter(), 1002);

        cpu.cpsr.set_sign_flag(true);

        let op_code = 0b1101_1011_11111100;
        let op_code: ThumbModeOpcode = cpu.decode(op_code);
        cpu.execute_thumb(op_code);

        // Asserting branch now that we set the condition
        assert_eq!(cpu.registers.program_counter(), 1002 + 4 - 8);
    }

    #[test]
    fn check_hi_reg_operation_branch_ex() {
        {
            // BX Hs
            let mut cpu = Arm7tdmi::default();
            let op_code: u16 = 0b0100_0111_0111_0000;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::HiRegisterOpBX);

            cpu.registers.set_register_at(14, 123);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.program_counter(), 123);
        }
        {
            // Add Rd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_00_0_1_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(1, 10);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 20);
        }
        {
            // Add Hd, Rs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_00_1_0_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 10);
            cpu.registers.set_register_at(9, 10);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 20);
        }
        {
            // Add Hd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_00_1_1_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(9, 10);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 20);
        }
        {
            // Cmp Rd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_01_0_1_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(8, 10);
            cpu.registers.set_register_at(1, 11);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 10);
        }
        {
            // Mov Hd, Rs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_10_1_0_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 10);
            cpu.registers.set_register_at(9, 11);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(9), 10);
        }
        {
            // Mov Hd, Hs
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b010001_10_1_1_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::PushPopReg);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(REG_SP, 1000);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(REG_SP), 1000 + (7 << 2));
        }
        // Negative offset
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b10110000_1_0000111;
        let op_code: ThumbModeOpcode = cpu.decode(op_code);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(REG_SP, 100);
            cpu.memory.lock().unwrap().write_word(100 + 0b11100, 999);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(0), 999);
        }
        {
            // Store
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1001_0_000_00000111;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(REG_SP, 100);
            cpu.registers.set_register_at(0, 999);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100 + 0b11100), 999);
        }
    }

    #[test]
    fn check_alu_op() {
        {
            // tst
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0010_0011_1110;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::AluOp);

            cpu.execute_thumb(op_code);

            assert!(!cpu.cpsr.sign_flag());
            assert!(cpu.cpsr.zero_flag());
        }
        {
            // orr
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_0010_1010;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::AluOp);

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::AluOp);

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
        let op_code: ThumbModeOpcode = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ThumbModeInstruction::LongBranchLink);

        cpu.registers.set_program_counter(100);
        cpu.registers.set_register_at(REG_LR, 200);
        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.register_at(REG_LR), 103);
        assert_eq!(cpu.registers.program_counter(), 330);
    }

    #[test]
    fn check_load_store_halfword() {
        {
            // Load
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_1_00001_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::LoadStoreHalfword);

            cpu.registers.set_register_at(0, 100);
            cpu.memory.lock().unwrap().write_half_word(102, 0xFF);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.register_at(1), 0xFF);
        }
        {
            // Store
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_0_00001_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ThumbModeInstruction::LoadStoreHalfword);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_register_at(1, 0xFF);

            cpu.execute_thumb(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_half_word(102), 0xFF);
        }
    }
}
