use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::{
    AIKind, ArithmeticOpResult, ArmModeAluInstr, Kind, PsrOpKind, shift,
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
use crate::cpu::registers::REG_PC;

use super::alu_instruction::PsrKind;

pub const SIZE_OF_INSTRUCTION: u32 = 4;

impl Arm7tdmi {
    /// Executes a data processing (ALU) instruction.
    ///
    /// # Panics
    ///
    /// Panics if register index conversion fails.
    pub fn data_processing(
        &mut self,
        op_code: ArmModeOpcode, // FIXME: This parameter will be remove after change `psr_transfer`.
        alu_instruction: ArmModeAluInstr,
        set_conditions: bool,
        op_kind: OperandKind,
        rn: u32,
        destination: u32,
    ) {
        let offset = match rn {
            // if Rn is R15(PC) we need to offset its value because of
            // instruction pipelining
            REG_PC => Self::get_pc_offset_alu(op_kind, op_code.get_bit(4)),
            _ => 0,
        };
        let op1 = self.registers.register_at(rn.try_into().unwrap()) + offset;

        let op2 = self.get_operand(
            alu_instruction,
            set_conditions,
            op_kind,
            op_code.get_bits(0..=11),
        );

        match alu_instruction {
            ArmModeAluInstr::And => {
                self.and(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Eor => {
                self.eor(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Sub => {
                self.sub(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Rsb => {
                self.rsb(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Add => {
                self.add(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Adc => {
                self.adc(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Sbc => {
                self.sbc(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Rsc => {
                self.rsc(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Tst => self.tst(op1, op2),
            ArmModeAluInstr::Teq => self.teq(op1, op2),
            ArmModeAluInstr::Cmp => self.cmp(op1, op2),
            ArmModeAluInstr::Cmn => self.cmn(op1, op2),
            ArmModeAluInstr::Orr => {
                self.orr(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Mov => {
                self.mov(destination.try_into().unwrap(), op2, set_conditions);
            }
            ArmModeAluInstr::Bic => {
                self.bic(destination.try_into().unwrap(), op1, op2, set_conditions);
            }
            ArmModeAluInstr::Mvn => {
                self.mvn(destination.try_into().unwrap(), op2, set_conditions);
            }
        }

        if set_conditions && destination == REG_PC {
            // We move current SPSR into the CPSR.

            assert!(
                !(self.cpsr.mode() == Mode::User),
                "S=1 and Rd=0xF is forbidden in User mode"
            );

            // We store the current spsr in a temp variable because `swap_mode` would overwrite it.
            // We need to call `swap_mode` because we need to swap banked registers.
            let current_spsr = self.spsr;

            // Try to get the mode from SPSR. If it's invalid (e.g., BIOS wrote 0),
            // we skip the mode swap and just copy the CPSR value.
            // The CPU will be in an undefined state, but we won't panic.
            let spsr_value: u32 = current_spsr.into();
            if let Ok(spsr_mode) = Mode::try_from(spsr_value & 0b11111) {
                self.swap_mode(spsr_mode);
            } else {
                tracing::warn!(
                    "SPSR has invalid mode bits (0b{:05b}), skipping mode swap",
                    spsr_value & 0b11111
                );
            }

            self.cpsr = current_spsr;
        }

        // Test instructions do not modify destination so we don't flush pipeline even if
        // destination == R15
        if !matches!(
            alu_instruction,
            ArmModeAluInstr::Teq
                | ArmModeAluInstr::Cmn
                | ArmModeAluInstr::Cmp
                | ArmModeAluInstr::Tst
        ) && destination == REG_PC
        {
            self.flush_pipeline();
        }
    }

    /// Executes a PSR transfer instruction (MRS/MSR).
    ///
    /// # Panics
    ///
    /// Panics if R15 is used as source/destination register.
    pub fn psr_transfer(&mut self, op_kind: PsrOpKind, psr_kind: PsrKind) {
        // Accessing SPSR in User/System mode has unpredictable behavior on real hardware
        // Most implementations return CPSR instead
        let effective_psr_kind = if matches!(self.cpsr.mode(), Mode::System | Mode::User)
            && psr_kind == PsrKind::Spsr
        {
            tracing::warn!(
                "Attempting to access SPSR in User/System mode at PC=0x{:08X}, returning CPSR instead",
                self.registers.program_counter().wrapping_sub(8)
            );
            PsrKind::Cpsr
        } else {
            psr_kind
        };

        match op_kind {
            PsrOpKind::Mrs {
                destination_register,
            } => {
                assert!(
                    destination_register != REG_PC,
                    "PSR transfer should not use R15 as source/destination"
                );

                let psr = match effective_psr_kind {
                    PsrKind::Cpsr => self.cpsr,
                    PsrKind::Spsr => self.spsr,
                };

                self.registers
                    .set_register_at(destination_register.try_into().unwrap(), psr.into());
            }
            PsrOpKind::Msr { source_register } => {
                assert!(
                    source_register != REG_PC,
                    "PSR transfer should not use R15 as source/destination"
                );

                let rm = self
                    .registers
                    .register_at(source_register.try_into().unwrap());

                let current_mode = self.cpsr.mode();

                // If we're modifying CPSR and changing modes, swap banked registers FIRST
                // before updating CPSR, otherwise swap_mode will see we're already in the new mode
                if effective_psr_kind == PsrKind::Cpsr && current_mode != Mode::User {
                    let new_mode_bits = rm.get_bits(0..=4);
                    if let Ok(new_mode) = Mode::try_from(new_mode_bits)
                        && current_mode != new_mode
                    {
                        self.swap_mode(new_mode);
                    }
                }

                {
                    let psr = match effective_psr_kind {
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
                            tracing::warn!(
                                "Changing state bit (arm/thumb) in MSR instruction. This should not happen."
                            );
                        }
                        psr.set_state_bit(rm.get_bit(5));
                    }
                }

                // If modifying SPSR, update mode bits using set_mode_raw
                if effective_psr_kind == PsrKind::Spsr {
                    // If we're modifying SPSR we're sure we're not in System|User (checked before)
                    // We use `set_mode_raw` since the BIOS sometimes writes 0 in the SPSR.
                    self.spsr.set_mode_raw(rm.get_bits(0..=4));
                } else if effective_psr_kind == PsrKind::Cpsr && current_mode != Mode::User {
                    // For CPSR, set the mode bits directly (swap was already done above)
                    self.cpsr.set_mode_raw(rm.get_bits(0..=4));
                }
            }
            PsrOpKind::MsrFlg {
                operand,
                field_mask,
            } => {
                let op = match operand {
                    crate::cpu::arm::alu_instruction::AluSecondOperandInfo::Register {
                        shift_op: _,
                        shift_kind: _,
                        register,
                    } => self.registers.register_at(register.try_into().unwrap()),
                    crate::cpu::arm::alu_instruction::AluSecondOperandInfo::Immediate {
                        base,
                        shift,
                    } => base.rotate_right(shift),
                };
                let current_mode = self.cpsr.mode();

                // field_mask bits: bit 0 = control (0-7), bit 1 = extension (8-15),
                //                   bit 2 = status (16-23), bit 3 = flags (24-31)

                // If we're modifying CPSR control field (mode bits), swap banked registers FIRST
                if effective_psr_kind == PsrKind::Cpsr
                    && field_mask & 0b0001 != 0
                    && current_mode != Mode::User
                {
                    let new_mode_bits = op.get_bits(0..=4);
                    if let Ok(new_mode) = Mode::try_from(new_mode_bits)
                        && current_mode != new_mode
                    {
                        self.swap_mode(new_mode);
                    }
                }

                let psr = match effective_psr_kind {
                    PsrKind::Cpsr => &mut self.cpsr,
                    PsrKind::Spsr => &mut self.spsr,
                };

                // Update fields based on field_mask
                if field_mask & 0b1000 != 0 {
                    // Flags field (bits 24-31)
                    psr.set_sign_flag(op.get_bit(31));
                    psr.set_zero_flag(op.get_bit(30));
                    psr.set_carry_flag(op.get_bit(29));
                    psr.set_overflow_flag(op.get_bit(28));
                }

                if field_mask & 0b0001 != 0 && current_mode != Mode::User {
                    // Control field (bits 0-7): mode bits, IRQ/FIQ disable, state bit
                    psr.set_irq_disable(op.get_bit(7));
                    psr.set_fiq_disable(op.get_bit(6));

                    if psr.state_bit() != op.get_bit(5) {
                        tracing::warn!(
                            "Changing state bit (arm/thumb) in MSR instruction. This should not happen."
                        );
                    }
                    psr.set_state_bit(op.get_bit(5));

                    // Set mode bits
                    if effective_psr_kind == PsrKind::Spsr {
                        psr.set_mode_raw(op.get_bits(0..=4));
                    } else if effective_psr_kind == PsrKind::Cpsr {
                        // For CPSR, mode bits were already swapped above
                        psr.set_mode_raw(op.get_bits(0..=4));
                    }
                }

                // Extension and status fields (bits 8-23) are reserved/unused on ARM7TDMI
            }
        }
    }

    pub fn shift_operand(
        &mut self,
        alu_instruction: ArmModeAluInstr,
        s: bool,
        shift_kind: ShiftKind,
        shift_amount: u32,
        rm: u32,
    ) -> u32 {
        let result = shift(shift_kind, shift_amount, rm, self.cpsr.carry_flag());

        // If the instruction is a logical ALU instruction and S is set we set the carry flag
        if alu_instruction.kind() == AIKind::Logical && s {
            self.cpsr.set_carry_flag(result.carry);
        }

        result.result
    }

    /// Gets the second operand for an ALU instruction.
    ///
    /// # Panics
    ///
    /// Panics if register index conversion fails.
    pub fn get_operand(
        &mut self,
        alu_instruction: ArmModeAluInstr,
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
                    REG_PC => Self::get_pc_offset_alu(i, r),
                    _ => 0,
                };
                let rm = self.registers.register_at(rm.try_into().unwrap()) + offset;
                let shift_kind = op2.get_bits(5..=6).into();

                let shift_amount = if r {
                    // the shift amount is read from Rs
                    // bits [11-8] - Shift register (R0-R14) - only lower 8bit 0-255 used
                    let rs = op2.get_bits(8..=11);
                    let rs = self.registers.register_at(rs.try_into().unwrap()) & 0xFF;
                    // If shift is taken from register and the value is 0 Rm is directly used as operand
                    if rs == 0 {
                        return rm;
                    }

                    rs
                } else {
                    // the shift amount is in the instruction
                    // bits [7-11] - Shift amount
                    op2.get_bits(7..=11)
                };

                self.shift_operand(alu_instruction, s, shift_kind, shift_amount, rm)
            }
            OperandKind::Immediate => {
                // bits [7-0] are the immediate value
                let imm = op2.get_bits(0..=7);
                // bit [11-8] are the rotate amount (multiplied by 2 to get actual rotation)
                let rotate_amount = op2.get_bits(8..=11) * 2;

                if rotate_amount == 0 {
                    // No rotation, carry flag not affected
                    imm
                } else {
                    // Use shift_operand to handle rotation and carry flag update for logical operations
                    self.shift_operand(alu_instruction, s, ShiftKind::Ror, rotate_amount, imm)
                }
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
    pub(crate) fn get_pc_offset_alu(i: OperandKind, r: bool) -> u32 {
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

    pub fn adc(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
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
            self.cpsr.set_flags(&result_op);
        }
    }

    pub fn sbc(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        // SBC computes: Rd = Rn - Op2 - NOT(Carry)
        // where NOT(Carry) = 0 if carry is set, 1 if carry is clear
        let not_carry = u32::from(!self.cpsr.carry_flag());

        // Calculate: Rn - Op2 - NOT(C)
        // Use u64 to properly detect carry/borrow
        let rn_64 = rn as u64;
        let op2_64 = op2 as u64;
        let not_carry_64 = not_carry as u64;

        // Compute the subtraction
        let result_64 = rn_64.wrapping_sub(op2_64).wrapping_sub(not_carry_64);
        let result = result_64 as u32;

        // Carry flag: set if no borrow occurred (Rn >= Op2 + NOT(C))
        let carry_out = rn_64 >= (op2_64 + not_carry_64);

        // Overflow: for subtraction A - B, overflow occurs when:
        // - Operands have different signs (sign_A != sign_B)
        // - AND result has opposite sign from A (sign_result != sign_A)
        let sign_rn = rn.get_bit(31);
        let sign_op2 = op2.get_bit(31);
        let sign_result = result.get_bit(31);
        let overflow = (sign_rn != sign_op2) && (sign_result != sign_rn);

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_carry_flag(carry_out);
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(sign_result);
            self.cpsr.set_overflow_flag(overflow);
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
            self.cpsr.set_flags(&sub_result);
        }
    }

    fn rsb(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        self.sub(rd, op2, rn, s);
    }

    #[must_use]
    pub fn add_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
        // we do the sum in 64bits so that the 32nd bit is the carry
        let result_and_carry = (first_op as u64).wrapping_add(second_op as u64);
        let result = result_and_carry as u32;

        let sign_op1 = first_op.get_bit(31);
        let sign_op2 = second_op.get_bit(31);
        let sign_r = result.get_bit(31);

        let carry = (result_and_carry & 0x0001_0000_0000) >> 32 == 1;

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

    #[must_use]
    pub fn sub_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
        let result = first_op.wrapping_sub(second_op);

        let sign_op1 = first_op.get_bit(31);
        let sign_op2 = second_op.get_bit(31);
        let sign_r = result.get_bit(31);

        let different_sign = sign_op1 != sign_op2;

        ArithmeticOpResult {
            result,
            carry: first_op >= second_op,
            overflow: different_sign && sign_op2 == sign_r,
            sign: result.get_bit(31),
            zero: result == 0,
        }
    }

    fn add(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let add_result = Self::add_inner_op(rn, op2);

        self.registers.set_register_at(rd, add_result.result);

        if s {
            self.cpsr.set_flags(&add_result);
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

        self.cpsr.set_flags(&sub_result);
    }

    pub fn cmn(&mut self, rn: u32, op2: u32) {
        let add_result = Self::add_inner_op(rn, op2);

        self.cpsr.set_flags(&add_result);
    }

    pub fn orr(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn | op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.is_bit_on(31));
        }
    }

    pub fn mov(&mut self, rd: usize, op2: u32, s: bool) {
        self.registers.set_register_at(rd, op2);

        if s {
            self.cpsr.set_zero_flag(op2 == 0);
            self.cpsr.set_sign_flag(op2.get_bit(31));
        }
    }

    pub fn bic(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
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

        // Clear appropriate bits based on target mode
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

    /// Executes a halfword/signed data transfer instruction.
    ///
    /// # Panics
    ///
    /// Panics if register index conversion fails.
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

        if base_register == REG_PC {
            assert!(
                !write_back,
                "WriteBack should not be specified when using R15 as base register."
            );

            assert!(!(indexing == Indexing::Post), "Post indexing uses write back but we're using R15 as base register.
                 Documentation says that when using R15 as base register WB should not be used. What should we do?");
        }

        let effective = match offsetting {
            Offsetting::Down => address.wrapping_sub(offset),
            Offsetting::Up => address.wrapping_add(offset),
        };

        // For STORE with writeback when Rd == Rn, save the value before writeback
        let store_value_before_writeback = if load_store_kind == LoadStoreKind::Store {
            if source_destination_register == REG_PC {
                let pc: u32 = self.registers.program_counter().try_into().unwrap();
                Some(pc + 4)
            } else {
                Some(
                    self.registers
                        .register_at(source_destination_register as usize),
                )
            }
        } else {
            None
        };

        // Perform writeback before load to match ARM behavior
        // This ensures that when Rd == Rn in a load, the loaded value is preserved
        let address: usize = match indexing {
            Indexing::Post => {
                // Post-indexing: use original address, then writeback
                self.registers
                    .set_register_at(base_register.try_into().unwrap(), effective);
                address.try_into().unwrap()
            }
            Indexing::Pre => {
                if write_back {
                    self.registers
                        .set_register_at(base_register.try_into().unwrap(), effective);
                }
                effective.try_into().unwrap()
            }
        };

        match load_store_kind {
            LoadStoreKind::Store => {
                // Use the value saved before writeback
                let value = store_value_before_writeback.unwrap();

                // On ARM7TDMI, SH=10 (LDRD) and SH=11 (STRD) with L=0 are undefined
                // Some games may hit these encodings. We treat them as STRH for compatibility.
                match transfer_kind {
                    HalfwordTransferKind::UnsignedHalfwords
                    | HalfwordTransferKind::SignedByte
                    | HalfwordTransferKind::SignedHalfwords => {
                        self.bus.write_half_word(address, value as u16);
                    }
                }
            }
            LoadStoreKind::Load => match transfer_kind {
                HalfwordTransferKind::UnsignedHalfwords => {
                    let v = self.read_half_word(address, false);
                    self.registers
                        .set_register_at(source_destination_register as usize, v);
                }
                HalfwordTransferKind::SignedByte => {
                    let v = self.bus.read_byte(address) as u32;
                    self.registers
                        .set_register_at(source_destination_register as usize, v.sign_extended(8));
                }
                HalfwordTransferKind::SignedHalfwords => {
                    let v = self.read_half_word(address, true);
                    self.registers
                        .set_register_at(source_destination_register as usize, v);
                }
            },
        }

        if load_store_kind == LoadStoreKind::Load && source_destination_register == REG_PC {
            self.flush_pipeline();
        }
    }

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

        // For STR with writeback when rd == base_register, we need to save the old value
        // before writeback modifies it
        let str_value_before_writeback = if kind == SingleDataTransferKind::Str {
            let mut v = self.registers.register_at(rd.try_into().unwrap());
            // If R15 we get the value of the current instruction + 4 (it is +8 already)
            if rd == REG_PC {
                v += 4;
            }
            Some(v)
        } else {
            None
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
                        .set_register_at(base_register as usize, offset_address);
                }
                offset_address as usize
            }
        };

        match kind {
            SingleDataTransferKind::Ldr => match quantity {
                ReadWriteKind::Byte => {
                    let value = self.bus.read_byte(address) as u32;
                    self.registers
                        .set_register_at(rd.try_into().unwrap(), value);
                }
                ReadWriteKind::Word => {
                    let v = self.read_word(address);

                    self.registers.set_register_at(rd.try_into().unwrap(), v);
                }
            },
            SingleDataTransferKind::Str => {
                // Use the value saved before writeback
                let v = str_value_before_writeback.unwrap();
                match quantity {
                    ReadWriteKind::Byte => {
                        self.bus.write_byte(address, v as u8);
                    }
                    ReadWriteKind::Word => {
                        self.bus.write_word(address, v);
                    }
                }
            }
            SingleDataTransferKind::Pld => todo!("implement single data transfer operation"),
        }

        // If LDR and Rd == R15 we flush the pipeline
        if kind == SingleDataTransferKind::Ldr && rd == REG_PC {
            self.flush_pipeline();
        }
    }

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
        let mut address: usize = memory_base.try_into().unwrap();

        let is_empty_list = reg_list == 0;
        let r15_in_list = reg_list.is_bit_on(15) || is_empty_list; // Empty list loads/stores R15
        let use_user_registers = load_psr && !r15_in_list;

        let transfer = match (load_store, use_user_registers) {
            (LoadStoreKind::Store, false) => {
                |arm: &mut Self, address: usize, reg_source: usize| {
                    let mut value = arm.registers.register_at(reg_source);

                    // If R15 we get the value of the current instruction + 4 (it is +8 already)
                    if reg_source == REG_PC.try_into().unwrap() {
                        value += 4;
                    }

                    arm.bus.write_word(address, value);
                }
            }
            (LoadStoreKind::Store, true) => {
                // STM with S bit and R15 not in list: Store user mode registers
                |arm: &mut Self, address: usize, reg_source: usize| {
                    let mut value = arm.read_user_register(reg_source);

                    // If R15 we get the value of the current instruction + 4 (it is +8 already)
                    if reg_source == REG_PC.try_into().unwrap() {
                        value += 4;
                    }

                    arm.bus.write_word(address, value);
                }
            }
            (LoadStoreKind::Load, false) => {
                |arm: &mut Self, address: usize, reg_destination: usize| {
                    let v = arm.bus.read_word(address);
                    arm.registers.set_register_at(reg_destination, v);
                }
            }
            (LoadStoreKind::Load, true) => {
                // LDM with S bit and R15 not in list: Load into user mode registers
                |arm: &mut Self, address: usize, reg_destination: usize| {
                    let v = arm.bus.read_word(address);
                    arm.write_user_register(reg_destination, v);
                }
            }
        };

        // When STM includes the base register with writeback,
        // the value stored depends on the base's position in the register list:
        // - If base is FIRST in register list (lowest numbered): store original value
        // - If base is NOT first in register list: store modified (writeback) value
        // We handle this by temporarily updating the base register before transfers if needed.
        let base_in_list = reg_list.is_bit_on(base_register as u8);
        let restore_base =
            if write_back && base_in_list && load_store == LoadStoreKind::Store && !is_empty_list {
                // final writeback
                let num_registers = reg_list.count_ones();
                let final_address = match offsetting {
                    Offsetting::Up => memory_base.wrapping_add(num_registers * 4),
                    Offsetting::Down => memory_base.wrapping_sub(num_registers * 4),
                };

                // Check if base is the first register in the register list (lowest numbered)
                // This is independent of transfer order
                let first_register_in_list =
                    (0..=15).find(|&i| reg_list.is_bit_on(i as u8)).unwrap();

                // If base is NOT first in register list, store the writeback value
                if first_register_in_list == base_register {
                    None
                } else {
                    self.registers.set_register_at(base_register, final_address);
                    Some(memory_base) // Remember to restore after
                }
            } else {
                None
            };

        // Handle empty register list: Load/Store R15 and adjust base by 0x40
        if is_empty_list {
            // For empty register list, R15 is transferred and base is adjusted by 0x40
            let transfer_address = match (indexing, offsetting) {
                (Indexing::Post, Offsetting::Up) => address, // Transfer at current, then add 0x40
                (Indexing::Post, Offsetting::Down) => address.wrapping_sub(0x3C), // Transfer at base-0x3C, final base-0x40
                (Indexing::Pre, Offsetting::Up) => address.wrapping_add(4), // Increment first, transfer, final base+0x40
                (Indexing::Pre, Offsetting::Down) => address.wrapping_sub(0x40), // Decrement to final, transfer
            };

            transfer(self, transfer_address, 15);

            // Update base by 0x40 total
            address = match offsetting {
                Offsetting::Up => memory_base.wrapping_add(0x40) as usize,
                Offsetting::Down => memory_base.wrapping_sub(0x40) as usize,
            };
        } else {
            self.exec_data_transfer(reg_list, indexing, &mut address, offsetting, transfer);
        }

        // Restore base register if we temporarily modified it
        if let Some(original_value) = restore_base {
            self.registers
                .set_register_at(base_register, original_value);
        }

        // Writeback: Update base register with final address
        // Exception: On LDM, if base is in register list, the loaded value takes precedence
        let skip_writeback = write_back && load_store == LoadStoreKind::Load && base_in_list;

        if write_back && !skip_writeback {
            self.registers
                .set_register_at(base_register, (address & 0xFFFF_FFFF) as u32);
        }

        // If LDM with S bit and R15 is in register list: Copy SPSR to CPSR (exception return)
        if load_store == LoadStoreKind::Load && load_psr && r15_in_list {
            let spsr = self.spsr;
            self.cpsr = spsr;
        }

        // If LDM and R15 is in register list we flush the pipeline
        if load_store == LoadStoreKind::Load && r15_in_list {
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

    /// Executes a branch instruction (B/BL).
    ///
    /// # Panics
    ///
    /// Panics if PC conversion fails.
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
        match mul_variant {
            // Unsiged multiply (32-bit by 32-bit, bottom 32-bit result).
            ArmModeMultiplyVariant::Mul => {
                self.mul_or_mla(set_condition_codes, false, rd, rn, rs, rm);
            }
            // Unsiged multiply-accumulate (32-bit by 32-bit, bottom 32-bit accumulate and result).
            ArmModeMultiplyVariant::Mla => {
                self.mul_or_mla(set_condition_codes, true, rd, rn, rs, rm);
            }
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
        match mul_variant {
            // Unsigned long multiply (32-bit by 32-bit, 64-bit result)
            ArmModeMultiplyLongVariant::Umull => {
                self.umull_or_umlal(set_condition_codes, false, rdhi, rdlo, rs, rm);
            }
            // Unsigned long multiply-accumulate (32-bit by 32-bit, 64-bit accumulate and result)
            ArmModeMultiplyLongVariant::Umlal => {
                self.umull_or_umlal(set_condition_codes, true, rdhi, rdlo, rs, rm);
            }
            // Signed long multiply (32-bit by 32-bit, 64-bit result)
            ArmModeMultiplyLongVariant::Smull => {
                self.smull_or_smlal(set_condition_codes, false, rdhi, rdlo, rs, rm);
            }
            // Signed multiply-accumulate (32-bit by 32-bit, 64-bit accumulate and result)
            ArmModeMultiplyLongVariant::Smlal => {
                self.smull_or_smlal(set_condition_codes, true, rdhi, rdlo, rs, rm);
            }
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
            result = result_add;
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
    use crate::cpu::cpu_modes::Mode;
    use crate::cpu::flags::ShiftKind;
    use crate::cpu::psr::Psr;

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
                    alu_instruction: ArmModeAluInstr::Teq,
                    set_conditions: true,
                    op_kind: OperandKind::Immediate,
                    rn: 12,
                    destination: 0,
                    op2: AluSecondOperandInfo::Immediate { base: 1, shift: 0 }
                }
            );

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "TEQ R12, #1");
            }

            cpu.registers.set_register_at(12, 0xFFFF_FFFF);
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

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "MSREQ CPSR, R12");
            }
        }
        {
            let op_code = 0b1110_00_0_1001_1_1001_0011_000000000000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstr::Teq,
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
                alu_instruction: ArmModeAluInstr::Cmp,
                set_conditions: true,
                op_kind: OperandKind::Immediate,
                rn: 14,
                destination: 0,
                op2: AluSecondOperandInfo::Immediate { base: 0, shift: 0 }
            }
        );

        {
            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "CMP R14, #0");
        }
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        cpu.execute_arm(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.carry_flag());
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
                    alu_instruction: ArmModeAluInstr::Orr,
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

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "ORREQ R12, R12, #192");
            }
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
                    alu_instruction: ArmModeAluInstr::Mov,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 0,
                    destination: 14,
                    op2: AluSecondOperandInfo::Immediate { base: 4, shift: 0 }
                }
            );

            assert!(!cpu.cpsr.can_execute(op_code.condition));

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "MOVEQ R14, #4");
            }
        }
        {
            let op_code: u32 = 0b1110_00_1_1101_0_0000_0000_000011011111;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstr::Mov,
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

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "MOV R0, #223");
            }

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
                    alu_instruction: ArmModeAluInstr::Mov,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 0,
                    destination: 12,
                    op2: AluSecondOperandInfo::Immediate { base: 1, shift: 6 }
                }
            );

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "MOV R12, #67108864");
            }

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
                    alu_instruction: ArmModeAluInstr::Add,
                    set_conditions: false,
                    op_kind: OperandKind::Immediate,
                    rn: 15,
                    destination: 0,
                    op2: AluSecondOperandInfo::Immediate { base: 1, shift: 0 }
                }
            );

            {
                let asm = op_code.instruction.disassembler();
                assert_eq!(asm, "ADD R0, R15, #1");
            }

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
                alu_instruction: ArmModeAluInstr::Add,
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
                alu_instruction: ArmModeAluInstr::Add,
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
                alu_instruction: ArmModeAluInstr::Add,
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
                alu_instruction: ArmModeAluInstr::Mov,
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
                alu_instruction: ArmModeAluInstr::Mov,
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
                alu_instruction: ArmModeAluInstr::Add,
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
                alu_instruction: ArmModeAluInstr::And,
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
                alu_instruction: ArmModeAluInstr::Eor,
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
                    alu_instruction: ArmModeAluInstr::Tst,
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
                alu_instruction: ArmModeAluInstr::Bic,
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
                alu_instruction: ArmModeAluInstr::Mvn,
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
                alu_instruction: ArmModeAluInstr::Sub,
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
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());

        //Covers carry logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.registers.set_register_at(2, 15);
        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(1) as i32, -5);
        assert!(!cpu.cpsr.carry_flag());
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
                alu_instruction: ArmModeAluInstr::Sub,
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
        assert!(!cpu.cpsr.carry_flag());
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
                alu_instruction: ArmModeAluInstr::Adc,
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
                alu_instruction: ArmModeAluInstr::Adc,
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
                alu_instruction: ArmModeAluInstr::Adc,
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
                alu_instruction: ArmModeAluInstr::Adc,
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
                alu_instruction: ArmModeAluInstr::Adc,
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

    // NOTE: The old check_sbc test was deleted because it was written to match
    // the buggy SBC implementation. The correct behavior is now tested by
    // check_sbc_carry_flag_test105 which is based on the actual GBA test ROM.

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
                            register: 0,
                        },
                        field_mask: op_code.get_bits(16..=19),
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
                            register: 0,
                        },
                        field_mask: op_code.get_bits(16..=19),
                    }
                }
            );
            cpu.cpsr.set_mode(Mode::Fiq);

            // Trying to change MODE bits to a User mode
            cpu.registers.set_register_at(0, 0b1111 << 28 | (0b10000));

            cpu.execute_arm(op_code);

            // All flags set, mode bits should remain as Supervisor (0b10011)
            assert_eq!(u32::from(cpu.spsr), 0b1111 << 28 | 0b10011);
        }
        {
            // Test MRS with SPSR in User mode - should return CPSR instead of panicking
            // This tests the graceful fallback behavior for SPSR access in User/System modes
            let mut cpu = Arm7tdmi::default();
            // MRS R1, SPSR (but we're in User mode, so it should return CPSR)
            let op_code = 0b1110_00010_1_001111_0001_000000000000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::PSRTransfer {
                    condition: Condition::AL,
                    psr_kind: PsrKind::Spsr,
                    kind: PsrOpKind::Mrs {
                        destination_register: 1
                    }
                }
            );

            cpu.cpsr.set_mode(Mode::User);
            cpu.cpsr.set_carry_flag(true);
            cpu.cpsr.set_zero_flag(true);

            // This should NOT panic - instead it should return CPSR value
            cpu.execute_arm(op_code);

            // R1 should contain CPSR (with carry and zero flags set, User mode)
            let result = cpu.registers.register_at(1);
            assert!(result.get_bit(29)); // carry flag
            assert!(result.get_bit(30)); // zero flag
            assert_eq!(result.get_bits(0..=4), 0b10000); // User mode
        }
        {
            // Test MRS with SPSR in System mode - should also return CPSR
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_1_001111_0010_000000000000;
            let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);

            cpu.cpsr.set_mode(Mode::System);
            cpu.cpsr.set_sign_flag(true);
            cpu.cpsr.set_overflow_flag(true);

            cpu.execute_arm(op_code);

            let result = cpu.registers.register_at(2);
            assert!(result.get_bit(31)); // sign flag
            assert!(result.get_bit(28)); // overflow flag
            assert_eq!(result.get_bits(0..=4), 0b11111); // System mode
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "LDRB R12, #768");
            }
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "LDR R13, #208");
            }
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "LDR R13, #184");
            }
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "LDR R13, #208");
            }
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "LDRB R13, #24");
            }

            // because in this specific case address will be
            // then will be 0x03000050 (.wrapping_add(offset))
            cpu.registers.set_program_counter(0x03000050);

            // simulate mem already contains something.
            cpu.bus.write_byte(0x03000068, 99);

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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "STRB R4, #520");
            }
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "STR R1, #0");
            }
            cpu.registers.set_register_at(1, 16843008);

            // because in this specific case address will be
            // then will be 0x03000050 + 8 (.wrapping_sub(offset))
            cpu.registers.set_program_counter(0x03000050);

            cpu.execute_arm(op_code);

            let bus = cpu.bus;

            assert_eq!(bus.read_raw(0x01010100), 0);
            assert_eq!(bus.read_raw(0x01010100 + 1), 1);
            assert_eq!(bus.read_raw(0x01010100 + 2), 1);
            assert_eq!(bus.read_raw(0x01010100 + 3), 1);
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

            {
                let f = op_code.instruction.disassembler();
                assert_eq!(f, "STRB R13, #24");
            }

            // because in this specific case address will be
            // then will be 0x03000050 (.wrapping_add(offset))
            cpu.registers.set_program_counter(0x03000050);
            cpu.registers.set_register_at(13, 50);

            cpu.execute_arm(op_code);

            let memory = cpu.bus.internal_memory;

            assert_eq!(memory.read_at(0x03000068), 50);
            assert_eq!(cpu.registers.program_counter(), 0x03000050);
        }
    }

    #[test]
    fn check_ldr_word() {
        // Use EWRAM base address for tests (0x02000000)
        const EWRAM: u32 = 0x0200_0000;
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

        // Set PC to EWRAM base so the PC-relative load lands in EWRAM
        cpu.registers.set_program_counter(EWRAM);
        // simulate mem already contains something at PC + offset.
        // in u32 this is 16843009 00000001_00000001_00000001_00000001.
        // Address = PC + 40 = EWRAM + 40
        cpu.bus.write_word((EWRAM + 40) as usize, 0x01010101);
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(13), 16843009);
        assert_eq!(cpu.registers.program_counter(), EWRAM as usize);
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

    #[test]
    fn check_rotated_immediate_sets_carry_flag_logical() {
        // Test case: movs r0, #0xFF000000
        // This is encoded as immediate 0xFF rotated right by 8 bits (rotation field = 4)
        // When 0xFF is rotated right by 8, last bit shifted out (bit 7 of 0xFF) is 1
        // So carry should be SET

        // MOVS R0, #0xFF000000 (encoded as immediate=0xFF, rotation field=4 -> 8 bits)
        // Bits: 1110_00_1_1101_1_0000_0000_0100_11111111
        //       cond  I Op  S Rn   Rd   rot  immediate
        let op_code = 0b1110_00_1_1101_1_0000_0000_0100_11111111;
        let mut cpu = Arm7tdmi::default();
        cpu.cpsr.set_carry_flag(false); // Start with carry clear

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(0), 0xFF000000);
        assert!(
            cpu.cpsr.carry_flag(),
            "Carry flag should be SET when bit rotated out is 1"
        );
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_rotated_immediate_clears_carry_flag_logical() {
        // Test case: movs r0, #0xFF00
        // This is encoded as immediate 0xFF rotated right by 24 bits (rotation field = 12)
        // 0xFF ROR 24 = 0xFF << 8 = 0xFF00
        // Last bit shifted out (bit 23 of 0xFF) is 0, so carry should be CLEAR

        // MOVS R0, #0xFF00 (encoded as immediate=0xFF, rotation field=12 -> 24 bits)
        // Bits: 1110_00_1_1101_1_0000_0000_1100_11111111
        let op_code = 0b1110_00_1_1101_1_0000_0000_1100_11111111;
        let mut cpu = Arm7tdmi::default();
        cpu.cpsr.set_carry_flag(true); // Start with carry set

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        assert_eq!(cpu.registers.register_at(0), 0xFF00);
        assert!(
            !cpu.cpsr.carry_flag(),
            "Carry flag should be CLEAR when bit rotated out is 0"
        );
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_rotated_immediate_no_rotation_preserves_carry() {
        // Test case: movs r0, #0x55 (no rotation)
        // When rotation is 0, carry flag should NOT be affected

        // MOVS R0, #0x55 (immediate=0x55, rotation=0)
        // Bits: 1110_00_1_1101_1_0000_0000_0000_01010101
        let op_code = 0b1110_00_1_1101_1_0000_0000_0000_01010101;

        // Test with carry initially SET
        let mut cpu = Arm7tdmi::default();
        cpu.cpsr.set_carry_flag(true);
        let op_code_decoded: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code_decoded);

        assert_eq!(cpu.registers.register_at(0), 0x55);
        assert!(
            cpu.cpsr.carry_flag(),
            "Carry flag should be preserved (SET) when no rotation"
        );

        // Test with carry initially CLEAR
        let mut cpu2 = Arm7tdmi::default();
        cpu2.cpsr.set_carry_flag(false);
        let op_code_decoded2: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu2.execute_arm(op_code_decoded2);

        assert_eq!(cpu2.registers.register_at(0), 0x55);
        assert!(
            !cpu2.cpsr.carry_flag(),
            "Carry flag should be preserved (CLEAR) when no rotation"
        );
    }

    #[test]
    fn check_rotated_immediate_arithmetic_no_carry_update() {
        // Test case: ADDS (arithmetic operation with rotated immediate)
        // For arithmetic operations, rotated immediate should NOT update carry flag from rotation
        // The carry comes from the arithmetic operation itself, not the rotation

        // ADDS R0, R1, #0xFF000000 (R1 + rotated immediate)
        // immediate=0xFF, rotation field=4 (8 bits) -> 0xFF000000
        // Bits: 1110_00_1_0100_1_0001_0000_0100_11111111
        //       cond  I ADD  S Rn   Rd   rot  immediate
        let op_code = 0b1110_00_1_0100_1_0001_0000_0100_11111111;
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(1, 0x00000001);
        cpu.cpsr.set_carry_flag(false);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // Result should be 0x00000001 + 0xFF000000 = 0xFF000001
        assert_eq!(cpu.registers.register_at(0), 0xFF000001);
        // Carry flag from addition (no carry out from this addition)
        // Even though rotation would set carry to 1, arithmetic ops don't use rotation carry
        assert!(
            !cpu.cpsr.carry_flag(),
            "Arithmetic operation should not set carry from rotation"
        );
    }

    #[test]
    fn check_rotated_immediate_tst_sets_carry() {
        // Test TST instruction with rotated immediate
        // TST is a logical operation so it should update carry from rotation

        // TST R0, #0xFF000000 (immediate=0xFF, rotation field=4 -> 8 bits)
        // Bits: 1110_00_1_1000_1_0000_0000_0100_11111111
        //       cond  I TST  S Rn   SBZ  rot  immediate
        // Note: TST always has S=1 implicitly
        let op_code = 0b1110_00_1_1000_1_0000_0000_0100_11111111;
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(0, 0xFFFFFFFF);
        cpu.cpsr.set_carry_flag(false);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // TST doesn't modify registers, only flags
        // Result of AND is 0xFF000000 (non-zero)
        assert!(
            cpu.cpsr.carry_flag(),
            "TST with rotated immediate should set carry"
        );
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_rotated_immediate_and_instruction() {
        // Test AND instruction with S bit and rotated immediate

        // ANDS R2, R1, #0x80000000 (immediate=0x80, rotation=8)
        // 0x80 ROR 16 = 0x00800000, wait...
        // 0x80000000 = 0x02 ROR 2 or 0x80 ROR 8 (8*2=16 bits)
        // Let me use 0x02 rotated by 1 position: 0x02 ROR 2 = 0x80000000
        // Bits: 1110_00_1_0000_1_0001_0010_0001_00000010
        //       cond  I AND  S Rn   Rd   rot  immediate
        let op_code = 0b1110_00_1_0000_1_0001_0010_0001_00000010;
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(1, 0xFFFFFFFF);
        cpu.cpsr.set_carry_flag(false);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // 0x02 ROR 2 = 0x80000000
        assert_eq!(cpu.registers.register_at(2), 0x80000000);
        // Bit 1 of 0x02 is shifted out, which is 1
        assert!(
            cpu.cpsr.carry_flag(),
            "AND with rotated immediate should set carry from rotation"
        );
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_subs_pc_with_s_bit_test223() {
        // Test from t223: PC as destination with S bit
        // This tests that when doing `subs pc, imm` in FIQ mode with SPSR=SYS,
        // it should restore SPSR to CPSR and swap banked registers

        let mut cpu = Arm7tdmi::default();

        // Set r8 = 32 in Supervisor mode (default)
        cpu.registers.set_register_at(8, 32);

        // Switch to FIQ mode (swap_mode updates registers, then we update CPSR)
        cpu.swap_mode(Mode::Fiq);
        cpu.cpsr = Psr::from(Mode::Fiq);

        // Set r8_fiq = 64 (should be in banked register now)
        cpu.registers.set_register_at(8, 64);

        // Set SPSR_fiq to System mode
        cpu.spsr = Psr::from(Mode::System);

        // Set PC to some address
        cpu.registers.set_program_counter(0x0800_0100);

        // Execute SUBS R15, R15, #4 with condition AL
        // opcode: 1110_00_0_0010_1_1111_1111_000000000100
        //         cond  I SUB  S Rn   Rd   immediate
        let op_code = 0b1110_00_1_0010_1_1111_1111_000000000100;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // After execution:
        // 1. CPSR should be System mode (from SPSR)
        assert_eq!(cpu.cpsr.mode(), Mode::System, "CPSR should be System mode");

        // 2. r8 should be 32 (from System mode), not 64 (from FIQ mode)
        assert_eq!(
            cpu.registers.register_at(8),
            32,
            "r8 should be restored from System mode"
        );

        // 3. PC should be updated (original + 8 - 4 = original + 4)
        // But after pipeline flush, the exact value depends on implementation
    }

    #[test]
    fn check_sbc_carry_flag_test105() {
        // Test from t105 in flags.asm
        // SBC with carry clear should compute: Rn - Op2 - 1

        // Test 1: 2 - 0 - 1 = 1 (no borrow, carry should be SET)
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(0, 2);
        cpu.cpsr.set_carry_flag(false); // NOT(C) = 1
        // SBCS R0, R0, #0
        let op_code = 0b1110_00_1_0110_1_0000_0000_0000_00000000;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(0), 1, "Result should be 1");
        assert!(cpu.cpsr.carry_flag(), "Carry should be SET (no borrow)");

        // Test 2: 2 - 1 - 1 = 0 (no borrow, carry should be SET)
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(0, 2);
        cpu.cpsr.set_carry_flag(false); // NOT(C) = 1
        // SBCS R0, R0, #1
        let op_code = 0b1110_00_1_0110_1_0000_0000_0000_00000001;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(0), 0, "Result should be 0");
        assert!(cpu.cpsr.carry_flag(), "Carry should be SET (no borrow)");

        // Test 3: 2 - 2 - 1 = -1 (borrow, carry should be CLEAR)
        let mut cpu = Arm7tdmi::default();
        cpu.registers.set_register_at(0, 2);
        cpu.cpsr.set_carry_flag(false); // NOT(C) = 1
        // SBCS R0, R0, #2
        let op_code = 0b1110_00_1_0110_1_0000_0000_0000_00000010;
        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);
        assert_eq!(
            cpu.registers.register_at(0),
            0xFFFFFFFF,
            "Result should be -1"
        );
        assert!(
            !cpu.cpsr.carry_flag(),
            "Carry should be CLEAR (borrow occurred)"
        );
    }

    #[test]
    fn test_misaligned_word_load_rotation() {
        // Test for ARM7 misaligned load behavior (test 355)
        // When loading a word from a misaligned address, the result should be rotated
        let mut cpu = Arm7tdmi::default();

        // Store 0x00000020 at aligned address
        let mem = 0x03000000; // IWRAM
        cpu.bus.write_word(mem, 0x00000020);

        // First test the Arm7tdmi::read_word method directly
        // 0x00000020 ror 24 = 0x00002000 (8192)
        let direct_read = cpu.read_word(mem + 3);
        assert_eq!(
            direct_read, 0x00002000,
            "Direct read_word should rotate correctly"
        );

        // Now test via LDR instruction
        // LDR r1, [r11, #3]  - Load from misaligned address (offset by 3)
        // Expected: value should be rotated right by 24 bits
        // 0x00000020 ror 24 = 0x00002000
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(26..=27, 0b01); // Single data transfer
        op_code.set_bits(25..=25, 0); // Immediate offset
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(22..=22, 0); // Word
        op_code.set_bits(21..=21, 0); // No write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 11); // Base register (r11)
        op_code.set_bits(12..=15, 1); // Destination register (r1)
        op_code.set_bits(0..=11, 3); // Offset of 3

        cpu.registers.set_register_at(11, mem as u32);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        let result = cpu.registers.register_at(1);

        // Check that r1 contains the rotated value
        assert_eq!(
            result, 0x00002000,
            "Misaligned load should rotate value: 0x00000020 ror 24 = 0x00002000"
        );
    }

    #[test]
    fn test_str_writeback_same_register() {
        // Test for ARM7 STR with writeback to same register (test 358)
        // When STR writes back to the same register being stored, the OLD value
        // (before writeback) should be stored, not the new value after writeback
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000;

        // STR r0, [r0, #4]! - Pre-indexed with writeback
        // opcode: 0xE5A00004
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(26..=27, 0b01); // Single data transfer
        op_code.set_bits(25..=25, 0); // Immediate offset
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(22..=22, 0); // Word
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 0); // Store
        op_code.set_bits(16..=19, 0); // Base register (r0)
        op_code.set_bits(12..=15, 0); // Source register (r0)
        op_code.set_bits(0..=11, 4); // Offset of 4

        cpu.registers.set_register_at(0, mem);

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // After execution:
        // 1. r0 should be updated to mem + 4 (writeback)
        assert_eq!(
            cpu.registers.register_at(0),
            mem + 4,
            "r0 should be written back to mem + 4"
        );

        // 2. The value stored at [mem + 4] should be the OLD value of r0 (mem)
        let stored_value = cpu.bus.read_word((mem + 4) as usize);
        assert_eq!(
            stored_value, mem,
            "Stored value should be OLD r0 value (before writeback)"
        );
    }

    #[test]
    fn test_misaligned_halfword_load_rotation() {
        // Test for ARM7 misaligned halfword load behavior (test 408)
        // When loading a halfword from a misaligned address, the result should be rotated
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000;

        // Store halfword 32 (0x0020) at aligned address
        cpu.bus.write_half_word(mem, 32);

        // Test direct read_half_word method
        // Load from mem+1 (misaligned), should rotate by 8 bits
        // 0x00000020 ror 8 = 0x20000000 (536870912)
        let result = cpu.read_half_word(mem + 1, false);
        assert_eq!(
            result, 0x20000000,
            "Misaligned halfword load should rotate: 0x0020 ror 8 = 0x20000000"
        );
    }

    #[test]
    fn test_misaligned_swap() {
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000; // IWRAM

        // store 64 at aligned address
        cpu.bus.write_word(mem, 64);

        // SWP r3, r0, [r2] where r0=32, r2=mem+1 (misaligned by 1 byte)
        // r3 should contain 64 rotated right by 8 bits = 0x40000000
        // after swap, reading from mem should give 32

        cpu.registers.set_register_at(0, 32); // r0 = 32
        cpu.registers.set_register_at(2, (mem + 1) as u32); // r2 = mem+1 (misaligned)

        // build SWP instruction: swp r3, r0, [r2]
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(23..=27, 0b00010); // Single data swap
        op_code.set_bits(22..=22, 0); // Word (not byte)
        op_code.set_bits(16..=19, 2); // Rn = r2 (base register)
        op_code.set_bits(12..=15, 3); // Rd = r3 (destination)
        op_code.set_bits(4..=7, 0b1001); // Fixed bits for swap
        op_code.set_bits(0..=3, 0); // Rm = r0 (source)

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // r3 contains the rotated value: 64 ror 8 = 0x40000000
        assert_eq!(
            cpu.registers.register_at(3),
            0x40000000,
            "Misaligned swap should rotate read value: 64 ror 8 = 0x40000000"
        );

        // reading from aligned address gives 32
        let mem_value = cpu.bus.read_word(mem);
        assert_eq!(
            mem_value, 32,
            "After swap, memory at aligned address should contain 32"
        );
    }

    #[test]
    fn test_block_transfer_store_user_registers() {
        // Test for ARM7 block transfer with S bit: Store user registers (test 511)
        // When in FIQ mode and using STM with S bit (^), should store user mode registers
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000; // IWRAM

        // Set system mode r8 = 32
        cpu.registers.set_register_at(8, 32);

        // Switch to FIQ mode
        cpu.swap_mode(Mode::Fiq);
        // Set FIQ mode r8 = 64 (banked register)
        cpu.registers.set_register_at(8, 64);

        // Build STMFD instruction with S bit: stmfd r0, {r8, r9}^
        // This should store user mode r8, r9 (not FIQ banked)
        cpu.registers.set_register_at(0, mem as u32);

        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 0); // Down
        op_code.set_bits(22..=22, 1); // S bit set (load_psr)
        op_code.set_bits(21..=21, 0); // No write-back
        op_code.set_bits(20..=20, 0); // Store
        op_code.set_bits(16..=19, 0); // Base register r0
        op_code.set_bits(8..=9, 0b11); // Register list: r8, r9

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // Switch back to system mode and check stored values
        cpu.swap_mode(Mode::System);
        let stored_r8 = cpu.bus.read_word(mem - 8);

        assert_eq!(
            stored_r8, 32,
            "STMFD with S bit should store user mode r8 (32), not FIQ mode r8 (64)"
        );
    }

    #[test]
    fn test_block_transfer_load_user_registers() {
        // Test for ARM7 block transfer with S bit: Load user registers (test 512)
        // When in FIQ mode and using LDM with S bit (^), should load into user mode registers
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000; // IWRAM

        // Store value 0xA at mem-4 (where r9 will be loaded from with LDMFD descending)
        cpu.bus.write_word(mem - 4, 0xA);

        // Switch to FIQ mode and set FIQ r8 and r9 = 0xB
        cpu.swap_mode(Mode::Fiq);
        cpu.registers.set_register_at(8, 0xB);
        cpu.registers.set_register_at(9, 0xB);

        // Build LDMFD instruction with S bit: ldmfd r0, {r8, r9}^
        // LDMFD with base=mem loads r9 from mem-4, r8 from mem-8 (pre-indexed, down, reversed order)
        // This should load into user mode r8, r9 (not FIQ banked)
        cpu.registers.set_register_at(0, mem as u32);

        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 0); // Down
        op_code.set_bits(22..=22, 1); // S bit set (load_psr)
        op_code.set_bits(21..=21, 0); // No write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 0); // Base register r0
        op_code.set_bits(8..=9, 0b11); // Register list: r8, r9

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // FIQ mode r9 should still be 0xB (unchanged)
        assert_eq!(
            cpu.registers.register_at(9),
            0xB,
            "FIQ mode r9 should remain unchanged (0xB)"
        );

        // Switch to system mode and check user mode r9
        cpu.swap_mode(Mode::System);
        assert_eq!(
            cpu.registers.register_at(9),
            0xA,
            "User mode r9 should now be 0xA (loaded from memory)"
        );
    }

    #[test]
    fn test_block_transfer_empty_register_list_ldmia() {
        // Test for empty register list with LDMIA (test 513)
        // LDMIA r0!, {} should load R15 from [r0] and increment r0 by 0x40
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000;

        // Store a test value at mem (this would be loaded into R15/PC)
        cpu.bus.write_word(mem, 0x08000100);

        // Set r0 to mem
        cpu.registers.set_register_at(0, mem as u32);

        // Build LDMIA r0!, {} instruction
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(22..=22, 0); // No S bit
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 0); // Base register r0
        op_code.set_bits(0..=15, 0); // Empty register list

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // r0 should be incremented by 0x40
        assert_eq!(
            cpu.registers.register_at(0),
            (mem + 0x40) as u32,
            "r0 should be incremented by 0x40 after LDMIA with empty list"
        );
    }

    #[test]
    fn test_block_transfer_empty_register_list_stmda() {
        // Test for empty register list with STMDA (test 530)
        // STMDA r0!, {} should store PC at [r0 - 0x3C] and set r0 = r0 - 0x40
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000100;

        cpu.registers.set_register_at(0, mem as u32);

        // Build STMDA r0!, {} instruction
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 0); // Down
        op_code.set_bits(22..=22, 0); // No S bit
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 0); // Store
        op_code.set_bits(16..=19, 0); // Base register r0
        op_code.set_bits(0..=15, 0); // Empty register list

        let pc_before = cpu.registers.program_counter();

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // PC should be stored at [mem - 0x3C]
        let stored_pc = cpu.bus.read_word(mem - 0x3C);
        assert_eq!(
            stored_pc,
            (pc_before + 4) as u32, // PC+4 for STM
            "PC+4 should be stored at [base - 0x3C] for STMDA with empty list"
        );

        // r0 should be decremented by 0x40
        assert_eq!(
            cpu.registers.register_at(0),
            (mem - 0x40) as u32,
            "r0 should be decremented by 0x40 after STMDA with empty list"
        );
    }

    #[test]
    fn test_block_transfer_empty_list_loads_pc() {
        // Test that empty register list loads R15 and jumps (test 513/514)
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000;
        let target_address = 0x08000100;

        // Store target address at [mem]
        cpu.bus.write_word(mem, target_address);

        // Set r0 to mem
        cpu.registers.set_register_at(0, mem as u32);

        let pc_before = cpu.registers.program_counter();

        // Build LDMIA r0!, {} instruction
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100);
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(22..=22, 0); // No S bit
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 0); // Base register r0
        op_code.set_bits(0..=15, 0); // Empty register list

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // PC should have changed to target_address
        let pc_after = cpu.registers.program_counter();
        assert_ne!(
            pc_after, pc_before,
            "PC should have changed after loading R15 from empty list"
        );

        // r0 should be incremented by 0x40
        assert_eq!(
            cpu.registers.register_at(0),
            (mem + 0x40) as u32,
            "r0 should be incremented by 0x40 even though PC changed"
        );
    }

    #[test]
    fn test_block_transfer_empty_list_test513_514() {
        // Exact simulation of tests 513/514 from arm.gba
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000;

        // Simulate t513: adr r0, t514; str r0, [mem]; mov r0, mem
        let t514_address = 0x08001000; // Simulated address of t514
        cpu.bus.write_word(mem, t514_address);
        cpu.registers.set_register_at(0, mem as u32);

        // Execute: ldmia r0!, {} (0xE8B00000)
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, 0xE); // AL
        op_code.set_bits(25..=27, 0b100); // Block transfer
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(22..=22, 0); // No S bit
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 0); // Rn = r0
        op_code.set_bits(0..=15, 0); // Empty reg list

        assert_eq!(
            op_code, 0xE8B00000,
            "Instruction encoding should match test"
        );

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // Simulate t514: sub r0, 0x40; cmp r0, mem
        let r0_after_ldm = cpu.registers.register_at(0);
        let r0_after_sub = r0_after_ldm.wrapping_sub(0x40);

        assert_eq!(
            r0_after_sub,
            mem as u32,
            "After sub r0, 0x40, r0 should equal mem. r0_after_ldm=0x{:08X}, expected=0x{:08X}",
            r0_after_ldm,
            (mem + 0x40) as u32
        );
    }

    #[test]
    fn test_block_transfer_base_in_list_first() {
        // Test 516: Load writeback base first in rlist
        // When base register is in the list, loaded value takes precedence over writeback
        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000;

        // Store r0=0xA and r2 to memory
        cpu.registers.set_register_at(0, 0xA);
        cpu.registers.set_register_at(1, mem as u32);

        // STMFD r1!, {r0, r2} - Store r0 and r2, decrement r1
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100);
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 0); // Down
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 0); // Store
        op_code.set_bits(16..=19, 1); // Base r1
        op_code.set_bits(0..=0, 1); // r0
        op_code.set_bits(2..=2, 1); // r2

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        let r1_after_store = cpu.registers.register_at(1);

        // LDMFA r1!, {r1, r2} = LDMIA r1!, {r1, r2}
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100);
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 1); // Base r1
        op_code.set_bits(1..=1, 1); // r1
        op_code.set_bits(2..=2, 1); // r2

        assert_eq!(op_code, 0xE8B10006, "Should match test instruction");

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // r1 should be 0xA (loaded value), not the writeback address
        assert_eq!(
            cpu.registers.register_at(1),
            0xA,
            "r1 should be loaded value (0xA), not writeback. r1_after_store was 0x{:08X}",
            r1_after_store
        );
    }

    #[test]
    fn test_stmfd_base_first_in_register_list() {
        // Test 518: STMFD base first in rlist
        // When base register is first in register list (lowest numbered),
        // the OLD value (before writeback) should be stored
        use crate::cpu::condition::Condition;

        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000u32;

        // Setup: r0 = mem, r1 = 0xA
        cpu.registers.set_register_at(0, mem);
        cpu.registers.set_register_at(1, 0xA);

        // Execute: stmfd r0!, {r0, r1}
        // This should store OLD r0 (mem) and r1 (0xA) with writeback
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 0); // Down
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 0); // Store
        op_code.set_bits(16..=19, 0); // Base r0
        op_code.set_bits(0..=0, 1); // r0 in list
        op_code.set_bits(1..=1, 1); // r1 in list

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // r0 should be updated by writeback (mem - 8)
        let r0_after = cpu.registers.register_at(0);
        assert_eq!(r0_after, mem - 8, "r0 should be written back to mem - 8");

        // Now load back and verify
        // Execute: ldmfd r0!, {r1, r2}
        // LDMFD = LDMIA (Increment After) - opposite of STMFD
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 0); // Base r0
        op_code.set_bits(1..=1, 1); // r1 in list
        op_code.set_bits(2..=2, 1); // r2 in list

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        let r1_loaded = cpu.registers.register_at(1);

        // The key assertion: r1 should contain the OLD value of r0 (mem)
        // not the writeback value (mem - 8)
        assert_eq!(
            r1_loaded, mem,
            "Test 518: r1 should equal original mem value (0x{:08X}), got 0x{:08X}. \
             When base is first in register list, OLD value should be stored.",
            mem, r1_loaded
        );
    }

    #[test]
    fn test_stmfd_base_not_first_in_register_list() {
        // Test 519: STMED base first in rlist (same as STMFD behavior)
        // When base register is NOT first in register list,
        // the writeback value should be stored
        use crate::cpu::condition::Condition;

        let mut cpu = Arm7tdmi::default();
        let mem = 0x03000000u32;

        // Setup: r1 = mem, r0 = 0xA
        cpu.registers.set_register_at(1, mem);
        cpu.registers.set_register_at(0, 0xA);

        // Execute: stmfd r1!, {r0, r1}
        // r1 is base but NOT first (r0 comes first)
        // This should store writeback value for r1
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 1); // Pre-indexed
        op_code.set_bits(23..=23, 0); // Down
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 0); // Store
        op_code.set_bits(16..=19, 1); // Base r1
        op_code.set_bits(0..=0, 1); // r0 in list
        op_code.set_bits(1..=1, 1); // r1 in list

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        // r1 should be updated by writeback (mem - 8)
        let r1_after = cpu.registers.register_at(1);
        assert_eq!(r1_after, mem - 8, "r1 should be written back to mem - 8");

        // Now load back and verify
        // Execute: ldmfd r1!, {r1, r2}
        let mut op_code = 0u32;
        op_code.set_bits(28..=31, Condition::AL as u32);
        op_code.set_bits(25..=27, 0b100); // Block data transfer
        op_code.set_bits(24..=24, 0); // Post-indexed
        op_code.set_bits(23..=23, 1); // Up
        op_code.set_bits(21..=21, 1); // Write-back
        op_code.set_bits(20..=20, 1); // Load
        op_code.set_bits(16..=19, 1); // Base r1
        op_code.set_bits(1..=1, 1); // r1 in list
        op_code.set_bits(2..=2, 1); // r2 in list

        let op_code: ArmModeOpcode = Arm7tdmi::decode(op_code);
        cpu.execute_arm(op_code);

        let r2_loaded = cpu.registers.register_at(2);

        // The key assertion: r2 should contain the writeback value (mem - 8)
        // not the original value (mem), since r1 was NOT first in the register list
        // (r2 loaded from mem-4, where r1's value was stored)
        assert_eq!(
            r2_loaded,
            mem - 8,
            "Test 519: When base is NOT first in register list, writeback value should be stored. \
             Expected 0x{:08X}, got 0x{:08X}",
            mem - 8,
            r2_loaded
        );
    }
}
