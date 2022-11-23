use crate::{
    arm::alu_instruction::{AluInstructionKind, ArmModeAluInstruction, Kind},
    arm::arm7tdmi::Arm7tdmi,
    arm::opcode::ArmModeOpcode,
    bitwise::Bits,
};

use super::{arm7tdmi::REG_PROGRAM_COUNTER, cpu_modes::Mode};

pub struct ArithmeticOpResult {
    result: u32,
    pub carry: bool,
    pub overflow: bool,
    pub sign: bool,
    pub zero: bool,
}

/// Represents the kind of PSR operation
enum PsrOpKind {
    /// MSR operation (transfer PSR contents to a register)
    Mrs,
    /// MSR operation (transfer register contents to PSR)
    Msr,
    /// MSR flags operation (transfer register contents or immediate value to PSR flag bits only)
    MsrFlg,
}

impl Arm7tdmi {
    fn shift_operand(
        &mut self,
        alu_opcode: u32,
        s: bool,
        shift_type: u32,
        shift_amount: u32,
        rm: u32,
    ) -> u32 {
        // Shift Type (0=LSL, 1=LSR, 2=ASR, 3=ROR)
        let mut carry: bool = self.cpsr.carry_flag();

        let result = match shift_type {
            // LSL
            0 => {
                match shift_amount {
                    // LSL#0: No shift performed, ie. directly value=Rm, the C flag is NOT affected.
                    0 => rm,
                    // LSL#1..32: Normal left logical shift
                    1..=32 => {
                        carry = rm.get_bit((32 - shift_amount).try_into().unwrap());

                        rm << shift_amount
                    }
                    // LSL#33...: Result is 0 and carry is 0
                    _ => {
                        carry = false;

                        0
                    }
                }
            }
            // LSR
            1 => {
                match shift_amount {
                    // LSR#0 is used to encode LSR#32, it has 0 result and carry equal to bit 31 of Rm
                    0 => {
                        carry = rm.get_bit(31);

                        0
                    }
                    // LSR#1..32: Normal right logical shift
                    1..=32 => {
                        carry = rm.get_bit((shift_amount - 1).try_into().unwrap());

                        rm >> shift_amount
                    }
                    // LSR#33...: result is 0 and carry is 0
                    _ => {
                        carry = false;

                        0
                    }
                }
            }
            //ASR
            2 => {
                match shift_amount {
                    // ASR#1..31: normal arithmetic right shift
                    1..=31 => {
                        carry = rm.get_bit((shift_amount - 1).try_into().unwrap());

                        ((rm as i32) >> shift_amount) as u32
                    }
                    // ASR#0 (which is used to encode ASR#32), ASR#32 and above all have the same result
                    _ => {
                        carry = rm.get_bit(31);
                        // arithmetically shifting by 31 is the same as shifting by 32, but with 32 rust complains
                        ((rm as i32) >> 31) as u32
                    }
                }
            }
            // ROR
            3 => {
                // from documentation: ROR by n where n is greater than 32 will give the same
                // result and carry out as ROR by n-32; therefore repeatedly y subtract 32 from n until the amount is
                // in the range 1 to 32
                let mut new_shift_amount = shift_amount;

                if shift_amount > 32 {
                    new_shift_amount %= 32;

                    // if modulo operation yields 0 it means that shift_amount was a multiple of 32
                    // so we should do ROR#32
                    if new_shift_amount == 0 {
                        new_shift_amount = 32;
                    }
                }

                match new_shift_amount {
                    // ROR#0 is used to encode RRX (appending C to the left and shift right by 1)
                    0 => {
                        let old_carry = self.cpsr.carry_flag() as u32;

                        carry = rm.get_bit(0);

                        (rm >> 1) | (old_carry << 31)
                    }
                    // ROR#1..31: normal rotate right
                    1..=31 => {
                        carry = rm.get_bit((shift_amount - 1).try_into().unwrap());

                        rm.rotate_right(shift_amount)
                    }
                    // ROR#32 doesn't change rm but sets carry to bit 31 of rm
                    32 => {
                        carry = rm.get_bit(31);

                        rm
                    }
                    // ROR#i with i > 32 is the same of ROR#n where n = i % 32
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        };

        // If the instruction is a logical ALU instruction and S is set we set the carry flag
        if ArmModeAluInstruction::from(alu_opcode).kind() == AluInstructionKind::Logical && s {
            self.cpsr.set_carry_flag(carry);
        }

        result
    }

    fn get_operand(&mut self, alu_opcode: u32, s: bool, i: bool, op2: u32) -> u32 {
        match i {
            // we get the operand from a register and then we shift it
            false => {
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
                // bits [6-5] - Shift Type (0=LSL, 1=LSR, 2=ASR, 3=ROR)
                let shift_type = op2.get_bits(5..=6);

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

                self.shift_operand(alu_opcode, s, shift_type, shift_amount, rm)
            }
            true => {
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
    /// # Arguments
    ///
    /// * `i` - A boolean value representing whether the 2nd operand is immediate or not
    /// * `r` - A boolean value representing whether the shift amount is to be taken from register or not
    const fn get_pc_offset_alu(&self, i: bool, r: bool) -> u32 {
        if !i && r {
            12
        } else {
            8
        }
    }

    fn psr_transfer(&mut self, op_code: ArmModeOpcode) {
        let psr_kind = if op_code.get_bits(23..=27) == 0b00010
            && op_code.get_bits(16..=21) == 0b001111
            && op_code.get_bits(0..=11) == 0b0000_0000_0000
        {
            PsrOpKind::Mrs
        } else if op_code.get_bits(23..=27) == 0b00010
            && op_code.get_bits(12..=21) == 0b10_1001_1111
            && op_code.get_bits(4..=11) == 0b0000_0000
        {
            PsrOpKind::Msr
        } else if op_code.get_bits(26..=27) == 0b00
            && op_code.get_bits(23..=24) == 0b10
            && op_code.get_bits(12..=21) == 0b10_1000_1111
        {
            PsrOpKind::MsrFlg
        } else {
            unreachable!()
        };

        // P = 0 means CPSR, P = 1 means SPSR_mode
        let p = op_code.get_bit(22);

        match psr_kind {
            PsrOpKind::Mrs => {
                let rd = op_code.get_bits(12..=15);

                if rd == REG_PROGRAM_COUNTER {
                    panic!("PSR transfer should not use R15 as source/destination");
                }

                let psr = match p {
                    false => self.cpsr,
                    true => self.get_spsr(),
                };

                self.registers
                    .set_register_at(rd.try_into().unwrap(), psr.into());
            }
            PsrOpKind::Msr => {
                let rm = op_code.get_bits(0..=3);
                if rm == REG_PROGRAM_COUNTER {
                    panic!("PSR transfer should not use R15 as source/destination");
                }

                let rm = self.registers.register_at(rm.try_into().unwrap());

                let current_mode = self.cpsr.mode();

                let psr = match p {
                    false => &mut self.cpsr,
                    true => self.get_spsr_as_ref_mut(),
                };

                // Setting flags
                psr.set_sign_flag(rm.get_bit(31));
                psr.set_zero_flag(rm.get_bit(30));
                psr.set_carry_flag(rm.get_bit(29));
                psr.set_overflow_flag(rm.get_bit(28));

                // In User mode we can only set the flags so we don't touch the Mode bits
                if current_mode != Mode::User {
                    psr.set_irq_disable(rm.get_bit(7));
                    psr.set_fiq_disable(rm.get_bit(6));

                    // Documentation says that software should never touch T (state) bit
                    // Should we set it? I guess software are written in order to not switch this bit
                    // but who knows?
                    // psr.set_state_bit(rm.get_bit(5));

                    psr.set_mode(Mode::try_from(rm.get_bits(0..=4)).unwrap())
                }
            }
            PsrOpKind::MsrFlg => {
                // Immediate: 0 register, 1 immediate
                let i = op_code.get_bit(25);

                let op = match i {
                    false => {
                        let rm = op_code.get_bits(0..=3);
                        self.registers.register_at(rm.try_into().unwrap())
                    }
                    true => {
                        let imm = op_code.get_bits(0..=7);
                        let rotate = op_code.get_bits(8..=11);

                        // FIXME: Is the *2 correct? Documentation doesn't specify it but
                        // GBATEK says: 11-8    Shift applied to Imm   (ROR in steps of two 0-30)
                        imm.rotate_right(rotate * 2)
                    }
                };

                let psr = match p {
                    false => &mut self.cpsr,
                    true => self.get_spsr_as_ref_mut(),
                };

                // Setting flags
                psr.set_sign_flag(op.get_bit(31));
                psr.set_zero_flag(op.get_bit(30));
                psr.set_carry_flag(op.get_bit(29));
                psr.set_overflow_flag(op.get_bit(28));
            }
        }
    }

    pub fn data_processing(&mut self, op_code: ArmModeOpcode) -> bool {
        // bit [25] is I = Immediate Flag
        let i: bool = op_code.get_bit(25);
        // bits [24-21]
        let alu_op_code = op_code.get_bits(21..=24);
        // bit [20] is sets condition codes
        let s = op_code.get_bit(20);
        // bits [15-12] are the Rd
        let rd = op_code.get_bits(12..=15);
        // bits [19-16] are the Rn
        let rn = op_code.get_bits(16..=19);
        let offset = match rn {
            // if Rn is R15(PC) we need to offset its value because of
            // instruction pipelining
            REG_PROGRAM_COUNTER => self.get_pc_offset_alu(i, op_code.get_bit(4)),
            _ => 0,
        };

        let rn = self.registers.register_at(rn.try_into().unwrap()) + offset;

        let op2 = self.get_operand(alu_op_code, s, i, op_code.get_bits(0..=11));

        // S = 1 and Rd = 0xF should not be allowed in User Mode.
        // TODO: When in other modes it should load SPSR_<current_mode> into CPSR
        if s && rd == REG_PROGRAM_COUNTER {
            unimplemented!("Implement cases when S=1 and Rd=0xF");
        }

        match ArmModeAluInstruction::from(alu_op_code) {
            ArmModeAluInstruction::And => self.and(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Eor => self.eor(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Sub => self.sub(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Rsb => self.rsb(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Add => self.add(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Adc => self.adc(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Sbc => self.sbc(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Rsc => self.rsc(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Tst => {
                if s {
                    self.tst(rn, op2)
                } else {
                    self.psr_transfer(op_code);

                    return true;
                }
            }
            ArmModeAluInstruction::Teq => {
                if s {
                    self.teq(rn, op2)
                } else {
                    self.psr_transfer(op_code);

                    return true;
                }
            }
            ArmModeAluInstruction::Cmp => {
                if s {
                    self.cmp(rn, op2)
                } else {
                    self.psr_transfer(op_code);

                    return true;
                }
            }
            ArmModeAluInstruction::Cmn => {
                if s {
                    self.cmn(rn, op2)
                } else {
                    self.psr_transfer(op_code);

                    return true;
                }
            }
            ArmModeAluInstruction::Orr => self.orr(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Mov => self.mov(rd.try_into().unwrap(), op2, s),
            ArmModeAluInstruction::Bic => self.bic(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Mvn => self.mvn(rd.try_into().unwrap(), op2, s),
        };

        // If is a "test" ALU instruction we ever advance PC.
        match ArmModeAluInstruction::from(alu_op_code) {
            ArmModeAluInstruction::Teq
            | ArmModeAluInstruction::Cmp
            | ArmModeAluInstruction::Cmn
            | ArmModeAluInstruction::Tst => true,
            _ => rd != 0xF,
        }
    }

    fn and(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result = rn & op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.get_bit(31));
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

    fn eor(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
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

    fn add_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
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
    fn sub_inner_op(first_op: u32, second_op: u32) -> ArithmeticOpResult {
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

    fn tst(&mut self, rn: u32, op2: u32) {
        let value = rn & op2;

        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    fn teq(&mut self, rn: u32, op2: u32) {
        let value = rn ^ op2;
        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    fn cmp(&mut self, rn: u32, op2: u32) {
        let sub_result = Self::sub_inner_op(rn, op2);

        self.cpsr.set_flags(sub_result);
    }

    fn cmn(&mut self, rn: u32, op2: u32) {
        let add_result = Self::add_inner_op(rn, op2);

        self.cpsr.set_flags(add_result);
    }

    fn orr(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
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

    fn mvn(&mut self, rd: usize, op2: u32, s: bool) {
        let result = !op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_sign_flag(result.get_bit(31));
            self.cpsr.set_zero_flag(result == 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        arm::arm7tdmi::Arm7tdmi,
        arm::condition::Condition,
        arm::{cpu_modes::Mode, instruction::ArmModeInstruction},
        cpu::Cpu,
    };

    #[test]
    fn check_teq() {
        let op_code = 0b1110_0001_0011_1001_0011_0000_0000_0000;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing);
        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 100);
        cpu.cpsr.set_sign_flag(true); // set for later verify.
        cpu.execute(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_cmp() {
        let op_code: u32 = 0b1110_0001_0011_1010_0011_0000_0000_0000;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing);
        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 1);
        cpu.execute(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_add() {
        let op_code = 0b1110_0010_1000_1111_0000_0000_0010_0000;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing);
        cpu.registers.set_register_at(15, 15);
        cpu.execute(op_code);
        assert_eq!(cpu.registers.register_at(0), 15 + 8 + 32);
    }

    #[test]
    fn check_add_pc_operand_shift_register() {
        // Case when R15 is used as operand and shift amount is taken from register:
        // R2 = R1 + (R15 << R3)
        let op_code = 0b1110_0000_1000_0001_0010_0011_0001_1111;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing);

        cpu.registers.set_register_at(2, 5);
        cpu.registers.set_register_at(1, 10);
        cpu.registers.set_register_at(15, 500);
        cpu.registers.set_register_at(3, 0);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(2), 500 + 12 + 10);
    }

    #[test]
    fn check_add_carry_bit() {
        let op_code: u32 = 0b1110_0000_1001_1111_0000_0000_0000_1110;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing);

        cpu.registers.set_register_at(15, 1 << 31);
        cpu.registers.set_register_at(14, 1 << 31);
        cpu.execute(op_code);
        assert_eq!(cpu.registers.register_at(0), 8);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_mov_rx_immediate() {
        // MOV R0, 0
        let mut op_code: u32 = 0b1110_0011_1010_0000_0000_0000_0000_0000;

        // bits [11-8] are ROR-Shift applied to nn
        let is = op_code & 0b0000_0000_0000_0000_0000_1111_0000_0000;

        // MOV Rx,x
        let mut cpu = Arm7tdmi::default();
        for rx in 0..=0xF {
            let register_for_op = rx << 12;
            let immediate_value = rx;

            // Rd parameter
            op_code = (op_code & 0b1111_1111_1111_1111_0000_1111_1111_1111) + register_for_op;
            // Immediate parameter
            op_code = (op_code & 0b1111_1111_1111_1111_1111_1111_0000_0000) + immediate_value;

            let op_code = cpu.decode(op_code);
            assert_eq!(op_code.condition, Condition::AL);
            assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing);

            cpu.execute(op_code);
            let rotated = rx.rotate_right(is * 2);
            assert_eq!(cpu.registers.register_at(rx.try_into().unwrap()), rotated);
        }
    }

    #[test]
    fn check_mov_cpsr() {
        // Checks for Z flag
        let op_code = 0b1110_00_0_1101_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert!(cpu.cpsr.zero_flag());

        // Checks for Z flag
        let op_code = 0b1110_00_0_1101_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(2, -5_i32 as u32);
        cpu.execute(op_code);

        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn shift_from_register_is_0() {
        let op_code = 0b1110_0000_1000_0000_0001_0011_0111_0010;
        let mut cpu = Arm7tdmi::default();

        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 5);
        cpu.registers.set_register_at(2, 11);
        cpu.registers.set_register_at(3, 8 << 8);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 16);
    }

    #[test]
    fn check_and() {
        let op_code = 0b1110_00_1_0000_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        // All 1 except msb
        cpu.registers.set_register_at(0, 2_u32.pow(31) - 1);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b10101010);
    }

    #[test]
    fn check_eor() {
        let op_code = 0b1110_00_1_0001_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b01010101);
    }

    #[test]
    fn check_tst() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b1110_00_1_1000_1_0000_0001_0000_00000000;
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_sign_flag(true);

        cpu.execute(op_code);

        assert!(cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_bic() {
        let op_code = 0b1110_00_1_1110_0_0000_0001_0000_10101010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 0b11111111);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0b01010101);
    }

    #[test]
    fn check_mvn() {
        let op_code = 0b1110_00_1_1111_1_0000_0001_0000_11111111;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), (2_u32.pow(24) - 1) << 8);
        assert!(cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_sub() {
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 10);
        cpu.registers.set_register_at(2, 5);
        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 5);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.sign_flag());

        //Covers carry logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code = cpu.decode(op_code);
        cpu.registers.set_register_at(2, 15);
        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1) as i32, -5);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());

        // Covers overflow logic
        let op_code = 0b1110_00_0_0010_1_0000_0001_00000_00_0_0010;
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 1);
        cpu.registers.set_register_at(2, i32::MIN as u32);

        cpu.execute(op_code);

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
        let op_code = cpu.decode(op_code);

        cpu.registers.set_register_at(0, 1);
        cpu.registers.set_register_at(2, 1);
        cpu.cpsr.set_carry_flag(true);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 3);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers carry during first sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);
        cpu.registers.set_register_at(0, u32::MAX);
        cpu.registers.set_register_at(2, 1);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 1);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers carry during second sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);
        cpu.registers.set_register_at(0, u32::MAX - 1);
        cpu.registers.set_register_at(2, 1);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 0);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers overflow during first sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        // All 1 except MSB
        cpu.registers.set_register_at(0, i32::MAX as u32);
        cpu.registers.set_register_at(2, 1);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), (1 << 31) + 1);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers overflow during second sum
        let op_code = 0b1110_00_0_0101_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        // All 1 except MSB
        cpu.registers.set_register_at(0, i32::MAX as u32 - 1);
        cpu.registers.set_register_at(2, 1);

