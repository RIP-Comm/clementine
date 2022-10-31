use crate::{
    alu_instruction::{AluInstructionKind, ArmModeAluInstruction, Kind},
    arm7tdmi::Arm7tdmi,
    bitwise::Bits,
    opcode::ArmModeOpcode,
};

pub struct ArithmeticOpResult {
    result: u32,
    pub carry: bool,
    pub overflow: bool,
    pub sign: bool,
    pub zero: bool,
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
                    0xF => self.get_pc_offset_alu(i, r),
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
            0xF => self.get_pc_offset_alu(i, op_code.get_bit(4)),
            _ => 0,
        };

        let rn = self.registers.register_at(rn.try_into().unwrap()) + offset;

        let op2 = self.get_operand(alu_op_code, s, i, op_code.get_bits(0..=11));

        // S = 1 and Rd = 0xF should not be allowed in User Mode.
        // TODO: When in other modes it should load SPSR_<current_mode> into CPSR
        if s && rd == 0xF {
            unimplemented!("Implement cases when S=1 and Rd=0xF");
        }

        match ArmModeAluInstruction::from(alu_op_code) {
            ArmModeAluInstruction::And => self.and(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Eor => self.eor(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Bic => self.bic(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Mvn => self.mvn(rd.try_into().unwrap(), op2, s),
            ArmModeAluInstruction::Tst => {
                if s {
                    self.tst(rn, op2)
                }
            }
            ArmModeAluInstruction::Mov => self.mov(rd.try_into().unwrap(), op2, s),
            ArmModeAluInstruction::Teq => {
                if s {
                    self.teq(rn, op2)
                }
            }
            ArmModeAluInstruction::Cmp => {
                if s {
                    self.cmp(rn, op2)
                }
            }
            ArmModeAluInstruction::Cmn => {
                if s {
                    self.cmn(rn, op2)
                }
            }
            ArmModeAluInstruction::Add => self.add(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Orr => self.orr(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Sub => self.sub(rd.try_into().unwrap(), rn, op2, s),
            ArmModeAluInstruction::Rsb => self.rsb(rd.try_into().unwrap(), rn, op2, s),
            _ => todo!("implement alu operation: {}", alu_op_code),
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
