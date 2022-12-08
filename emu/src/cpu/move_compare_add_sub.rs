use crate::bitwise::Bits;

use super::{
    arm7tdmi::{Arm7tdmi, SIZE_OF_THUMB_INSTRUCTION},
    opcode::ThumbModeOpcode,
};

enum Operation {
    Mov,
    Cmp,
    Add,
    Sub,
}

impl From<u16> for Operation {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Mov,
            1 => Self::Cmp,
            2 => Self::Add,
            3 => Self::Sub,
            _ => unreachable!(),
        }
    }
}

impl Arm7tdmi {
    pub fn move_compare_add_sub_imm(&mut self, op_code: ThumbModeOpcode) -> Option<u32> {
        let op: Operation = op_code.get_bits(11..=12).into();
        let rd = op_code.get_bits(8..=10);
        let offset8 = op_code.get_bits(0..=7) as u32;

        match op {
            Operation::Mov => {
                self.registers
                    .set_register_at(rd.try_into().unwrap(), offset8);

                // FIXME: Not sure if we should preserve the carry flag.
                // Documentation says that this is equal to an ARM MOVS Rd, #offset8
                // And in general MOV doesn't preserve the carry flag in ARM
                self.cpsr.set_carry_flag(false);
                self.cpsr.set_zero_flag(offset8 == 0);

                // FIXME: Since we're using an 8bits immediate it can't be negative since it's zero-extended
                // To check if it's zero-extended for real. Documentation says that this is equal to an
                // ARM MOVS Rd, #offset8 and ARM zero-extends in Mov immediate.
                self.cpsr.set_sign_flag(false);
            }
            Operation::Cmp => {
                let rd = self.registers.register_at(rd.try_into().unwrap());
                let sub_result = Self::sub_inner_op(rd, offset8);
                self.cpsr.set_flags(sub_result);
            }
            Operation::Add => {
                let rd_value = self.registers.register_at(rd.try_into().unwrap());
                let add_result = Self::add_inner_op(rd_value, offset8);
                self.registers
                    .set_register_at(rd.try_into().unwrap(), add_result.result);
                self.cpsr.set_flags(add_result);
            }
            Operation::Sub => {
                let rd_value = self.registers.register_at(rd.try_into().unwrap());
                let sub_result = Self::sub_inner_op(rd_value, offset8);
                self.registers
                    .set_register_at(rd.try_into().unwrap(), sub_result.result);
                self.cpsr.set_flags(sub_result);
            }
        };

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::{
        arm7tdmi::Arm7tdmi, instruction::ThumbModeInstruction, opcode::ThumbModeOpcode,
    };

    #[test]
    fn check_move_compare_add_sub_imm() {
        let mut cpu = Arm7tdmi::default();
        let op_code = 0b0010_0000_0000_0000_u16;
        let op_code: ThumbModeOpcode = cpu.decode(op_code);
        assert_eq!(
            op_code.instruction,
            ThumbModeInstruction::MoveCompareAddSubtractImm
        );

        cpu.execute_thumb(op_code);

        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }
}
