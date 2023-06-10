use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::{
    shift, AluInstructionKind, ArithmeticOpResult, ArmModeAluInstruction, Kind, PsrOpKind,
};
use crate::cpu::arm::instructions::{
    ArmModeMultiplyLongVariant, ArmModeMultiplyVariant, SingleDataTransferKind,
    SingleDataTransferOffsetInfo,
};
use crate::cpu::arm::mode::ArmModeOpcode;
use crate::cpu::arm7tdmi::{Arm7tdmi, HalfwordTransferKind};
use crate::cpu::cpu_modes::Mode;
use crate::cpu::flags::{
    HalfwordDataTransferOffsetKind, Indexing, LoadStoreKind, Offsetting, OperandKind,
    ReadWriteKind, ShiftKind,
};
use crate::cpu::psr::CpuState;
use crate::cpu::registers::REG_PROGRAM_COUNTER;
use crate::memory::io_device::IoDevice;
use logger::log;

use super::alu_instruction::{AluSecondOperandInfo, PsrKind};

pub const SIZE_OF_INSTRUCTION: u32 = 4;

impl Arm7tdmi {
    pub fn data_processing(
        &mut self,
        op_code: ArmModeOpcode, // FIXME: This parameter will be remove after change `psr_transfer`.
        alu_instruction: ArmModeAluInstruction,
        set_conditions: bool,
        op_kind: OperandKind,
        rn: u32,
        destination: u32,
    ) {
        let offset = match rn {
            // if Rn is R15(PC) we need to offset its value because of
            // instruction pipelining
            REG_PROGRAM_COUNTER => self.get_pc_offset_alu(op_kind, op_code.get_bit(4)),
            _ => 0,
        };
        let op1 = self.registers.register_at(rn.try_into().unwrap()) + offset;

        let op2 = self.get_operand(
            alu_instruction,
            set_conditions,
            op_kind,
            op_code.get_bits(0..=11),
        );
        // S = 1 and Rd = 0xF should not be allowed in User Mode.
        // TODO: When in other modes it should load SPSR_<current_mode> into CPSR
        if set_conditions && destination == REG_PROGRAM_COUNTER {
            unimplemented!("Implement cases when S=1 and Rd=0xF");
        }

        use ArmModeAluInstruction::*;
        match alu_instruction {
            And => self.and(destination.try_into().unwrap(), op1, op2, set_conditions),
            Eor => self.eor(destination.try_into().unwrap(), op1, op2, set_conditions),
            Sub => self.sub(destination.try_into().unwrap(), op1, op2, set_conditions),
            Rsb => self.rsb(destination.try_into().unwrap(), op1, op2, set_conditions),
            Add => self.add(destination.try_into().unwrap(), op1, op2, set_conditions),
            Adc => self.adc(destination.try_into().unwrap(), op1, op2, set_conditions),
            Sbc => self.sbc(destination.try_into().unwrap(), op1, op2, set_conditions),
            Rsc => self.rsc(destination.try_into().unwrap(), op1, op2, set_conditions),
            Tst => self.tst(op1, op2),
            Teq => self.teq(op1, op2),
            Cmp => self.cmp(op1, op2),
            Cmn => self.cmn(op1, op2),
            Orr => self.orr(destination.try_into().unwrap(), op1, op2, set_conditions),
            Mov => self.mov(destination.try_into().unwrap(), op2, set_conditions),
            Bic => self.bic(destination.try_into().unwrap(), op1, op2, set_conditions),
            Mvn => self.mvn(destination.try_into().unwrap(), op2, set_conditions),
        };

        // Test instructions do not modify destination so we don't flush pipeline even if
        // destination == R15
        if !matches!(alu_instruction, Teq | Cmn | Cmp | Tst) && destination == REG_PROGRAM_COUNTER {
            self.flush_pipeline();
        }
    }

    pub fn psr_transfer(&mut self, op_kind: PsrOpKind, psr_kind: PsrKind) {
        if matches!(self.cpsr.mode(), Mode::System | Mode::User) && psr_kind == PsrKind::Spsr {
            panic!("Can't access SPSR in System/User mode")
        }

        match op_kind {
            PsrOpKind::Mrs {
                destination_register,
            } => {
                if destination_register == REG_PROGRAM_COUNTER {
                    panic!("PSR transfer should not use R15 as source/destination");
                }

                let psr = match psr_kind {
                    PsrKind::Cpsr => self.cpsr,
                    PsrKind::Spsr => self.spsr,
                };

                self.registers
                    .set_register_at(destination_register.try_into().unwrap(), psr.into());
            }
            PsrOpKind::Msr { source_register } => {
                if source_register == REG_PROGRAM_COUNTER {
                    panic!("PSR transfer should not use R15 as source/destination");
                }

                let rm = self
                    .registers
                    .register_at(source_register.try_into().unwrap());

                let current_mode = self.cpsr.mode();

                {
                    let psr = match psr_kind {
                        PsrKind::Cpsr => &mut self.cpsr,
                        PsrKind::Spsr => &mut self.spsr,
                    };

                    // Setting flags
                    psr.set_sign_flag(rm.get_bit(31));
                    psr.set_zero_flag(rm.get_bit(30));
                    psr.set_carry_flag(rm.get_bit(29));
                    psr.set_overflow_flag(rm.get_bit(28));

                    // In User mode we can only set the flags so we don't touch the other bits
                    if current_mode != Mode::User {
                        psr.set_irq_disable(rm.get_bit(7));
                        psr.set_fiq_disable(rm.get_bit(6));

                        // Documentation says that software should never touch T (state) bit
                        // Should we set it? I guess software are written in order to not switch this bit
                        // but who knows?
                        if psr.state_bit() != rm.get_bit(5) {
                            log("WARNING: Changing state bit (arm/thumb) in MSR instruction. This should not happen.")
                        }
                        psr.set_state_bit(rm.get_bit(5));
                    }
                }

                // If we're modifying CPSR we need to be sure we're not in User mode.
                // Since in User mode we can only modify flags.
                if psr_kind == PsrKind::Cpsr && self.cpsr.mode() != Mode::User {
                    self.swap_mode(rm.get_bits(0..=4).try_into().unwrap());
                } else if psr_kind == PsrKind::Spsr {
                    // If we're modifying SPSR we're sure we're not in System|User (checked before)
                    // We use `set_mode_raw` since the BIOS sometimes writes 0 in the SPSR.
                    self.spsr.set_mode_raw(rm.get_bits(0..=4));
                }
            }
            PsrOpKind::MsrFlg { operand } => {
                let op = match operand {
                    AluSecondOperandInfo::Register {
                        shift_op: _,
                        shift_kind: _,
                        register,
                    } => self.registers.register_at(register.try_into().unwrap()),
                    AluSecondOperandInfo::Immediate { base, shift } => base.rotate_right(shift),
                };

                let psr = match psr_kind {
                    PsrKind::Cpsr => &mut self.cpsr,
                    PsrKind::Spsr => &mut self.spsr,
                };

                // Setting flags
                psr.set_sign_flag(op.get_bit(31));
                psr.set_zero_flag(op.get_bit(30));
                psr.set_carry_flag(op.get_bit(29));
                psr.set_overflow_flag(op.get_bit(28));
            }
        }
    }

    pub fn shift_operand(
        &mut self,
        alu_instruction: ArmModeAluInstruction,
        s: bool,
        shift_kind: ShiftKind,
        shift_amount: u32,
        rm: u32,
    ) -> u32 {
        let result = shift(shift_kind, shift_amount, rm, self.cpsr.carry_flag());

        // If the instruction is a logical ALU instruction and S is set we set the carry flag
        if alu_instruction.kind() == AluInstructionKind::Logical && s {
            self.cpsr.set_carry_flag(result.carry);
        }

        result.result
    }

