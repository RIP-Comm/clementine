use std::convert::TryInto;
use std::ops::Mul;
use std::sync::{Arc, Mutex};

use logger::log;

use crate::bitwise::Bits;
use crate::cpu::alu_instruction::ShiftKind;
use crate::cpu::condition::Condition;
use crate::cpu::cpu_modes::Mode;
use crate::cpu::instruction::{ArmModeInstruction, ThumbModeInstruction};
use crate::cpu::move_compare_add_sub::ThumbHighRegisterOperation;
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

    pub disassembler_buffer: Vec<String>,
}

impl Default for Arm7tdmi {
    fn default() -> Self {
        let mut s = Self {
            memory: Arc::new(Mutex::new(InternalMemory::default())),
            cpsr: Psr::from(Mode::Supervisor), // FIXME: Starting as Supervisor? Not sure
            spsr: Psr::default(),
            registers: Registers::default(),
            register_bank: RegisterBank::default(),
            disassembler_buffer: vec![],
        };

        // Setting ARM mode at startup
        s.cpsr.set_cpu_state(CpuState::Arm);
        s.cpsr.set_irq_disable(true);
        s.cpsr.set_fiq_disable(true);

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
            let decimal_value = self.registers.program_counter();
            let padded_hex_value = format!("{:#04X}", decimal_value);
            self.disassembler_buffer.push(format!(
                "{}: {}",
                padded_hex_value,
                op_code.instruction.disassembler()
            ));
            match op_code.instruction {
                DataProcessing {
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
                Multiply => todo!(),
                MultiplyLong => todo!(),
                SingleDataSwap => todo!(),
                BranchAndExchange {
                    condition: _,
                    register,
                } => self.branch_and_exchange(register),
                HalfwordDataTransferRegisterOffset => self.half_word_data_transfer(op_code),
                HalfwordDataTransferImmediateOffset => self.half_word_data_transfer(op_code),
                SingleDataTransfer {
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
                Undefined => todo!(),
                BlockDataTransfer {
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
                Branch {
                    condition: _,
                    link,
                    offset,
                } => self.branch(link, offset),
                CoprocessorDataTransfer {
                    condition: _,
                    indexing,
                    offsetting,
                    transfer_length,
                    write_back,
                    load_store,
                    rn,
                    crd,
                    cp_number,
                    offset,
                } => self.coprocessor_data_transfer(
                    indexing,
                    offsetting,
                    transfer_length,
                    write_back,
                    load_store,
                    rn,
                    crd,
                    cp_number,
                    offset,
                ),
                CoprocessorDataOperation => todo!(),
                CoprocessorRegisterTrasfer => todo!(),
                SoftwareInterrupt => todo!(),
            }
        };

        self.registers
            .advance_program_counter(bytes_to_advance.unwrap_or(0));
    }

    pub fn execute_thumb(&mut self, op_code: ThumbModeOpcode) {
        let decimal_value = self.registers.program_counter();
        let padded_hex_value = format!("{:#04X}", decimal_value);
        self.disassembler_buffer.push(format!(
            "{}: {}",
            padded_hex_value,
            op_code.instruction.disassembler()
        ));
        use ThumbModeInstruction::*;
        let bytes_to_advance: Option<u32> = match op_code.instruction {
            MoveShiftedRegister {
                op,
                offset5,
                rs,
                rd,
            } => self.move_shifted_reg(op, offset5, rs, rd),
            AddSubtract {
                operation_kind,
                op,
                rn_offset3,
                rs,
                rd,
            } => self.add_subtract(operation_kind, op, rn_offset3, rs, rd),
            MoveCompareAddSubtractImm {
                op,
                r_destination,
                offset,
            } => self.move_compare_add_sub_imm(op, r_destination, offset),
            AluOp { op, rs, rd } => self.alu_op(op, rs, rd),
            HiRegisterOpBX {
                op,
                source_register,
                destination_register,
            } => self.hi_reg_operation_branch_ex(op, source_register, destination_register),
            PCRelativeLoad {
                r_destination,
                immediate_value,
            } => self.pc_relative_load(r_destination, immediate_value),
            LoadStoreRegisterOffset {
                load_store,
                byte_word,
                ro,
                rb,
                rd,
            } => self.load_store_register_offset(load_store, byte_word, ro, rb, rd),
            LoadStoreSignExtByteHalfword {
                h_flag,
                sign_extend_flag,
                r_offset,
                r_base,
                r_destination,
            } => self.load_store_sign_extend_byte_halfword(
                h_flag,
                sign_extend_flag,
                r_offset,
                r_base,
                r_destination,
            ),
            LoadStoreImmOffset => self.load_store_immediate_offset(op_code),
            LoadStoreHalfword {
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
            SPRelativeLoadStore {
                load_store,
                r_destination,
                word8,
            } => self.sp_relative_load_store(load_store, r_destination, word8),
            LoadAddress {
                sp,
                r_destination,
                offset,
            } => self.load_address(sp, r_destination.try_into().unwrap(), offset),
            AddOffsetSP { s, word7 } => self.add_offset_sp(s, word7),
            PushPopReg {
                load_store,
                pc_lr,
                register_list,
            } => self.push_pop_register(load_store, pc_lr, register_list),
            MultipleLoadStore {
                load_store,
                base_register,
                register_list,
            } => self.multiple_load_store(load_store, base_register as usize, register_list),
            CondBranch {
                condition,
                immediate_offset,
            } => self.cond_branch(condition, immediate_offset),
            Swi => unimplemented!(),
            UncondBranch { offset } => self.uncond_branch(offset),
            LongBranchLink { h, offset } => self.long_branch_link(h, offset),
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

    fn load_store_sign_extend_byte_halfword(
        &mut self,
        h_flag: bool,
        sign_extend_flag: bool,
        r_offset: u32,
        r_base: u32,
        r_destination: u32,
    ) -> Option<u32> {
        let offset = self.registers.register_at(r_offset.try_into().unwrap());
        let base = self.registers.register_at(r_base.try_into().unwrap());
        let address: usize = base.wrapping_add(offset).try_into().unwrap();

        match (sign_extend_flag, h_flag) {
            // Store halfword
            (false, false) => {
                let value = self
                    .registers
                    .register_at(r_destination.try_into().unwrap());

                self.memory
                    .lock()
                    .unwrap()
                    .write_half_word(address, value as u16);
            }
            // Load halfword
            (false, true) => {
                let value = self.memory.lock().unwrap().read_half_word(address);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value as u32);
            }
            // Load sign-extended byte
            (true, false) => {
                let mut value = self.memory.lock().unwrap().read_at(address) as u32;
                value = value.sign_extended(8);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value);
            }
            // Load sign-extended halfword
            (true, true) => {
                let mut value = self.memory.lock().unwrap().read_half_word(address) as u32;
                value = value.sign_extended(16);

                self.registers
                    .set_register_at(r_destination.try_into().unwrap(), value);
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn load_store_halfword(
        &mut self,
        load_store: LoadStoreKind,
        offset: u16,
        base_register: u16,
        source_destination_register: u16,
    ) -> Option<u32> {
        let rb = self
            .registers
            .register_at(base_register.try_into().unwrap());
        let address: usize = rb.wrapping_add(offset as u32).try_into().unwrap();
        let mut mem = self.memory.lock().unwrap();

        match load_store {
            LoadStoreKind::Load => {
                self.registers.set_register_at(
                    source_destination_register as usize,
                    mem.read_half_word(address) as u32,
                );
            }
            LoadStoreKind::Store => {
                mem.write_half_word(
                    address,
                    self.registers
                        .register_at(source_destination_register as usize)
                        as u16,
                );
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn cond_branch(&mut self, condition: Condition, immediate_offset: i32) -> Option<u32> {
        if self.cpsr.can_execute(condition) {
            let pc = self.registers.program_counter() as i32;
            let new_pc = pc + 4 + immediate_offset;
            self.registers.set_program_counter(new_pc as u32);
            log("cond branch can execute");
            None
        } else {
            Some(SIZE_OF_THUMB_INSTRUCTION)
        }
    }

    fn uncond_branch(&mut self, offset: u32) -> Option<u32> {
        let offset = offset.sign_extended(12) as i32;
        let pc = self.registers.program_counter() as i32 + 4; // NOTE: Emulating prefetch with this +4.
        let new_pc = pc + offset;
        self.registers.set_program_counter(new_pc as u32);

        None
    }

    fn add_subtract(
        &mut self,
        operation_kind: OperandKind,
        op: bool,
        rn_offset3: u16,
        rs: u16,
        rd: u16,
    ) -> Option<u32> {
        let rs = self.registers.register_at(rs.try_into().unwrap());
        let offset = match operation_kind {
            OperandKind::Immediate => rn_offset3 as u32,
            OperandKind::Register => self.registers.register_at(rn_offset3.try_into().unwrap()),
        };

        match op {
            // Add
            false => {
                let add_result = Self::add_inner_op(rs, offset);
                self.registers
                    .set_register_at(rd as usize, add_result.result);
                self.cpsr.set_flags(add_result);
            }
            // Sub
            true => {
                let sub_result = Self::sub_inner_op(rs, offset);
                self.registers
                    .set_register_at(rd as usize, sub_result.result);
                self.cpsr.set_flags(sub_result);
            }
        };

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn load_store_register_offset(
        &mut self,
        load_store: LoadStoreKind,
        byte_word: ReadWriteKind,
        offset_register: u16,
        base_register: u16,
        source_desitination_register: u16,
    ) -> Option<u32> {
        let ro = self
            .registers
            .register_at(offset_register.try_into().unwrap());
        let rb = self
            .registers
            .register_at(base_register.try_into().unwrap());
        let address: usize = rb.wrapping_add(ro).try_into().unwrap();
        let mut mem = self.memory.lock().unwrap();
        let rd: usize = source_desitination_register.try_into().unwrap();
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

    fn load_store_immediate_offset(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let byte_word: ReadWriteKind = op_code.get_bit(12).into();
        let load_store: LoadStoreKind = op_code.get_bit(11).into();
        let offset5 = op_code.get_bits(6..=10) as u32;
        let offset = offset5
            << match byte_word {
                ReadWriteKind::Word => 2,
                ReadWriteKind::Byte => 0,
            };

        let rb = op_code.get_bits(3..=5);
        let rd = op_code.get_bits(0..=2);

        let base = self.registers.register_at(rb.try_into().unwrap());
        let address = base.wrapping_add(offset);

        let mut mem = self.memory.lock().unwrap();
        match (load_store, byte_word) {
            (LoadStoreKind::Store, ReadWriteKind::Word) => {
                let v = self.registers.register_at(rd.try_into().unwrap());
                mem.write_word(address.try_into().unwrap(), v)
            }
            (LoadStoreKind::Store, ReadWriteKind::Byte) => {
                let v = self.registers.register_at(rd.try_into().unwrap());
                mem.write_at(address.try_into().unwrap(), v as u8)
            }
            (LoadStoreKind::Load, ReadWriteKind::Word) => {
                let v = mem.read_word(address.try_into().unwrap());
                self.registers.set_register_at(rd.try_into().unwrap(), v);
            }
            (LoadStoreKind::Load, ReadWriteKind::Byte) => todo!(),
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn branch_and_exchange(&mut self, register: usize) -> Option<u32> {
        let mut rn = self.registers.register_at(register);
        let state: CpuState = rn.get_bit(0).into();
        self.cpsr.set_cpu_state(state);

        match self.cpsr.cpu_state() {
            CpuState::Thumb => rn.set_bit_off(0),
            CpuState::Arm => {
                rn.set_bit_off(0);
                rn.set_bit_off(1);
            }
        }

        self.registers.set_program_counter(rn);

        None
    }

    fn half_word_data_transfer(&mut self, op_code: ArmModeOpcode) -> Option<u32> {
        let indexing: Indexing = op_code.get_bit(24).into();
        let offsetting: Offsetting = op_code.get_bit(23).into();
        let write_back = op_code.get_bit(21);
        let load_store: LoadStoreKind = op_code.get_bit(20).into();
        let rn_base_register = op_code.get_bits(16..=19);
        let rd_source_destination_register = op_code.get_bits(12..=15);
        let transfer_type = HalfwordTransferType::from(op_code.get_bits(5..=6) as u8);

        let operand_kind: OperandKind = op_code.get_bit(22).into();

        let offset = match operand_kind {
            OperandKind::Immediate => {
                let immediate_offset_high = op_code.get_bits(8..=11);
                let immediate_offset_low = op_code.get_bits(0..=3);
                (immediate_offset_high << 4) | immediate_offset_low
            }
            OperandKind::Register => {
                let rm: usize = op_code.get_bits(0..=3).try_into().unwrap();
                self.registers.register_at(rm)
            }
        };

        let mut address = self
            .registers
            .register_at(rn_base_register.try_into().unwrap());

        if rn_base_register == REG_PROGRAM_COUNTER {
            // prefetching
            address = address.wrapping_add(8);

            if write_back {
                panic!("WriteBack should not be specified when using R15 as base register.");
            }

            if indexing == Indexing::Post {
                panic!("Post indexing uses write back but we're using R15 as base register.
                Documentation says that when using R15 as base register WB should not be used. What should we do?");
            }
        }

        let effective = match offsetting {
            Offsetting::Down => address.wrapping_sub(offset),
            Offsetting::Up => address.wrapping_add(offset),
        };

        let address: usize = match indexing {
            Indexing::Pre => effective.try_into().unwrap(),
            Indexing::Post => address.try_into().unwrap(),
        };

        let mut mem = self.memory.lock().unwrap();

        match load_store {
            LoadStoreKind::Store => {
                let value = if rd_source_destination_register == REG_PROGRAM_COUNTER {
                    let pc: u32 = self.registers.program_counter().try_into().unwrap();
                    pc + 12
                } else {
                    self.registers
                        .register_at(rd_source_destination_register as usize)
                };

                match transfer_type {
                    HalfwordTransferType::UnsignedHalfwords => {
                        mem.write_at(address, value.get_bits(0..=7) as u8);
                        mem.write_at(address + 1, value.get_bits(8..=15) as u8);
                    }
                    _ => unreachable!("HS flags can't be != from 01 for STORE (L=0)"),
                };
            }
            LoadStoreKind::Load => match transfer_type {
                HalfwordTransferType::UnsignedHalfwords => {
                    let v = mem.read_half_word(address);
                    self.registers
                        .set_register_at(rd_source_destination_register as usize, v.into());
                }
                HalfwordTransferType::SignedByte => {
                    let v = mem.read_at(address) as u32;
                    self.registers.set_register_at(
                        rd_source_destination_register as usize,
                        v.sign_extended(8),
                    );
                }
                HalfwordTransferType::SignedHalfwords => {
                    let v = mem.read_half_word(address) as u32;
                    self.registers.set_register_at(
                        rd_source_destination_register as usize,
                        v.sign_extended(16),
                    );
                }
            },
        }

        if indexing == Indexing::Post || write_back {
            self.registers
                .set_register_at(rn_base_register.try_into().unwrap(), effective);
        }

        if !(load_store == LoadStoreKind::Load
            && rd_source_destination_register == REG_PROGRAM_COUNTER)
        {
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

    fn branch(&mut self, is_link: bool, offset: u32) -> Option<u32> {
        let offset = offset.sign_extended(26) as i32;
        let old_pc: u32 = self.registers.program_counter().try_into().unwrap();
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

    #[allow(clippy::too_many_arguments)]
    fn block_data_transfer(
        &mut self,
        indexing: Indexing,
        offsetting: Offsetting,
        load_psr: bool,
        write_back: bool,
        load_store: LoadStoreKind,
        rn: u32,
        reg_list: u32,
    ) -> Option<u32> {
        let base_register = rn.try_into().unwrap();
        let memory_base = self.registers.register_at(base_register);
        let mut address = memory_base.try_into().unwrap();

        if load_psr {
            unimplemented!();
        }

        let transfer = match load_store {
            LoadStoreKind::Store => {
                |arm: &mut Self, address: usize, reg_source: usize| {
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
                }
            }
            LoadStoreKind::Load => |arm: &mut Self, address: usize, reg_destination: usize| {
                let memory = arm.memory.lock().unwrap();
                let part_0: u32 = memory.read_at(address).try_into().unwrap();
                let part_1: u32 = memory.read_at(address + 1).try_into().unwrap();
                let part_2: u32 = memory.read_at(address + 2).try_into().unwrap();
                let part_3: u32 = memory.read_at(address + 3).try_into().unwrap();
                drop(memory);
                let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                arm.registers.set_register_at(reg_destination, v);
            },
        };

        self.exec_data_transfer(reg_list, indexing, &mut address, offsetting, transfer);

        if write_back {
            self.registers
                .set_register_at(base_register, address.try_into().unwrap());
        };

        // If LDM and R15 is in register list we don't advance PC
        if !(load_store == LoadStoreKind::Load && reg_list.is_bit_on(15)) {
            Some(SIZE_OF_ARM_INSTRUCTION)
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn coprocessor_data_transfer(
        &mut self,
        indexing: Indexing,
        offsetting: Offsetting,
        _transfer_length: bool,
        _write_back: bool,
        _load_store: LoadStoreKind,
        rn: u32,
        _crd: u32,
        _cp_number: u32,
        offset: u32,
    ) -> Option<u32> {
        let mut _address = self.registers.register_at(rn.try_into().unwrap());
        let effective = match offsetting {
            Offsetting::Down => _address.wrapping_sub(offset),
            Offsetting::Up => _address.wrapping_add(offset),
        };

        let _address = match indexing {
            Indexing::Pre => effective,
            Indexing::Post => _address,
        };

        // TODO: take a look if we need to finish this for real.
        todo!("finish this");
        // Some(SIZE_OF_ARM_INSTRUCTION)
    }

    fn exec_data_transfer<F>(
        &mut self,
        reg_list: u32,
        indexing: Indexing,
        address: &mut usize,
        offsetting: Offsetting,
        transfer: F,
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

                transfer(self, *address, reg_source.into());

                if indexing == Indexing::Post {
                    *address = change_address(*address);
                }
            }
        }
    }

    pub fn pc_relative_load(&mut self, r_destination: u16, immediate_value: u16) -> Option<u32> {
        let mut pc = self.registers.program_counter() as u32;
        // word alignment
        pc.set_bit_off(1);
        pc.set_bit_off(0);
        let address = pc as usize + 4_usize + immediate_value as usize;
        let value = self.memory.lock().unwrap().read_word(address);
        let dest = r_destination.try_into().unwrap();
        self.registers.set_register_at(dest, value);

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    pub(crate) fn hi_reg_operation_branch_ex(
        &mut self,
        op: ThumbHighRegisterOperation,
        reg_source: u16,
        reg_destination: u16,
    ) -> Option<u32> {
        let d_value = self.registers.register_at(reg_destination as usize);
        let s_value = self.registers.register_at(reg_source as usize);

        match op {
            ThumbHighRegisterOperation::Add => {
                let r = d_value.wrapping_add(s_value);
                self.registers.set_register_at(reg_destination as usize, r);

                if reg_destination == REG_PROGRAM_COUNTER as u16 {
                    self.registers.set_program_counter(r + 4);
                    None
                } else {
                    Some(SIZE_OF_THUMB_INSTRUCTION)
                }
            }
            ThumbHighRegisterOperation::Cmp => {
                let first_op = d_value
                    + match reg_destination as u32 {
                        REG_PROGRAM_COUNTER => 4,
                        _ => 0,
                    };

                let second_op = s_value
                    + match reg_source as u32 {
                        REG_PROGRAM_COUNTER => 4,
                        _ => 0,
                    };

                let sub_result = Self::sub_inner_op(first_op, second_op);

                self.cpsr.set_flags(sub_result);

                Some(SIZE_OF_THUMB_INSTRUCTION)
            }
            ThumbHighRegisterOperation::Mov => {
                let second_op = s_value
                    + match reg_source as u32 {
                        REG_PROGRAM_COUNTER => 4,
                        _ => 0,
                    };

                self.registers
                    .set_register_at(reg_destination as usize, second_op);

                if reg_destination == REG_PROGRAM_COUNTER as u16 {
                    None
                } else {
                    Some(SIZE_OF_THUMB_INSTRUCTION)
                }
            }
            ThumbHighRegisterOperation::BxOrBlx => {
                let value = s_value
                    + match reg_source as u32 {
                        REG_PROGRAM_COUNTER => 4,
                        _ => 0,
                    };
                let new_state = value.get_bit(0);
                self.cpsr.set_cpu_state(new_state.into());
                let new_pc = value & !1;
                self.registers.set_program_counter(new_pc);

                None
            }
        }
    }

    pub(crate) fn push_pop_register(
        &mut self,
        load_store: LoadStoreKind,
        pc_lr: bool,
        register_list: u16,
    ) -> Option<u32> {
        let mut reg_sp = self.registers.register_at(REG_SP);
        let mut memory = self.memory.lock().unwrap();

        match load_store {
            LoadStoreKind::Store => {
                if pc_lr {
                    reg_sp -= 4;
                    memory.write_word(
                        reg_sp.try_into().unwrap(),
                        self.registers.register_at(REG_LR),
                    );
                }

                for r in (0..=7).rev() {
                    if register_list.get_bit(r) {
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
                    if register_list.get_bit(r) {
                        self.registers.set_register_at(
                            r.try_into().unwrap(),
                            memory.read_word(reg_sp.try_into().unwrap()),
                        );

                        reg_sp += 4;
                    }
                }

                if pc_lr {
                    self.registers
                        .set_program_counter(memory.read_word(reg_sp.try_into().unwrap()));

                    reg_sp += 4;
                }
            }
        }

        self.registers.set_register_at(REG_SP, reg_sp);

        if load_store == LoadStoreKind::Load && pc_lr {
            None
        } else {
            Some(SIZE_OF_THUMB_INSTRUCTION)
        }
    }

    pub(crate) fn multiple_load_store(
        &mut self,
        load_store: LoadStoreKind,
        base_register: usize,
        register_list: u16,
    ) -> Option<u32> {
        match load_store {
            LoadStoreKind::Store => unimplemented!("multiple store"),
            LoadStoreKind::Load => {
                let base_address = self.registers.register_at(base_register);
                let mut address = base_address;
                for r in 0..=7 {
                    if register_list.get_bit(r) {
                        let value = self.memory.lock().unwrap().read_word(address as usize);
                        self.registers.set_register_at(r as usize, value);
                        address += 4;
                    }
                }

                self.registers.set_register_at(base_register, address);
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn add_offset_sp(&mut self, s: bool, word7: u16) -> Option<u32> {
        let value = (word7 as i32).mul(if s { -1 } else { 1 });
        let old_sp = self.registers.register_at(REG_SP) as i32;
        let new_sp = old_sp + value;

        self.registers.set_register_at(REG_SP, new_sp as u32);

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn sp_relative_load_store(
        &mut self,
        load_store: LoadStoreKind,
        r_destination: u16,
        word8: u16,
    ) -> Option<u32> {
        let address = self.registers.register_at(REG_SP) + (word8 as u32);

        let rd = r_destination.try_into().unwrap();
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

    fn load_address(&mut self, sp: bool, r_destination: usize, offset: u32) -> Option<u32> {
        let v = if sp {
            let stack_pointer = self.registers.register_at(REG_SP);
            stack_pointer.wrapping_add(offset)
        } else {
            let pc = self.registers.program_counter() as u32;
            let mut pc = pc.wrapping_add(4);
            pc.set_bit_off(0);
            pc.wrapping_add(offset)
        };

        self.registers.set_register_at(r_destination, v);
        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn alu_op(&mut self, op: ThumbModeAluInstruction, rs: u16, rd: u16) -> Option<u32> {
        let rs = self.registers.register_at(rs.try_into().unwrap());
        match op {
            ThumbModeAluInstruction::And => self.and(
                rd.into(),
                self.registers.register_at(rd.try_into().unwrap()),
                rs,
                true,
            ),
            ThumbModeAluInstruction::Eor => todo!(),
            ThumbModeAluInstruction::Lsl => todo!(),
            ThumbModeAluInstruction::Lsr => todo!(),
            ThumbModeAluInstruction::Asr => todo!(),
            ThumbModeAluInstruction::Adc => todo!(),
            ThumbModeAluInstruction::Sbc => todo!(),
            ThumbModeAluInstruction::Ror => todo!(),
            ThumbModeAluInstruction::Tst => {
                self.tst(self.registers.register_at(rd.try_into().unwrap()), rs)
            }
            ThumbModeAluInstruction::Neg => todo!(),
            ThumbModeAluInstruction::Cmp => {
                self.cmp(self.registers.register_at(rd.try_into().unwrap()), rs)
            }
            ThumbModeAluInstruction::Cmn => todo!(),
            ThumbModeAluInstruction::Orr => self.orr(
                rd.into(),
                self.registers.register_at(rd.try_into().unwrap()),
                rs,
                true,
            ),
            ThumbModeAluInstruction::Mul => self.mul(
                rd.into(),
                rs,
                self.registers.register_at(rd.try_into().unwrap()),
            ),
            ThumbModeAluInstruction::Bic => todo!(),
            ThumbModeAluInstruction::Mvn => {
                self.mvn(rd.try_into().unwrap(), rs, true);
            }
        }

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }

    fn long_branch_link(&mut self, h: bool, offset: u32) -> Option<u32> {
        if h {
            let offset = offset << 1;

            let next_instruction =
                self.registers.program_counter() as u32 + SIZE_OF_THUMB_INSTRUCTION;
            let lr = self.registers.register_at(REG_LR);

            self.registers.set_program_counter(lr.wrapping_add(offset));
            self.registers.set_register_at(REG_LR, next_instruction | 1);
            None
        } else {
            let offset = offset << 12;
            let offset = offset.sign_extended(23);

            let pc = self.registers.program_counter() as u32
                + SIZE_OF_THUMB_INSTRUCTION
                + SIZE_OF_THUMB_INSTRUCTION;
            self.registers
                .set_register_at(REG_LR, pc.wrapping_add(offset));
            Some(SIZE_OF_THUMB_INSTRUCTION)
        }
    }

    pub(crate) fn move_shifted_reg(
        &mut self,
        op: ShiftKind,
        offset5: u16,
        rs: u16,
        rd: u16,
    ) -> Option<u32> {
        let source = self.registers.register_at(rs.try_into().unwrap());
        let r = alu_instruction::shift(op, offset5.into(), source, self.cpsr.carry_flag());
        self.registers
            .set_register_at(rd.try_into().unwrap(), r.result);

        self.cpsr.set_carry_flag(r.carry);
        self.cpsr.set_zero_flag(r.result == 0);
        self.cpsr.set_sign_flag(r.result.get_bit(31));

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
    use crate::cpu::instruction::ArmModeInstruction::{Branch, BranchAndExchange};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn check_branch_and_exchange() {
        {
            let op_code = 0b1110_0_0_0_1_0_0_1_0_1_1_1_1_1_1_1_1_1_1_1_1_0_0_0_1_0000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                BranchAndExchange {
                    condition: Condition::AL,
                    register: 0
                }
            );
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "BX R0");
        }
    }

    #[test]
    fn check_branch() {
        {
            let op_code = 0b0000_101_0_111111111111111111100011;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                Branch {
                    condition: Condition::EQ,
                    link: false,
                    offset: 0x3FFFF8C,
                }
            );

            assert!(!cpu.cpsr.can_execute(op_code.condition));
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "BEQ 0x03FFFF8C");
        }
        {
            let op_code = 0b1110_101_1_000000000000000000001111;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                Branch {
                    condition: Condition::AL,
                    link: true,
                    offset: 60,
                }
            );
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "BL 0x0000003C");
        }

        // Covers a positive offset

        // 15(1111b) << 2 = 60 bytes
        let op_code = 0b1110_1010_0000_0000_0000_0000_0000_1111;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = cpu.decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.program_counter(), 68);

        // Covers a negative offset

        // -9 << 2 = -36 bytes
        let op_code = 0b1110_1010_1111_1111_1111_1111_1111_0111;
        let op_code: ArmModeOpcode = cpu.decode(op_code);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.program_counter(), 68 + 8 - 36);

        // Covers link

        let op_code = 0b1110_1011_0000_0000_0000_0000_0000_1111;
        let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
    fn check_half_word_data_transfer() {
        {
            // Register offset
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
        {
            // Immediate offset, pre-index, down, no wb, load, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_0_1_0000_0001_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

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
            let op_code: ArmModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_program_counter(500);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100), 512);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
        {
            // Immediate offset, pre-index, down, no wb, store PC, unsigned halfword, base PC
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_1_0_1_0_0_1111_1111_0001_1_01_1_1111;
            let op_code: ArmModeOpcode = cpu.decode(op_code);

            cpu.registers.set_program_counter(500);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(500 + 8 - 0b11111), 512);
            assert_eq!(cpu.registers.program_counter(), 504);
        }
        {
            // Register offset, post-index, down, no wb (but implicit), store PC, unsigned halfword
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_000_0_0_0_0_0_0000_1111_0000_1_01_1_0010;
            let op_code: ArmModeOpcode = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 100);
            cpu.registers.set_program_counter(500);
            cpu.registers.set_register_at(2, 0b11111);

            cpu.execute_arm(op_code);

            assert_eq!(cpu.memory.lock().unwrap().read_word(100), 512);
            assert_eq!(cpu.registers.register_at(0), 100 - 0b11111);
        }
    }

    #[test]
    fn check_pc_relative_load() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b0100_1001_0101_1000_u16;
        let op_code: ThumbModeOpcode = cpu.decode(op_code);
        assert_eq!(
            op_code.instruction,
            ThumbModeInstruction::PCRelativeLoad {
                r_destination: 1,
                immediate_value: 352,
            }
        );

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

            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::LoadStoreRegisterOffset {
                    load_store: LoadStoreKind::Store,
                    byte_word: Default::default(),
                    ro: 0,
                    rb: 1,
                    rd: 2,
                }
            );
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
    fn check_load_store_immediate_offset() {
        {
            // Store Word
            let op_code = 0b0110_0011_0111_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
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
    fn check_uncond_branch() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1110_0001_0010_1111;
        let op_code: ThumbModeOpcode = cpu.decode(op_code);
        assert_eq!(
            op_code.instruction,
            ThumbModeInstruction::UncondBranch { offset: 606 }
        );

        cpu.registers.set_program_counter(1000);

        cpu.execute_thumb(op_code);

        assert_eq!(cpu.registers.program_counter(), 1610);
    }

    #[test]
    fn check_hi_reg_operation_branch_ex() {
        {
            // BX Hs
            let mut cpu = Arm7tdmi::default();
            let op_code: u16 = 0b0100_0111_0111_0000;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::HiRegisterOpBX {
                    op: ThumbHighRegisterOperation::BxOrBlx,
                    source_register: 14,
                    destination_register: 0,
                }
            );
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::HiRegisterOpBX {
                    op: ThumbHighRegisterOperation::BxOrBlx,
                    source_register: 14,
                    destination_register: 0,
                }
            );

            cpu.registers.set_register_at(14, 123);
            cpu.execute_thumb(op_code);

            assert_eq!(cpu.registers.program_counter(), 122);
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
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::PushPopReg {
                    load_store: LoadStoreKind::Store,
                    pc_lr: true,
                    register_list: 240,
                }
            );

            assert_eq!(
                op_code.instruction.disassembler(),
                "PUSH {R4, R5, R6, R7, PC}"
            );

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
            // mul
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_0110_0000;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::AluOp {
                    op: ThumbModeAluInstruction::Mul,
                    rs: 4,
                    rd: 0,
                }
            );

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::AluOp {
                    op: ThumbModeAluInstruction::And,
                    rs: 3,
                    rd: 0,
                }
            );

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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::AluOp {
                    op: ThumbModeAluInstruction::Tst,
                    rs: 7,
                    rd: 6,
                }
            );

            cpu.execute_thumb(op_code);

            assert!(!cpu.cpsr.sign_flag());
            assert!(cpu.cpsr.zero_flag());
        }
        {
            // orr
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b0100_0011_0010_1010;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::AluOp {
                    op: ThumbModeAluInstruction::Orr,
                    rs: 5,
                    rd: 2,
                }
            );

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
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::AluOp {
                    op: ThumbModeAluInstruction::Mvn,
                    rs: 1,
                    rd: 7,
                }
            );

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

        assert_eq!(cpu.registers.register_at(REG_LR), 103);
        assert_eq!(cpu.registers.program_counter(), 328);
    }

    #[test]
    fn check_load_store_halfword() {
        {
            // Load
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1000_1_00001_000_001;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::LoadStoreHalfword {
                    load_store: LoadStoreKind::Load,
                    offset: 2,
                    base_register: 0,
                    source_destination_register: 1,
                }
            );

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
            assert_eq!(
                op_code.instruction,
                ThumbModeInstruction::LoadStoreHalfword {
                    load_store: LoadStoreKind::Store,
                    offset: 2,
                    base_register: 0,
                    source_destination_register: 1,
                }
            );

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
                    h_flag: false,
                    sign_extend_flag: false,
                    r_offset: 0,
                    r_base: 1,
                    r_destination: 2,
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
                    h_flag: true,
                    sign_extend_flag: false,
                    r_offset: 0,
                    r_base: 1,
                    r_destination: 2,
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
                    h_flag: false,
                    sign_extend_flag: true,
                    r_offset: 0,
                    r_base: 1,
                    r_destination: 2,
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
                    h_flag: true,
                    sign_extend_flag: true,
                    r_offset: 0,
                    r_base: 1,
                    r_destination: 2,
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
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
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
                    r_destination: 1,
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
                    r_destination: 1,
                    offset: 8,
                },
                prepare_fn: Box::new(|cpu| {
                    cpu.registers.set_program_counter(10);
                }),
                check_fn: Box::new(|cpu| {
                    let v = cpu.registers.register_at(1);
                    assert_eq!(v, 22);
                }),
            },
        ] {
            let mut cpu = Arm7tdmi::default();
            let op_code = case.opcode;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
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

        for case in [Test {
            opcode: 0b1100_1_001_10100000,
            expected_decode: ThumbModeInstruction::MultipleLoadStore {
                load_store: LoadStoreKind::Load,
                base_register: 1,
                register_list: 160,
            },
            prepare_fn: Box::new(|cpu| {
                cpu.registers.set_register_at(1, 100);
                cpu.memory.lock().unwrap().write_at(100, 0xFF);
                cpu.memory.lock().unwrap().write_at(104, 0xFF);
            }),
            check_fn: Box::new(|cpu| {
                assert_eq!(cpu.registers.register_at(5), 0xFF);
                assert_eq!(cpu.registers.register_at(7), 0xFF);
                assert_eq!(cpu.registers.register_at(1), 108);
            }),
        }] {
            let mut cpu = Arm7tdmi::default();
            let op_code = case.opcode;
            let op_code: ThumbModeOpcode = cpu.decode(op_code);
            assert_eq!(op_code.instruction, case.expected_decode);

            (*case.prepare_fn)(&mut cpu);

            cpu.execute_thumb(op_code);

            (*case.check_fn)(cpu);
        }
    }
}
