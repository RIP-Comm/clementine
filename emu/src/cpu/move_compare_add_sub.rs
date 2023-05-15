use std::fmt;

use super::arm7tdmi::{Arm7tdmi, SIZE_OF_THUMB_INSTRUCTION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbHighRegisterOperation {
    Add,
    Cmp,
    Mov,
    BxOrBlx,
}

impl fmt::Display for ThumbHighRegisterOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mov => f.write_str("MOV"),
            Self::Cmp => f.write_str("CMP"),
            Self::Add => f.write_str("ADD"),
            Self::BxOrBlx => f.write_str("BX/BLX"),
        }
    }
}

impl From<u16> for ThumbHighRegisterOperation {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Add,
            1 => Self::Cmp,
            2 => Self::Mov,
            3 => Self::BxOrBlx,
            _ => unreachable!(),
        }
    }
}

/// Operation to perform in the Move Compare Add Subtract Immediate instruction.
#[derive(Debug, PartialEq, Eq)]
pub enum Operation {
    Mov,
    Cmp,
    Add,
    Sub,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mov => f.write_str("MOV"),
            Self::Cmp => f.write_str("CMP"),
            Self::Add => f.write_str("ADD"),
            Self::Sub => f.write_str("SUB"),
        }
    }
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
    pub fn move_compare_add_sub_imm(
        &mut self,
        op: Operation,
        r_destination: u16,
        offset: u32,
    ) -> Option<u32> {
        let dest = r_destination.try_into().unwrap();
        match op {
            Operation::Mov => {
                self.registers.set_register_at(dest, offset);

                // FIXME: Not sure if we should preserve the carry flag.
                // Documentation says that this is equal to an ARM MOVS Rd, #offset8
                // And in general MOV doesn't preserve the carry flag in ARM
                self.cpsr.set_carry_flag(false);
                self.cpsr.set_zero_flag(offset == 0);

                // FIXME: Since we're using an 8bits immediate it can't be negative since it's zero-extended
                // To check if it's zero-extended for real. Documentation says that this is equal to an
                // ARM MOVS Rd, #offset8 and ARM zero-extends in Mov immediate.
                self.cpsr.set_sign_flag(false);
            }
            Operation::Cmp => {
                let rd = self.registers.register_at(dest);
                let sub_result = Self::sub_inner_op(rd, offset);
                self.cpsr.set_flags(sub_result);
            }
            Operation::Add => {
                let rd_value = self.registers.register_at(dest);
                let add_result = Self::add_inner_op(rd_value, offset);
                self.registers.set_register_at(dest, add_result.result);
                self.cpsr.set_flags(add_result);
            }
            Operation::Sub => {
                let rd_value = self.registers.register_at(dest);
                let sub_result = Self::sub_inner_op(rd_value, offset);
                self.registers.set_register_at(dest, sub_result.result);
                self.cpsr.set_flags(sub_result);
            }
        };

        Some(SIZE_OF_THUMB_INSTRUCTION)
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::move_compare_add_sub::Operation;
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
            ThumbModeInstruction::MoveCompareAddSubtractImm {
                op: Operation::Mov,
                r_destination: 0,
                offset: 0,
            }
        );

        cpu.execute_thumb(op_code);

        assert!(!cpu.cpsr.carry_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }
}