        cpu.execute(op_code);

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
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, 10);
        cpu.registers.set_register_at(2, 5);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 5);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers carry during first diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, 0);
        cpu.registers.set_register_at(2, 1);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers carry during sum
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, u32::MAX);
        cpu.registers.set_register_at(2, 0);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers carry during second diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(false);

        cpu.registers.set_register_at(0, 0);
        cpu.registers.set_register_at(2, 0);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), -1_i32 as u32);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(!cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers overflow during first diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, i32::MAX as u32);
        cpu.registers.set_register_at(2, -1_i32 as u32);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), 1 << 31);
        assert!(cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(cpu.cpsr.sign_flag());

        // Covers overflow during sum
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(true);

        cpu.registers.set_register_at(0, i32::MAX as u32);
        cpu.registers.set_register_at(2, 0);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), i32::MAX as u32);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());

        // Covers overflow during second diff
        let op_code = 0b1110_00_0_0110_1_0000_0001_0000_0_00_0_0010;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.cpsr.set_carry_flag(false);

        cpu.registers.set_register_at(0, i32::MIN as u32);
        cpu.registers.set_register_at(2, 0);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(1), i32::MAX as u32);
        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.zero_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
    }

    #[test]
    fn check_psr_transfer() {
        {
            // Covers MRS with CPSR and User mode
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_0_001111_0000_000000000000;
            let op_code = cpu.decode(op_code);

            cpu.cpsr.set_carry_flag(true);
            cpu.cpsr.set_overflow_flag(true);
            cpu.cpsr.set_zero_flag(true);
            cpu.cpsr.set_sign_flag(true);

            cpu.execute(op_code);

            assert_eq!(
                cpu.registers.register_at(0),
                0b1111_00000000000000000000_001_10000
            );
        }
        {
            // Covers MRS with SPSR_fiq
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_1_001111_0000_000000000000;
            let op_code = cpu.decode(op_code);
            cpu.cpsr.set_mode(Mode::Fiq);

            cpu.register_bank.spsr_fiq.set_state_bit(true);
            cpu.register_bank.spsr_fiq.set_mode(Mode::Fiq);
            cpu.register_bank.spsr_fiq.set_carry_flag(true);
            cpu.register_bank.spsr_fiq.set_overflow_flag(true);
            cpu.register_bank.spsr_fiq.set_zero_flag(true);
            cpu.register_bank.spsr_fiq.set_sign_flag(true);

            cpu.execute(op_code);

            assert_eq!(
                cpu.registers.register_at(0),
                0b1111_00000000000000000000_001_10001
            );
        }
        {
            // Covers MSR with CPSR and User Mode
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_0_1010011111_00000000_0000;
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 0b1111 << 28);

            cpu.execute(op_code);

            // All flags set and User mode
            assert_eq!(u32::from(cpu.cpsr), 0b1111 << 28 | (0b110000));
        }
        {
            // Covers MSR with SPSR_fiq
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00010_1_1010011111_00000000_0000;
            let op_code = cpu.decode(op_code);
            cpu.cpsr.set_mode(Mode::Fiq);

            cpu.registers.set_register_at(0, 0b1111 << 28 | (0b10001));

            cpu.execute(op_code);

            // All flags set and Fiq mode
            assert_eq!(
                u32::from(cpu.register_bank.spsr_fiq),
                0b1111 << 28 | (0b10001)
            );
        }
        {
            // Covers MSR-flags with CPSR and User mode
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00_0_10_0_1010001111_00000000_0000;
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(0, 0b1111 << 28);

            cpu.execute(op_code);

            // All flags set and User mode
            assert_eq!(u32::from(cpu.cpsr), 0b1111 << 28 | (0b110000));
        }
        {
            // Covers MSR-flags with SPSR_fiq
            let mut cpu = Arm7tdmi::default();
            let op_code = 0b1110_00_0_10_1_1010001111_00000000_0000;
            let op_code = cpu.decode(op_code);
            cpu.cpsr.set_mode(Mode::Fiq);

            // Trying to change MODE bits to a User mode
            cpu.registers.set_register_at(0, 0b1111 << 28 | (0b10000));

            cpu.execute(op_code);

            // All flags set
            assert_eq!(u32::from(cpu.register_bank.spsr_fiq), 0b1111 << 28);
        }
    }
}