    pub fn get_operand(
        &mut self,
        alu_instruction: ArmModeAluInstruction,
        s: bool,
        i: OperandKind,
        op2: u32,
    ) -> u32 {
        match i {
            // we get the operand from a register and then we shift it
            OperandKind::Register => {
                // bits [0-3] 2nd Operand Register (R0..R15) (including PC=R15)
                let rm = op2.get_bits(0..=3);
                // bit [4] - is Shift by Register Flag (0=Immediate, 1=Register)
                let r = op2.get_bit(4);
                let offset = match rm {
                    // if Rm is R15(PC) we need to offset its value because of
                    // instruction pipelining
                    REG_PROGRAM_COUNTER => self.get_pc_offset_alu(i, r),
                    _ => 0,
                };
                let rm = self.registers.register_at(rm.try_into().unwrap()) + offset;
                let shift_kind = op2.get_bits(5..=6).into();

                let shift_amount = match r {
                    // the shift amount is in the instruction
                    false => {
                        // bits [7-11] - Shift amount
                        op2.get_bits(7..=11)
                    }
                    // the shift amount is read from Rs
                    true => {
                        // bits [11-8] - Shift register (R0-R14) - only lower 8bit 0-255 used
                        let rs = op2.get_bits(8..=11);

                        let rs = self.registers.register_at(rs.try_into().unwrap()) & 0xFF;

                        // If shift is taken from register and the value is 0 Rm is directly used as operand
                        if rs == 0 {
                            return rm;
                        }

                        rs
                    }
                };

                self.shift_operand(alu_instruction, s, shift_kind, shift_amount, rm)
            }
            OperandKind::Immediate => {
                // bits [7-0] are the immediate value
                let imm = op2.get_bits(0..=7);
                // bit [11-8] are the rotate amount
                let rotate_amount = op2.get_bits(8..=11);

                imm.rotate_right(rotate_amount * 2)
            }
        }
    }

    /// Returns the offset that has to be applied to the value read by `PC`
    /// in the case of data processing (ALU) instruction.
    ///
    /// This is needed because when the instruction at address `X` is executing,
    /// PC points to `X+8` because of pipelining. If we need to read the shift
    /// amount from register (`i` is `False` and `r` is `True`) the instruction
    /// takes an additional cycle, thus `PC` points to `X+12`.
    ///
    /// PC is by default at `X+8` when executing the instruction `X` so we return 4 or 0
    ///
    /// # Arguments
    ///
    /// * `i` - A boolean value representing whether the 2nd operand is immediate or not
    /// * `r` - A boolean value representing whether the shift amount is to be taken from register or not
    pub(crate) fn get_pc_offset_alu(&self, i: OperandKind, r: bool) -> u32 {
        if i == OperandKind::Register && r {
            4
        } else {
            0
        }
    }

