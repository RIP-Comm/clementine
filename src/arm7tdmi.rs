use std::{fmt::Error, rc::Rc};

use crate::{condition::Condition, cpsr::Cpsr, cpu::Cpu};

pub(crate) struct Arm7tdmi {
    rom: Rc<Vec<u8>>,

    program_counter: u32,
    cpsr: Cpsr,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu for Arm7tdmi {
    type OpCodeType = u32;
    type InstructionType = ArmModeInstruction;

    fn fetch(&self) -> Self::OpCodeType {
        let instruction_index = self.program_counter as usize;
        let end_instruction = instruction_index + OPCODE_ARM_SIZE;
        let data_instruction: [u8; 4] = self.rom[instruction_index..end_instruction]
            .try_into()
            .unwrap();

        let op_code = u32::from_le_bytes(data_instruction);
        println!();
        println!("opcode -> {:b}", op_code);

        op_code
    }

    fn decode(&self, op_code: Self::OpCodeType) -> (Condition, Self::InstructionType) {
        let condition = (op_code >> 28) as u8; // bit 31..=28
        println!("condition -> {:x}", condition);

        let res_decode = ArmModeInstruction::get_instruction(op_code);
        if res_decode.is_err() {
            todo!("ArmModeInstruction")
        }
        let instruction = res_decode.expect("ArmMode");
        println!("instruction -> {:?}", instruction);

        (condition.into(), instruction)
    }

    fn execute(&mut self, op_code: u32, instruction_type: ArmModeInstruction) {
        use ArmModeInstruction::*;
        match instruction_type {
            Branch => {
                self.branch(op_code);
            }
            BranchLink => {
                self.branch_link(op_code);
            }
            _ => todo!("Instruction not implemented yet."),
        }
    }

    fn step(&mut self) {
        let op_code = self.fetch();

        let (condition, instruction) = self.decode(op_code);
        if self.cpsr.can_execute(condition) {
            self.execute(op_code, instruction)
        }
    }
}

impl Arm7tdmi {
    pub(crate) fn new(rom: Rc<Vec<u8>>) -> Self {
        Self {
            rom,
            program_counter: 0,
            cpsr: Cpsr::default(),
        }
    }

    fn branch(&mut self, op_code: u32) {
        let offset = op_code & 0x00_FF_FF_FF;
        println!("offset: {:?}", offset);

        self.program_counter += 8 + offset * 4;
        println!("PC: {:?}", self.program_counter);
    }

    fn branch_link(&mut self, op_code: u32) {
        todo!("Branch Link")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArmModeInstruction {
    Branch = 0x0A_00_00_00,
    BranchLink = 0x0B_00_00_00,
}

impl ArmModeInstruction {
    fn get_instruction(op_code: u32) -> Result<ArmModeInstruction, Error> {
        use ArmModeInstruction::*;

        if Self::check(Branch, op_code) {
            return Ok(Branch);
        }
        if Self::check(BranchLink, op_code) {
            return Ok(BranchLink);
        } else {
            Err(Error)
        }
    }

    fn check(instruction_type: ArmModeInstruction, op_code: u32) -> bool {
        (Self::get_mask(&instruction_type) & op_code) == instruction_type as u32
    }

    fn get_mask(instruction_type: &ArmModeInstruction) -> u32 {
        use ArmModeInstruction::*;

        match instruction_type {
            Branch | BranchLink => 0x0F_00_00_00,
            _ => todo!(),
        }
    }
}
#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn decode_branch() {
        let output = ArmModeInstruction::get_instruction(0b1110_1010_0000_0000_0000_0000_0111_1111);
        assert_eq!(output, Ok(ArmModeInstruction::Branch));
    }
}
