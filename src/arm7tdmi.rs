use std::{convert::TryInto, fmt::Debug};

use crate::{condition::Condition, cpsr::Cpsr, cpu::Cpu};

pub(crate) struct Arm7tdmi {
    rom: Vec<u8>,

    registers: [u32; 16],

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

        let instruction: ArmModeInstruction = match op_code.try_into() {
            Ok(instruction) => instruction,
            Err(e) => todo!("{}", e),
        };

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
            Mov => {
                self.mov(op_code);
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
    pub(crate) fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            program_counter: 0,
            registers: [0; 16],
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

    fn mov(&mut self, op_code: u32) {
        // bits [24-21] are the RD
        let rd = (op_code & 0x00_00_F0_00) >> 12;
        println!("RD: {:?}", rd);

        // 25th bit is I = Immediate Flag
        let immediate: bool = (op_code & 0x02_00_00_00) >> 25 == 1;
        println!("Immediate: {:?}", immediate);

        // 20th bit is S = Condition Set
        if op_code & 0x00_08_00_00 > 0 {
            todo!("Condition set")
        }

        if immediate {
            // bits [7-0] are the immediate value
            let immediate_value = op_code & 0x00_00_00_FF;
            println!("value: {:?}", immediate_value);

            // the instruction is MOV RD, immediate_value
            self.registers[rd as usize] = immediate_value;
        } else {
            todo!("Not implemented yet.");
        }

        // N.B: I'm not sure where this has to be executed
        self.program_counter += OPCODE_ARM_SIZE as u32;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArmModeInstruction {
    Branch = 0x0A_00_00_00,
    BranchLink = 0x0B_00_00_00,

    /// 27-26 must be 0b00
    /// 24-21 must be 0b1101
    /// 19-16 must be 0b0000
    Mov = 0x01_A0_00_00,
}

impl TryFrom<u32> for ArmModeInstruction {
    type Error = String;

    fn try_from(op_code: u32) -> Result<Self, Self::Error> {
        use ArmModeInstruction::*;

        if Self::check(Branch, op_code) {
            Ok(Branch)
        } else if Self::check(BranchLink, op_code) {
            Ok(BranchLink)
        } else if Self::check(Mov, op_code) {
            Ok(Mov)
        } else {
            Err("instruction not implemented :(.".to_owned())
        }
    }
}

impl ArmModeInstruction {
    fn check(instruction_type: ArmModeInstruction, op_code: u32) -> bool {
        (Self::get_mask(&instruction_type) & op_code) == instruction_type as u32
    }

    fn get_mask(instruction_type: &ArmModeInstruction) -> u32 {
        use ArmModeInstruction::*;

        match instruction_type {
            Branch | BranchLink => 0x0F_00_00_00,
            Mov => 0x0D_EF_00_00,
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
        let output: Result<ArmModeInstruction, String> =
            0b1110_1010_0000_0000_0000_0000_0111_1111.try_into();
        assert_eq!(output, Ok(ArmModeInstruction::Branch));
    }

    #[test]
    fn check_mov_rx_immediate() {
        // MOV R0, 0
        let mut opcode: u32 = 0b11100011101000000000000000000000;

        // MOV Rx,x
        let mut cpu = Arm7tdmi::new(vec![]);
        for rx in 0..16u32 {
            let register_for_op = rx << 12;
            let immediate_value = rx;

            //Rd parameter
            opcode = (opcode & 0xFF_FF_0F_FF) + register_for_op;
            //Immediate parameter
            opcode = (opcode & 0xFF_FF_FF_00) + immediate_value;

            let (condition, instruction_type) = cpu.decode(opcode);
            assert_eq!(condition as u32, Condition::AL as u32);
            assert_eq!(instruction_type, ArmModeInstruction::Mov);

            cpu.execute(opcode, instruction_type);
            assert_eq!(cpu.registers[rx as usize], rx);
        }
    }
}