    pub fn and(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn & op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(31));
        }
    }

    /// Rotate Right rd by the value in rs, store the result in rd and set condition codes.
    pub fn ror(&mut self, rd: usize, rs: u32) {
        let rs = rs & 0xFF;
        let rd_value = self.registers.register_at(rd);
        if rs != 0 {
            let r = shift(ShiftKind::Ror, rs, rd_value, self.cpsr.carry_flag());
            self.registers.set_register_at(rd, r.result);
            self.cpsr.set_zero_flag(r.result == 0);
            self.cpsr.set_sign_flag(r.result.get_bit(31));
            self.cpsr.set_carry_flag(r.carry);
        } else {
            self.cpsr.set_zero_flag(rd_value == 0);
            self.cpsr.set_sign_flag(rd_value.get_bit(31));
        }
    }

    fn adc(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let carry: u32 = self.cpsr.carry_flag().into();

        let first_op_result = Self::add_inner_op(rn, op2);
        let second_op_result = Self::add_inner_op(first_op_result.result, carry);

        let result_op = ArithmeticOpResult {
            result: second_op_result.result,
            carry: first_op_result.carry || second_op_result.carry,
            overflow: first_op_result.overflow || second_op_result.overflow,
            sign: second_op_result.sign,
            zero: second_op_result.zero,
        };

        self.registers.set_register_at(rd, result_op.result);

        if s {
            self.cpsr.set_flags(result_op);
        }
    }

    fn sbc(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let carry: u32 = self.cpsr.carry_flag().into();

        let first_op_result = Self::sub_inner_op(rn, op2);
        let second_op_result = Self::add_inner_op(first_op_result.result, carry);
        let third_op_result = Self::sub_inner_op(second_op_result.result, 1);

        let result = ArithmeticOpResult {
            result: third_op_result.result,
            carry: first_op_result.carry || second_op_result.carry || third_op_result.carry,
            overflow: first_op_result.overflow
                || second_op_result.overflow
                || third_op_result.overflow,
            sign: third_op_result.sign,
            zero: third_op_result.zero,
        };

        self.registers.set_register_at(rd, result.result);

        if s {
            self.cpsr.set_flags(result);
        }
    }

    fn rsc(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        self.sbc(rd, op2, rn, s);
    }

    pub fn eor(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn ^ op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(31));
        }
    }

    fn sub(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let sub_result = Self::sub_inner_op(rn, op2);

        self.registers.set_register_at(rd, sub_result.result);

        if s {
            self.cpsr.set_flags(sub_result);
        }
    }

    fn rsb(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        self.sub(rd, op2, rn, s);
    }

    pub fn add_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
        // we do the sum in 64bits so that the 32nd bit is the carry
        let result_and_carry = (first_op as u64).wrapping_add(second_op as u64);
        let result = result_and_carry as u32;

        let sign_op1 = first_op.get_bit(31);
        let sign_op2 = second_op.get_bit(31);
        let sign_r = result.get_bit(31);

        let carry = (result_and_carry & 0x100000000) >> 32 == 1;

        // overflow only occurs when operands have the same sign and result has the opposite one
        let same_sign = sign_op1 == sign_op2;

        ArithmeticOpResult {
            result,
            carry,
            overflow: same_sign && (sign_op1 != sign_r),
            sign: result.get_bit(31),
            zero: result == 0,
        }
    }

    pub fn sub_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
        let result = first_op.wrapping_sub(second_op);

        let sign_op1 = first_op.get_bit(31);
        let sign_op2 = second_op.get_bit(31);
        let sign_r = result.get_bit(31);

        let different_sign = sign_op1 != sign_op2;

        ArithmeticOpResult {
            result,
            carry: first_op < second_op,
            overflow: different_sign && sign_op2 == sign_r,
            sign: result.get_bit(31),
            zero: result == 0,
        }
    }

    fn add(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let add_result = Self::add_inner_op(rn, op2);

        self.registers.set_register_at(rd, add_result.result);

        if s {
            self.cpsr.set_flags(add_result);
        }
    }

    pub fn tst(&mut self, rn: u32, op2: u32) {
        let value = rn & op2;

        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    /// Subtract the contents of rs from zero, and store the result in rd.
    pub fn neg(&mut self, rd: usize, rs: u32) {
        self.rsb(rd, rs, 0, true);
    }

    fn teq(&mut self, rn: u32, op2: u32) {
        let value = rn ^ op2;
        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    pub fn cmp(&mut self, rn: u32, op2: u32) {
        let sub_result = Self::sub_inner_op(rn, op2);

        self.cpsr.set_flags(sub_result);
    }

    fn cmn(&mut self, rn: u32, op2: u32) {
        let add_result = Self::add_inner_op(rn, op2);

        self.cpsr.set_flags(add_result);
    }

    pub fn orr(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn | op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.is_bit_on(31));
        }
    }

    fn mov(&mut self, rd: usize, op2: u32, s: bool) {
        self.registers.set_register_at(rd, op2);

        if s {
            self.cpsr.set_zero_flag(op2 == 0);
            self.cpsr.set_sign_flag(op2.get_bit(31));
        }
    }

    fn bic(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn & !op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_sign_flag(result.get_bit(31));
            self.cpsr.set_zero_flag(result == 0);
        }
    }

    pub fn mvn(&mut self, rd: usize, op2: u32, s: bool) {
        let result = !op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_sign_flag(result.get_bit(31));
            self.cpsr.set_zero_flag(result == 0);
        }
    }

    pub fn branch_and_exchange(&mut self, register: usize) {
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

        self.flush_pipeline();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn half_word_data_transfer(
        &mut self,
        indexing: Indexing,
        offsetting: Offsetting,
        write_back: bool,
        load_store_kind: LoadStoreKind,
        offset_kind: HalfwordDataTransferOffsetKind,
        base_register: u32,
        source_destination_register: u32,
        transfer_kind: HalfwordTransferKind,
    ) {
        let offset = match offset_kind {
            HalfwordDataTransferOffsetKind::Immediate { offset } => offset,
            HalfwordDataTransferOffsetKind::Register { register } => {
                self.registers.register_at(register as usize)
            }
        };

        let address = self
            .registers
            .register_at(base_register.try_into().unwrap());

        if base_register == REG_PROGRAM_COUNTER {
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

        let mut bus = self.bus.lock().unwrap();

        match load_store_kind {
            LoadStoreKind::Store => {
                let value = if source_destination_register == REG_PROGRAM_COUNTER {
                    let pc: u32 = self.registers.program_counter().try_into().unwrap();
                    pc + 4
                } else {
                    self.registers
                        .register_at(source_destination_register as usize)
                };

                match transfer_kind {
                    HalfwordTransferKind::UnsignedHalfwords => {
                        bus.write_half_word(address, value as u16);
                    }
                    _ => unreachable!("HS flags can't be != from 01 for STORE (L=0)"),
                };
            }
            LoadStoreKind::Load => match transfer_kind {
                HalfwordTransferKind::UnsignedHalfwords => {
                    let v = bus.read_half_word(address);
                    self.registers
                        .set_register_at(source_destination_register as usize, v.into());
                }
                HalfwordTransferKind::SignedByte => {
                    let v = bus.read_at(address) as u32;
                    self.registers
                        .set_register_at(source_destination_register as usize, v.sign_extended(8));
                }
                HalfwordTransferKind::SignedHalfwords => {
                    let v = bus.read_half_word(address) as u32;
                    self.registers
                        .set_register_at(source_destination_register as usize, v.sign_extended(16));
                }
            },
        }

        drop(bus);

        if indexing == Indexing::Post || write_back {
            self.registers
                .set_register_at(base_register.try_into().unwrap(), effective);
        }

        if load_store_kind == LoadStoreKind::Load
            && source_destination_register == REG_PROGRAM_COUNTER
        {
            self.flush_pipeline();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn single_data_transfer(
        &mut self,
        kind: SingleDataTransferKind,
        quantity: ReadWriteKind,
        write_back: bool,
        indexing: Indexing,
        rd: u32,
        base_register: u32,
        offset_info: SingleDataTransferOffsetInfo,
        offsetting: Offsetting,
    ) {
        let address = self
            .registers
            .register_at(base_register.try_into().unwrap());

        let amount = match offset_info {
            SingleDataTransferOffsetInfo::Immediate { offset } => offset,
            SingleDataTransferOffsetInfo::RegisterImmediate {
                shift_amount,
                shift_kind,
                reg_offset,
            } => {
                let v = self.registers.register_at(reg_offset.try_into().unwrap());
                let r = shift(shift_kind, shift_amount, v, self.cpsr.carry_flag());
                r.result
            }
        };

        let offset_address = match offsetting {
            Offsetting::Down => address.wrapping_sub(amount),
            Offsetting::Up => address.wrapping_add(amount),
        };

        let address = match indexing {
            Indexing::Post => {
                // write back is always true when using post indexing
                self.registers
                    .set_register_at(base_register as usize, offset_address);
                address as usize
            }
            Indexing::Pre => {
                if write_back {
                    self.registers
                        .set_register_at(offset_address as usize, base_register);
                }
                offset_address as usize
            }
        };

        match kind {
            SingleDataTransferKind::Ldr => match quantity {
                ReadWriteKind::Byte => {
                    let value = self.bus.lock().unwrap().read_at(address) as u32;
                    self.registers
                        .set_register_at(rd.try_into().unwrap(), value)
                }
                ReadWriteKind::Word => {
                    let bus = self.bus.lock().unwrap();
                    let mem = bus.internal_memory.lock().unwrap();
                    let part_0: u32 = mem.read_at(address).try_into().unwrap();
                    let part_1: u32 = mem.read_at(address + 1).try_into().unwrap();
                    let part_2: u32 = mem.read_at(address + 2).try_into().unwrap();
                    let part_3: u32 = mem.read_at(address + 3).try_into().unwrap();
                    drop(mem);
                    drop(bus);
                    let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                    self.registers.set_register_at(rd.try_into().unwrap(), v);
                }
            },
            SingleDataTransferKind::Str => match quantity {
                ReadWriteKind::Byte => {
                    let mut v = self.registers.register_at(rd.try_into().unwrap());

                    // If R15 we get the value of the current instruction + 4 (it is +8 already)
                    if rd == REG_PROGRAM_COUNTER {
                        v += 4;
                    }

                    self.bus.lock().unwrap().write_at(address, v as u8)
                }
                ReadWriteKind::Word => {
                    let mut v = self.registers.register_at(rd.try_into().unwrap());

                    // If R15 we get the value of the current instruction + 4 (it is +8 already)
                    if rd == REG_PROGRAM_COUNTER {
                        v += 4;
                    }

                    self.bus
                        .lock()
                        .unwrap()
                        .write_at(address, v.get_bits(0..=7) as u8);
                    self.bus
                        .lock()
                        .unwrap()
                        .write_at(address + 1, v.get_bits(8..=15) as u8);
                    self.bus
                        .lock()
                        .unwrap()
                        .write_at(address + 2, v.get_bits(16..=23) as u8);
                    self.bus
                        .lock()
                        .unwrap()
                        .write_at(address + 3, v.get_bits(24..=31) as u8);
                }
            },
            _ => todo!("implement single data transfer operation"),
        }

        // If LDR and Rd == R15 we flush the pipeline
        if kind == SingleDataTransferKind::Ldr && rd == REG_PROGRAM_COUNTER {
            self.flush_pipeline();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn block_data_transfer(
        &mut self,
        indexing: Indexing,
        offsetting: Offsetting,
        load_psr: bool,
        write_back: bool,
        load_store: LoadStoreKind,
        rn: u32,
        reg_list: u32,
    ) {
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

                    // If R15 we get the value of the current instruction + 4 (it is +8 already)
                    if reg_source == REG_PROGRAM_COUNTER.try_into().unwrap() {
                        value += 4;
                    }
                    let mut bus = arm.bus.lock().unwrap();
                    bus.write_word(address, value);
                }
            }
            LoadStoreKind::Load => |arm: &mut Self, address: usize, reg_destination: usize| {
                let v = arm.bus.lock().unwrap().read_word(address);
                arm.registers.set_register_at(reg_destination, v);
            },
        };

        self.exec_data_transfer(reg_list, indexing, &mut address, offsetting, transfer);

        if write_back {
            self.registers
                .set_register_at(base_register, address.try_into().unwrap());
        };

        // If LDM and R15 is in register list we flush the pipeline
        if load_store == LoadStoreKind::Load && reg_list.is_bit_on(15) {
            self.flush_pipeline();
        }
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

    pub fn branch(&mut self, is_link: bool, offset: u32) {
        let offset = offset.sign_extended(26) as i32;
        let old_pc: u32 = self.registers.program_counter().try_into().unwrap();
        if is_link {
            self.registers
                .set_register_at(14, old_pc.wrapping_sub(SIZE_OF_INSTRUCTION));
        }

        let new_pc = old_pc as i32 + offset;
        self.registers.set_program_counter(new_pc as u32);

        self.flush_pipeline();
    }

    pub fn multiply(
        &mut self,
        mul_variant: ArmModeMultiplyVariant,
        set_condition_codes: bool,
        rd: u32,
        rn: u32,
        rs: u32,
        rm: u32,
    ) {
        use ArmModeMultiplyVariant::*;
        match mul_variant {
            // Unsiged multiply (32-bit by 32-bit, bottom 32-bit result).
            Mul => self.mul_or_mla(set_condition_codes, false, rd, rn, rs, rm),
            // Unsiged multiply-accumulate (32-bit by 32-bit, bottom 32-bit accumulate and result).
            Mla => self.mul_or_mla(set_condition_codes, true, rd, rn, rs, rm),
        }
    }

    pub fn multiply_long(
        &mut self,
        mul_variant: ArmModeMultiplyLongVariant,
        set_condition_codes: bool,
        rdhi: u32,
        rdlo: u32,
        rs: u32,
        rm: u32,
    ) {
        use ArmModeMultiplyLongVariant::*;
        match mul_variant {
            // Unsigned long multiply (32-bit by 32-bit, 64-bit result)
            Umull => self.umull_or_umlal(set_condition_codes, false, rdhi, rdlo, rs, rm),
            // Unsigned long multiply-accumulate (32-bit by 32-bit, 64-bit accumulate and result)
            Umlal => self.umull_or_umlal(set_condition_codes, true, rdhi, rdlo, rs, rm),
            // Signed long multiply (32-bit by 32-bit, 64-bit result)
            Smull => self.smull_or_smlal(set_condition_codes, false, rdhi, rdlo, rs, rm),
            // Signed multiply-accumulate (32-bit by 32-bit, 64-bit accumulate and result)
            Smlal => self.smull_or_smlal(set_condition_codes, true, rdhi, rdlo, rs, rm),
        }
    }

    pub fn mul_or_mla(
        &mut self,
        set_condition_codes: bool,
        does_accumulate: bool,
        rd: u32,
        rn: u32,
        rs: u32,
        rm: u32,
    ) {
        let rm_operand_value = self.registers.register_at(rm as usize);
        let rs_operand_value = self.registers.register_at(rs as usize);

        let (mut result, _) = rm_operand_value.overflowing_mul(rs_operand_value);
        if does_accumulate {
            let rn_register_value = self.registers.register_at(rn as usize);
            let (result_add, _) = result.overflowing_add(rn_register_value);
            result = result_add
        }

        self.registers.set_register_at(rd as usize, result);

        if set_condition_codes {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(31));
        }
    }

    pub fn umull_or_umlal(
        &mut self,
        set_condition_codes: bool,
        does_accumulate: bool,
        rdhi: u32,
        rdlo: u32,
        rs: u32,
        rm: u32,
    ) {
        let rm_operand_value = self.registers.register_at(rm as usize) as u64;
        let rs_operand_value = self.registers.register_at(rs as usize) as u64;

        let (mut result, _) = rm_operand_value.overflowing_mul(rs_operand_value);
        if does_accumulate {
            let rdhi_register_value = self.registers.register_at(rdhi as usize) as u64;
            let rdlo_register_value = self.registers.register_at(rdlo as usize) as u64;
            let rdhilo_register_value = rdhi_register_value << 32 | rdlo_register_value;
            let (result_add, _) = result.overflowing_add(rdhilo_register_value);
            result = result_add;
        }

        self.registers
            .set_register_at(rdlo as usize, result.get_bits(0..=31) as u32);
        self.registers
            .set_register_at(rdhi as usize, result.get_bits(32..=63) as u32);

        if set_condition_codes {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(63));
        }
    }

    pub fn smull_or_smlal(
        &mut self,
        set_condition_codes: bool,
        does_accumulate: bool,
        rdhi: u32,
        rdlo: u32,
        rs: u32,
        rm: u32,
    ) {
        let rm_operand_value = self.registers.register_at(rm as usize);
        let rs_operand_value = self.registers.register_at(rs as usize);
        let rm_operand_value_sgn: i64 = (rm_operand_value as i32) as i64;
        let rs_operand_value_sgn: i64 = (rs_operand_value as i32) as i64;

        let (mut result_sgn, _) = rm_operand_value_sgn.overflowing_mul(rs_operand_value_sgn);
        if does_accumulate {
            let rdhi_register_value = self.registers.register_at(rdhi as usize) as u64;
            let rdlo_register_value = self.registers.register_at(rdlo as usize) as u64;
            let rdhilo_register_value = rdhi_register_value << 32 | rdlo_register_value;
            let rdhilo_register_value_sgn = rdhilo_register_value as i64;
            let (result_sgn_add, _) = result_sgn.overflowing_add(rdhilo_register_value_sgn);
            result_sgn = result_sgn_add;
        }

        let result = result_sgn as u64;
        self.registers
            .set_register_at(rdlo as usize, result.get_bits(0..=31) as u32);
        self.registers
            .set_register_at(rdhi as usize, result.get_bits(32..=63) as u32);

        if set_condition_codes {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(63));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::arm::alu_instruction::{AluSecondOperandInfo, ShiftOperator};
    use crate::cpu::arm::instructions::ArmModeInstruction::SingleDataTransfer;
    use crate::cpu::arm::instructions::{ArmModeInstruction, SingleDataTransferOffsetInfo};
    use crate::cpu::condition::Condition;
    use crate::cpu::flags::ShiftKind;

    use pretty_assertions::assert_eq;

    pub trait BitsUtilsTest
    where
        Self: Clone + Sized + Into<u128> + TryFrom<u128> + From<bool> + TryInto<u8> + From<u8>,
        <Self as TryFrom<u128>>::Error: std::fmt::Debug,
    {
        fn set_bits(&mut self, bits_range: std::ops::RangeInclusive<u8>, value: Self) {
            let start = bits_range.start();
            let length = bits_range.len() as u32;
            let self_bits: u128 = self.clone().into();

            // Set all of the desider bits to 1 then & it with the value provided
            // considering the right amount of bits (given by bits_range.len()).
            // Order goes from lsb to msb (right to left).
            let mask = (2_u128.pow(length)) - 1;
            let value_bits: u128 = value.into();
            let value_bits: u128 = (value_bits & mask) << start;

            // Now, shift the mask then flip it so we can choose where to insert
            // our value bits
            let reverse_mask = !(mask << start);

            // Say we have self being a u16 value with the following bits:
            //     0b0000....10010011_01110011_u128
            // and we want to set bits 7..=10, 4 bits starting from 7 to 0b1001.
            //
            // The constant `reverse_mask` will look something like:
            //     0b1111....11111000_01111111_u128
            // which is helpful for clearing the bits we are about to set.
            //
            //     0b0000....10010011_01110011_u128 &
            //     0b1111....11111000_01111111_u128
            //     --------------------------------
            //     0b0000....10010000_01110011_u128
            //
            // The value above can be used in bit-or with our value bits
            // to obtain the expected result:
            //
            //     0b0000....10010000_01110011_u128 |
            //     0b0000....00000100_10000000_u128 =
            //     --------------------------------
            //     0b0000....10010100_11110011_u128

            let new_self =
                <Self as TryFrom<u128>>::try_from((self_bits & reverse_mask) | value_bits).unwrap();
            self.clone_from(&new_self);
        }
    }

    impl BitsUtilsTest for u32 {}

    #[test]
    fn set_bits() {
        let mut b = 0b10001001_u32;
        b.set_bits(4..=5, 0b11);
        assert_eq!(b, 0b10111001_u32);
        b.set_bits(1..=2, 0b11);
        assert_eq!(b, 0b10111111_u32);

        let mut b = 0b00000000_00000000_u32;
        b.set_bits(0..=7, 0b11111111_u32);
        assert_eq!(b, 0b00000000_11111111_u32);
    }

    #[test]
    fn check_cmn() {
        {
            let op_code = 0b1110_00_0_1011_0_1001_1111_000000001110;
            let mut cpu = Arm7tdmi::default();
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
            cpu.execute_arm(Arm7tdmi::decode(op_code));
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
    }

    #[test]
    fn check_teq() {
        {
            let op_code = 0b1110_00_1_1001_1_1100_0000_000000000001;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstruction::Teq,
                    set_conditions: true,
                    op_kind: OperandKind::Immediate,
                    rn: 12,
                    destination: 0,
                    op2: AluSecondOperandInfo::Immediate { base: 1, shift: 0 }
                }
            );

            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "TEQ R12, #1");

            cpu.registers.set_register_at(12, 0xFFFFFFFF);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
            cpu.execute_arm(op_code);
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
        {
            let op_code: u32 = 0b0000_00_0_1001_0_1001_1111_000000001100;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::EQ,
                    psr_kind: PsrKind::Cpsr,
                    kind: PsrOpKind::Msr {
                        source_register: 12
                    }
                }
            );

            assert!(!cpu.cpsr.can_execute(op_code.condition));
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "MSREQ CPSR, R12");
        }

        let op_code = 0b1110_00_0_1001_1_1001_0011_000000000000;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Teq,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 9,
                destination: 3,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 0
                }
            }
        );

        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 100);
        cpu.cpsr.set_sign_flag(true); // set for later verify.
        cpu.execute_arm(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_cmp() {
        let op_code: u32 = 0b1110_00_1_1010_1_1110_0000_000000000000;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Cmp,
                set_conditions: true,
                op_kind: OperandKind::Immediate,
                rn: 14,
                destination: 0,
                op2: AluSecondOperandInfo::Immediate { base: 0, shift: 0 }
            }
        );

        let asm = op_code.instruction.disassembler();
        assert_eq!(asm, "CMP R14, #0");

        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        cpu.execute_arm(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
    }

    #[test]
    fn check_orr() {
        {
            let op_code: u32 = 0b0000_00_1_1100_0_1100_1100_000011000000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::EQ,
                    alu_instruction: ArmModeAluInstruction::Orr,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 12,
                    destination: 12,
                    op2: AluSecondOperandInfo::Immediate {
                        base: 192,
                        shift: 0
                    }
                }
            );

            assert!(!cpu.cpsr.can_execute(op_code.condition));
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "ORREQ R12, R12, #192");
        }
    }

    #[test]
    fn check_mov() {
        {
            let op_code: u32 = 0b0000_00_1_1101_0_0000_1110_000000000100;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::EQ,
                    alu_instruction: ArmModeAluInstruction::Mov,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 0,
                    destination: 14,
                    op2: AluSecondOperandInfo::Immediate { base: 4, shift: 0 }
                }
            );

            assert!(!cpu.cpsr.can_execute(op_code.condition));
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "MOVEQ R14, #4");
        }
        {
            let op_code: u32 = 0b1110_00_1_1101_0_0000_0000_000011011111;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstruction::Mov,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 0,
                    destination: 0,
                    op2: AluSecondOperandInfo::Immediate {
                        base: 223,
                        shift: 0
                    }
                }
            );

            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "MOV R0, #223");

            cpu.registers.set_register_at(0, 1);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
            cpu.execute_arm(op_code);
            assert_eq!(cpu.registers.register_at(0), 0xDF);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
        {
            let op_code: u32 = 0b1110_00_1_1101_0_0000_1100_001100000001;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstruction::Mov,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 0,
                    destination: 12,
                    op2: AluSecondOperandInfo::Immediate { base: 1, shift: 6 }
                }
            );

            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "MOV R12, #67108864");

            cpu.registers.set_register_at(12, 1);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
            cpu.execute_arm(op_code);
            assert_eq!(cpu.registers.register_at(12), 0x4000000);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }
    }

    #[test]
    fn check_add() {
        {
            let op_code: u32 = 0b1110_00_1_0100_0_1111_0000_000000000001;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstruction::Add,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 15,
                    destination: 0,
                    op2: AluSecondOperandInfo::Immediate { base: 1, shift: 0 }
                }
            );

            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "ADD R0, R15, #1");

            cpu.registers.set_register_at(15, 15);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
            cpu.execute_arm(op_code);
            assert_eq!(cpu.registers.register_at(0), 16);
            assert!(!cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.carry_flag());
            assert!(!cpu.cpsr.overflow_flag());
        }

        let op_code = 0b1110_00_1_0100_0_1111_0000_000000100000;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Add,
                set_conditions: false,
                op_kind: OperandKind::Immediate,
                rn: 15,
                destination: 0,
                op2: AluSecondOperandInfo::Immediate { base: 32, shift: 0 }
            }
        );
        cpu.registers.set_register_at(15, 15);
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(0), 15 + 32);
    }

    #[test]
    fn check_add_pc_operand_shift_register() {
        // Case when R15 is used as operand and shift amount is taken from register:
        // R2 = R1 + (R15 << R3)
        let op_code = 0b1110_00_0_0100_0_0001_0010_0011_0001_1111;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Add,
                set_conditions: false,
                op_kind: OperandKind::Register,
                rn: 1,
                destination: 2,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Register(3),
                    shift_kind: ShiftKind::Lsl,
                    register: 15,
                }
            },
        );

        cpu.registers.set_register_at(2, 5);
        cpu.registers.set_register_at(1, 10);
        cpu.registers.set_register_at(15, 500);
        cpu.registers.set_register_at(3, 0);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(2), 500 + 4 + 10);
    }

    #[test]
    fn check_add_carry_bit() {
        let op_code: u32 = 0b1110_00_0_0100_1_1111_0000_0000_0000_1110;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Add,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 15,
                destination: 0,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 14,
                }
            }
        );

        cpu.registers.set_register_at(15, (1 << 31) + 1);
        cpu.registers.set_register_at(14, 1 << 31);
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(0), 1);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_mov_rx_immediate() {
        // MOV R0, 0
        let mut op_code: u32 = 0b1110_00_1_1101_0_0000_0000_0000_0000_0000;

        // bits [11-8] are ROR-Shift applied to nn
        let is = op_code & 0b0000_0000_0000_0000_0000_1111_0000_0000;

        // MOV Rx,x
        let mut cpu = Arm7tdmi::default();
        let rx = 0;
        let register_for_op = rx << 12;
        let immediate_value = rx;

        // Rd parameter
        op_code = (op_code & 0b1111_1111_1111_1111_0000_1111_1111_1111) + register_for_op;
        // Immediate parameter
        op_code = (op_code & 0b1111_1111_1111_1111_1111_1111_0000_0000) + immediate_value;

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Mov,
                set_conditions: false,
                op_kind: OperandKind::Immediate,
                rn: 0,
                destination: 0,
                op2: AluSecondOperandInfo::Immediate { base: 0, shift: 0 }
            }
        );

        cpu.execute_arm(op_code);
        let rotated = rx.rotate_right(is * 2);
        assert_eq!(cpu.registers.register_at(rx.try_into().unwrap()), rotated);
    }

    #[test]
    fn check_mov_cpsr() {
        // Checks for Z flag
        let op_code = 0b1110_00_0_1101_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Mov,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );

        cpu.execute_arm(op_code);

        assert!(cpu.cpsr.zero_flag());

        // Checks for Z flag
        let op_code = 0b1110_00_0_1101_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.registers.set_register_at(2, -5_i32 as u32);
        cpu.execute_arm(op_code);

        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn shift_from_register_is_0() {
        let op_code = 0b1110_00_0_0100_0_0000_0001_0011_0111_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Add,
                set_conditions: false,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    register: 2,
                    shift_kind: ShiftKind::Ror,
                    shift_op: ShiftOperator::Register(3)
                }
            }
        );

        cpu.registers.set_register_at(0, 5);
        cpu.registers.set_register_at(2, 11);
        cpu.registers.set_register_at(3, 8 << 8);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 16);
    }

    #[test]
    fn check_and() {
        let op_code = 0b1110_00_1_0000_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::And,
                set_conditions: false,
                op_kind: OperandKind::Immediate,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Immediate {
                    base: 170,
                    shift: 0
                }
            }
        );

        // All 1 except msb
        cpu.registers.set_register_at(0, 2_u32.pow(31) - 1);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b10101010);
    }

    #[test]
    fn check_eor() {
        let op_code = 0b1110_00_1_0001_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Eor,
                set_conditions: false,
                op_kind: OperandKind::Immediate,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Immediate {
                    base: 170,
                    shift: 0
                }
            }
        );

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b01010101);
    }

    #[test]
    fn check_tst() {
        {
            let op_code = 0b0000_00_0_1000_0_1111_1100_0000_00000000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::EQ,
                    psr_kind: PsrKind::Cpsr,
                    kind: PsrOpKind::Mrs {
                        destination_register: 12
                    }
                }
            );

            assert!(!cpu.cpsr.can_execute(op_code.condition));
        }
        {
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00_1_1000_1_0000_0001_0000_00000000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstruction::Tst,
                    set_conditions: true,
                    op_kind: OperandKind::Immediate,
                    rn: 0,
                    destination: 1,
                    op2: AluSecondOperandInfo::Immediate { base: 0, shift: 0 }
                }
            );
            cpu.cpsr.set_sign_flag(true);

            cpu.execute_arm(op_code);
            assert!(cpu.cpsr.zero_flag());
            assert!(!cpu.cpsr.sign_flag());
        }
    }

    #[test]
    fn check_bic() {
        let op_code = 0b1110_00_1_1110_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Bic,
                set_conditions: false,
                op_kind: OperandKind::Immediate,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Immediate {
                    base: 170,
                    shift: 0
                }
            }
        );

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b01010101);
    }

    #[test]
    fn check_mvn() {
        let op_code = 0b1110_00_1_1111_1_0000_0001_0000_11111111;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Mvn,
                set_conditions: true,
                op_kind: OperandKind::Immediate,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Immediate {
                    base: 255,
                    shift: 0
                }
            }
        );

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), (2_u32.pow(24) - 1) << 8);
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_sub() {
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sub,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );

        cpu.registers.set_register_at(0, 10);
        cpu.registers.set_register_at(2, 5);
        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 5);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());

        //Covers carry logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.registers.set_register_at(2, 15);
        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1) as i32, -5);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());

        // Covers overflow logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sub,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );

        cpu.registers.set_register_at(0, 1);
        cpu.registers.set_register_at(2, i32::MIN as u32);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), (i32::MIN + 1) as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_adc() {
        // Covers all flags=0
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Adc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );

        cpu.registers.set_register_at(0, 1);
        cpu.registers.set_register_at(2, 1);
        cpu.cpsr.set_carry_flag(true);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 3);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers carry during first sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Adc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );

        cpu.cpsr.set_carry_flag(true);
        cpu.registers.set_register_at(0, u32::MAX);
        cpu.registers.set_register_at(2, 1);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 1);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers carry during second sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Adc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );

        cpu.cpsr.set_carry_flag(true);
        cpu.registers.set_register_at(0, u32::MAX - 1);
        cpu.registers.set_register_at(2, 1);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 0);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers overflow during first sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Adc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        // All 1 except MSB
        cpu.registers.set_register_at(0, i32::MAX as u32);
        cpu.registers.set_register_at(2, 1);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), (1 << 31) + 1);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers overflow during second sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Adc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        // All 1 except MSB
        cpu.registers.set_register_at(0, i32::MAX as u32 - 1);
        cpu.registers.set_register_at(2, 1);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 1 << 31);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_sbc() {
        // Covers all flag=0
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, 10);
        cpu.registers.set_register_at(2, 5);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 5);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers carry during first diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, 0);
        cpu.registers.set_register_at(2, 1);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers carry during sum
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, u32::MAX);
        cpu.registers.set_register_at(2, 0);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers carry during second diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(false);

        cpu.registers.set_register_at(0, 0);
        cpu.registers.set_register_at(2, 0);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers overflow during first diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, i32::MAX as u32);
        cpu.registers.set_register_at(2, -1_i32 as u32);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), 1 << 31);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers overflow during sum
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, i32::MAX as u32);
        cpu.registers.set_register_at(2, 0);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), i32::MAX as u32);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers overflow during second diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            ArmModeInstruction::DataProcessing {
                condition: Condition::AL,
                alu_instruction: ArmModeAluInstruction::Sbc,
                set_conditions: true,
                op_kind: OperandKind::Register,
                rn: 0,
                destination: 1,
                op2: AluSecondOperandInfo::Register {
                    shift_op: ShiftOperator::Immediate(0),
                    shift_kind: ShiftKind::Lsl,
                    register: 2,
                }
            }
        );
        cpu.cpsr.set_carry_flag(false);

        cpu.registers.set_register_at(0, i32::MIN as u32);
        cpu.registers.set_register_at(2, 0);

        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1), i32::MAX as u32);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_ror() {
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(5, 1);
        // rd = 5
        // value in rs = 10
        cpu.ror(5, 10);

        assert_eq!(4194304, cpu.registers.register_at(5));
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.carry_flag());
    }

    #[test]
    fn check_psr_transfer() {
        {
            // Covers MRS with CPSR and User mode
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_0_001111_0000_000000000000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Cpsr,
                    kind: PsrOpKind::Mrs {
                        destination_register: 0
                    }
                }
            );
            cpu.cpsr.set_mode(Mode::User);

            cpu.cpsr.set_carry_flag(true);
            cpu.cpsr.set_overflow_flag(true);
            cpu.cpsr.set_zero_flag(true);
            cpu.cpsr.set_sign_flag(true);

            cpu.execute_arm(op_code);

            assert_eq!(
                0b1111_00000000000000000000_110_10000,
                cpu.registers.register_at(0),
            );
        }
        {
            // Covers MRS with SPSR_fiq
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_1_001111_0000_000000000000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Spsr,
                    kind: PsrOpKind::Mrs {
                        destination_register: 0
                    }
                }
            );

            cpu.register_bank.spsr_fiq.set_state_bit(true);
            cpu.register_bank.spsr_fiq.set_mode(Mode::Fiq);
            cpu.register_bank.spsr_fiq.set_carry_flag(true);
            cpu.register_bank.spsr_fiq.set_overflow_flag(true);
            cpu.register_bank.spsr_fiq.set_zero_flag(true);
            cpu.register_bank.spsr_fiq.set_sign_flag(true);

            cpu.swap_mode(Mode::Fiq);

            cpu.execute_arm(op_code);

            assert_eq!(
                cpu.registers.register_at(0),
                0b1111_00000000000000000000_001_10001
            );
        }
        {
            // Covers MSR with CPSR and User Mode
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_0_1010011111_00000000_0000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Cpsr,
                    kind: PsrOpKind::Msr { source_register: 0 }
                }
            );
            cpu.cpsr.set_mode(Mode::User);

            cpu.registers.set_register_at(0, 0b1111 << 28);

            cpu.execute_arm(op_code);

            // All flags set and User mode
            assert_eq!(0b1111_00000000000000000000_110_10000, u32::from(cpu.cpsr));
        }
        {
            // Covers MSR with SPSR_fiq
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_1_1010011111_00000000_0000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Spsr,
                    kind: PsrOpKind::Msr { source_register: 0 }
                }
            );
            cpu.cpsr.set_mode(Mode::Fiq);

            cpu.registers.set_register_at(0, 0b1111 << 28 | (0b10001));

            cpu.execute_arm(op_code);

            // All flags set and Fiq mode
            assert_eq!(u32::from(cpu.spsr), 0b1111 << 28 | (0b10001));
        }
        {
            // Covers MSR-flags with CPSR and User mode
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00_0_10_0_1010001111_00000000_0000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Cpsr,
                    kind: PsrOpKind::MsrFlg {
                        operand: AluSecondOperandInfo::Register {
                            shift_op: ShiftOperator::Immediate(0),
                            shift_kind: ShiftKind::Lsl,
                            register: 0
                        }
                    }
                }
            );
            cpu.cpsr.set_mode(Mode::User);

            cpu.registers.set_register_at(0, 0b1111 << 28);

            cpu.execute_arm(op_code);

            // All flags set and User mode
            assert_eq!(0b1111_00000000000000000000_110_10000, u32::from(cpu.cpsr));
        }
        {
            // Covers MSR-flags with SPSR_fiq
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00_0_10_1_1010001111_00000000_0000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Spsr,
                    kind: PsrOpKind::MsrFlg {
                        operand: AluSecondOperandInfo::Register {
                            shift_op: ShiftOperator::Immediate(0),
                            shift_kind: ShiftKind::Lsl,
                            register: 0
                        }
                    }
                }
            );
            cpu.cpsr.set_mode(Mode::Fiq);

            // Trying to change MODE bits to a User mode
            cpu.registers.set_register_at(0, 0b1111 << 28 | (0b10000));

            cpu.execute_arm(op_code);

            // All flags set
            assert_eq!(u32::from(cpu.spsr), 0b1111 << 28);
        }
    }

    #[test]
    fn check_ldr() {
        {
            let op_code = 0b1110_01_0_1_1_1_0_1_1100_1100_001100000000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 12,
                    base_register: 12,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 768 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDRB R12, #768");
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000011010000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 208 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDR R13, #208");
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000010111000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 184 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDR R13, #184");
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000011010000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 208 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDR R13, #208");
        }
        {
            let op_code = 0b1110_0101_1101_1111_1101_0000_0001_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 24 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDRB R13, #24");

            // because in this specific case address will be
            // then will be 0x03000050 (.wrapping_add(offset))
            cpu.registers.set_program_counter(0x03000050);

            // simulate mem already contains something.
            cpu.bus.lock().unwrap().write_at(0x03000068, 99);

            cpu.execute_arm(op_code);
            assert_eq!(cpu.registers.register_at(13), 99);
            assert_eq!(cpu.registers.program_counter(), 0x03000050);
        }
    }

    #[test]
    fn check_str() {
        {
            let op_code = 0b1110_01_0_1_1_1_0_0_0100_0100_001000001000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Str,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 4,
                    base_register: 4,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 520 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "STRB R4, #520");
        }
        {
            let op_code: u32 = 0b1110_0101_1000_0001_0001_0000_0000_0000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Str,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 1,
                    base_register: 1,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 0 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "STR R1, #0");

            cpu.registers.set_register_at(1, 16843009);

            // because in this specific case address will be
            // then will be 0x03000050 + 8 (.wrapping_sub(offset))
            cpu.registers.set_program_counter(0x03000050);

            cpu.execute_arm(op_code);

            let bus = cpu.bus.lock().unwrap();
            let memory = bus.internal_memory.lock().unwrap();

            assert_eq!(memory.read_at(0x01010101), 1);
            assert_eq!(memory.read_at(0x01010101 + 1), 1);
            assert_eq!(memory.read_at(0x01010101 + 2), 1);
            assert_eq!(memory.read_at(0x01010101 + 3), 1);
            assert_eq!(cpu.registers.program_counter(), 0x03000050);
        }
        {
            let op_code = 0b1110_0101_1100_1111_1101_0000_0001_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Str,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 24 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "STRB R13, #24");

            // because in this specific case address will be
            // then will be 0x03000050 (.wrapping_add(offset))
            cpu.registers.set_program_counter(0x03000050);
            cpu.registers.set_register_at(13, 50);

            cpu.execute_arm(op_code);

            let bus = cpu.bus.lock().unwrap();
            let memory = bus.internal_memory.lock().unwrap();

            assert_eq!(memory.read_at(0x03000068), 50);
            assert_eq!(cpu.registers.program_counter(), 0x03000050);
        }
    }

    #[test]
    fn check_ldr_word() {
        let op_code = 0b1110_0101_1001_1111_1101_0000_0010_1000;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        assert_eq!(
            op_code.instruction,
            SingleDataTransfer {
                condition: Condition::AL,
                kind: SingleDataTransferKind::Ldr,
                quantity: ReadWriteKind::Word,
                write_back: false,
                indexing: Indexing::Pre,
                rd: 13,
                base_register: 15,
                offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 40 },
                offsetting: Offsetting::Up,
            }
        );

        {
            let mut memory = cpu.bus.lock().unwrap();

            // simulate mem already contains something.
            // in u32 this is 16843009 00000001_00000001_00000001_00000001.
            memory.write_at(0x28, 1);
            memory.write_at(0x28 + 1, 1);
            memory.write_at(0x28 + 2, 1);
            memory.write_at(0x28 + 3, 1);
        }
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(13), 16843009);
        assert_eq!(cpu.registers.program_counter(), 0);
    }

    #[test]
    fn check_multiply_non_halfword_mul() {
        let mut cpu = Arm7tdmi::default();

        let rm_operand_register: u32 = 5;
        let rs_operand_register: u32 = 6;
        let rd_destination_register: u32 = 7;

        cpu.registers
            .set_register_at(rm_operand_register as usize, 100);
        cpu.registers
            .set_register_at(rs_operand_register as usize, 101);
        cpu.registers
            .set_register_at(rd_destination_register as usize, 0);

        let mut op_code = 0u32;
        op_code.set_bits(4..=7, 0b1001);
        op_code.set_bits(0..=3, rm_operand_register);
        op_code.set_bits(8..=11, rs_operand_register);
        op_code.set_bits(16..=19, rd_destination_register);
        op_code.set_bits(20..=20, 0b1);
        op_code.set_bits(21..=24, 0b0000); // 0000b: MUL{cond}{S}   Rd,Rm,Rs        ;multiply   Rd = Rm*Rs
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(28..=31, Condition::AL as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);
        assert_eq!(
            cpu.registers.register_at(rd_destination_register as usize),
            10100_u32
        );
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_multiply_non_halfword_mla() {
        let mut cpu = Arm7tdmi::default();

        let rm_operand_register: u32 = 5;
        let rs_operand_register: u32 = 6;
        let rd_destination_register: u32 = 7;
        let rn_acc_register: u32 = 8;

        cpu.registers
            .set_register_at(rm_operand_register as usize, 100);
        cpu.registers
            .set_register_at(rs_operand_register as usize, 101);
        cpu.registers
            .set_register_at(rd_destination_register as usize, 0);
        cpu.registers.set_register_at(rn_acc_register as usize, 32);

        let mut op_code = 0u32;
        op_code.set_bits(4..=7, 0b1001);
        op_code.set_bits(0..=3, rm_operand_register);
        op_code.set_bits(8..=11, rs_operand_register);
        op_code.set_bits(12..=15, rn_acc_register);
        op_code.set_bits(16..=19, rd_destination_register);
        op_code.set_bits(20..=20, 0b1);
        op_code.set_bits(21..=24, 0b0001); // 0001b: MLA{cond}{S}   Rd,Rm,Rs,Rn     ;mul.& accumulate Rd = Rm*Rs+Rn
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(28..=31, Condition::AL as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);
        assert_eq!(
            cpu.registers.register_at(rd_destination_register as usize),
            10132_u32
        );
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_multiply_long_non_halfword_umull() {
        let mut cpu = Arm7tdmi::default();

        let rm_operand_register: u32 = 5;
        let rs_operand_register: u32 = 6;
        let rdhi_destination_register: u32 = 7;
        let rdlo_destination_register: u32 = 8;

        cpu.registers
            .set_register_at(rm_operand_register as usize, 123456);
        cpu.registers
            .set_register_at(rs_operand_register as usize, 654321);
        cpu.registers
            .set_register_at(rdhi_destination_register as usize, 0);
        cpu.registers
            .set_register_at(rdlo_destination_register as usize, 0);

        let mut op_code = 0u32;
        op_code.set_bits(4..=7, 0b1001);
        op_code.set_bits(0..=3, rm_operand_register);
        op_code.set_bits(8..=11, rs_operand_register);
        op_code.set_bits(12..=15, rdlo_destination_register);
        op_code.set_bits(16..=19, rdhi_destination_register);
        op_code.set_bits(20..=20, 0b1);
        op_code.set_bits(21..=24, 0b0100); // 0100b: UMULL{cond}{S} RdLo,RdHi,Rm,Rs ;multiply   RdHiLo=Rm*Rs
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(28..=31, Condition::AL as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);
        let rdhi_register_value: u64 =
            cpu.registers
                .register_at(rdhi_destination_register as usize) as u64;
        let rdlo_register_value: u64 =
            cpu.registers
                .register_at(rdlo_destination_register as usize) as u64;
        let rdhilo_register_value: u64 = rdhi_register_value << 32 | rdlo_register_value;
        assert_eq!(rdhilo_register_value, 123456 * 654321);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_multiply_long_non_halfword_umlal() {
        let mut cpu = Arm7tdmi::default();

        let rm_operand_register: u32 = 5;
        let rs_operand_register: u32 = 6;
        let rdhi_destination_register: u32 = 7;
        let rdlo_destination_register: u32 = 8;

        let operand_1 = 123456_u32;
        let operand_2 = 654321_u32;
        let accumulate = 123456789_u32;
        cpu.registers
            .set_register_at(rm_operand_register as usize, operand_1);
        cpu.registers
            .set_register_at(rs_operand_register as usize, operand_2);
        cpu.registers
            .set_register_at(rdhi_destination_register as usize, 0_u32);
        cpu.registers
            .set_register_at(rdlo_destination_register as usize, accumulate);

        let mut op_code = 0u32;
        op_code.set_bits(4..=7, 0b1001);
        op_code.set_bits(0..=3, rm_operand_register);
        op_code.set_bits(8..=11, rs_operand_register);
        op_code.set_bits(12..=15, rdlo_destination_register);
        op_code.set_bits(16..=19, rdhi_destination_register);
        op_code.set_bits(20..=20, 0b1);
        op_code.set_bits(21..=24, 0b0101); // 0101b: UMLAL{cond}{S} RdLo,RdHi,Rm,Rs ;mul.& acc. RdHiLo=Rm*Rs+RdHiLo
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(28..=31, Condition::AL as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);
        let rdhi_register_value: u64 =
            cpu.registers
                .register_at(rdhi_destination_register as usize) as u64;
        let rdlo_register_value: u64 =
            cpu.registers
                .register_at(rdlo_destination_register as usize) as u64;
        let rdhilo_register_value: u64 = rdhi_register_value << 32 | rdlo_register_value;

        let expected = operand_1 as u64 * operand_2 as u64 + accumulate as u64;
        assert_eq!(rdhilo_register_value, expected);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_multiply_long_non_halfword_smull() {
        let mut cpu = Arm7tdmi::default();

        let rm_operand_register: u32 = 5;
        let rs_operand_register: u32 = 6;
        let rdhi_destination_register: u32 = 7;
        let rdlo_destination_register: u32 = 8;

        let operand_1 = -123456_i32;
        let operand_2 = 654321_i32;
        cpu.registers.set_register_at(
            rm_operand_register as usize,
            u32::from_be_bytes(operand_1.to_be_bytes()),
        );
        cpu.registers.set_register_at(
            rs_operand_register as usize,
            u32::from_be_bytes(operand_2.to_be_bytes()),
        );
        cpu.registers
            .set_register_at(rdhi_destination_register as usize, 0_u32);
        cpu.registers
            .set_register_at(rdlo_destination_register as usize, 0_u32);

        let mut op_code = 0u32;
        op_code.set_bits(4..=7, 0b1001);
        op_code.set_bits(0..=3, rm_operand_register);
        op_code.set_bits(8..=11, rs_operand_register);
        op_code.set_bits(12..=15, rdlo_destination_register);
        op_code.set_bits(16..=19, rdhi_destination_register);
        op_code.set_bits(20..=20, 0b1);
        op_code.set_bits(21..=24, 0b0110); // 0110b: SMULL{cond}{S} RdLo,RdHi,Rm,Rs ;sign.mul.  RdHiLo=Rm*Rs
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(28..=31, Condition::AL as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);
        let rdhi_register_value: u64 =
            cpu.registers
                .register_at(rdhi_destination_register as usize) as u64;
        let rdlo_register_value: u64 =
            cpu.registers
                .register_at(rdlo_destination_register as usize) as u64;
        let rdhilo_register_value: u64 = rdhi_register_value << 32 | rdlo_register_value;
        let rdhilo_register_value: i64 = i64::from_be_bytes(rdhilo_register_value.to_be_bytes());

        let expected = operand_1 as i64 * operand_2 as i64;
        assert_eq!(rdhilo_register_value, expected);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_multiply_long_non_halfword_smlal() {
        let mut cpu = Arm7tdmi::default();

        let rm_operand_register: u32 = 5;
        let rs_operand_register: u32 = 6;
        let rdlo_destination_register: u32 = 7;
        let rdhi_destination_register: u32 = 8;

        let operand_1 = 453_i32;
        let operand_2 = -754_i32;
        let accumulate = 98764_i64;
        let accumulate_u64 = u64::from_be_bytes(accumulate.to_be_bytes());
        cpu.registers.set_register_at(
            rm_operand_register as usize,
            u32::from_be_bytes(operand_1.to_be_bytes()),
        );
        cpu.registers.set_register_at(
            rs_operand_register as usize,
            u32::from_be_bytes(operand_2.to_be_bytes()),
        );

        cpu.registers.set_register_at(
            rdlo_destination_register as usize,
            accumulate_u64.get_bits(0..=31) as u32,
        );
        cpu.registers.set_register_at(
            rdhi_destination_register as usize,
            accumulate_u64.get_bits(32..=63) as u32,
        );

        let mut op_code = 0u32;
        op_code.set_bits(4..=7, 0b1001);
        op_code.set_bits(0..=3, rm_operand_register);
        op_code.set_bits(8..=11, rs_operand_register);
        op_code.set_bits(12..=15, rdlo_destination_register as u32);
        op_code.set_bits(16..=19, rdhi_destination_register as u32);
        op_code.set_bits(20..=20, 0b1);
        op_code.set_bits(21..=24, 0b0111); // 0111b: SMLAL{cond}{S} RdLo,RdHi,Rm,Rs ;sign.m&a.  RdHiLo=Rm*Rs+RdHiLo
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(25..=27, 0b000);
        op_code.set_bits(28..=31, Condition::AL as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

        cpu.execute_arm(op_code);
        let rdhi_register_value: u64 =
            cpu.registers
                .register_at(rdhi_destination_register as usize) as u64;
        let rdlo_register_value: u64 =
            cpu.registers
                .register_at(rdlo_destination_register as usize) as u64;
        let rdhilo_register_value: u64 = rdhi_register_value << 32 | rdlo_register_value;
        let rdhilo_register_value: i64 = i64::from_be_bytes(rdhilo_register_value.to_be_bytes());

        let expected = operand_1 as i64 * operand_2 as i64 + accumulate;
        assert_eq!(rdhilo_register_value, expected);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
    }
}
