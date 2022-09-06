use std::{convert::TryInto, fmt::Debug};

use crate::alu_instruction::ArmModeAluInstruction;
use crate::instruction::ArmModeInstruction;
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
            DataProcessing1 | DataProcessing2 | DataProcessing3 => {
                self.data_processing(op_code);
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

    fn data_processing(&mut self, op_code: u32) {
        /// bit [25] is I = Immediate Flag
        let i = ((op_code & 0x02_00_00_00) >> 25) as u8;
        /// bit [24-21]
        let alu_opcode = ((op_code & 0x01_E0_00_00) >> 25) as u8;
        /// bit [20] is sets condition codes
        let s = ((op_code & 0x00_10_00_00) >> 20) as u8;
        /// bits [15-12] are the Rd
        let rd = ((op_code & 0x00_00_F0_00) >> 12) as u8;

        let op2 = match i {
            /// Register as 2nd Operand
            0 => {
                // Shift Type (0=LSL, 1=LSR, 2=ASR, 3=ROR)
                let shift_type = ((op_code & 0x00_00_00_60) >> 5) as u8;
                // bit [4] is Shift by Register Flag (0=Immediate, 1=Register)
                let r = (op_code & 0x00_00_00_10) >> 4;
                // 2nd Operand Register (R0..R15) (including PC=R15)
                let mut op2 = ((op_code & 0x00_00_00_0F) >> 8);

                match r {
                    /// Shift by amount
                    0 => {
                        /// Shift amount
                        let is = ((op_code & 0x00_00_07_80) >> 7) as u8;
                        match is {
                            0 => match shift_type {
                                /// LSL#0: No shift performed, ie. directly Op2=Rm, the C flag is NOT affected.
                                0 => (), // TODO: It's better to implement the logical instruction in order to execute directly LSL#0?
                                /// LSR#0: Interpreted as LSR#32, ie. Op2 becomes zero, C becomes Bit 31 of Rm.
                                1 => {
                                    // TODO: It's better to implement the logical instruction in order to execute directly LSR#0?
                                    let rm = self.registers[op2 as usize];
                                    match (rm & 0b1000_0000_0000_0000_0000_0000_0000_0000) >> 31 {
                                        1 => self.cpsr.set_signed(),
                                        0 => self.cpsr.set_not_signed(),
                                        _ => unreachable!(),
                                    }

                                    op2 = 0;
                                }
                                /// ASR#0: Interpreted as ASR#32, ie. Op2 and C are filled by Bit 31 of Rm.
                                2 => {
                                    // TODO: It's better to implement the logical instruction in order to execute directly ASR#0?
                                    let rm = self.registers[op2 as usize];
                                    match (rm & 0b1000_0000_0000_0000_0000_0000_0000_0000) >> 31 {
                                        1 => {
                                            op2 = 1;
                                            self.cpsr.set_signed()
                                        }
                                        0 => {
                                            op2 = 0;
                                            self.cpsr.set_not_signed()
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                                /// ROR#0: Interpreted as RRX#1 (RCR), like ROR#1, but Op2 Bit 31 set to old C.
                                3 => {
                                    // TODO: It's better to implement the logical instruction in order to execute directly RRX#0?
                                    todo!("Op2 Bit 31 set to old C"); // I'm not sure what "old C" means
                                }
                                _ => unreachable!(),
                            },
                            is => op2 <<= is,
                        };
                    }
                    /// Shift by register
                    1 => {
                        let rs = ((op_code & 0x00_00_0F_00) >> 8) as u8;
                        op2 <<= self.registers[rs as usize] & 0x00_00_00_FF;
                    }
                    _ => unreachable!(),
                };

                op2
            }
            /// Immediate as 2nd Operand
            1 => {
                /// bits [11-8] are ROR-Shift applied to nn
                let is = op_code & 0x00_00_0F_00;
                /// bits [7-0] are the immediate value
                let nn = op_code & 0x00_00_00_FF;

                // I'm not sure about `* 2`
                nn.rotate_right(is * 2) // TODO: review "ROR-Shift applied to nn (0-30, in steps of 2)"
            }
            _ => unreachable!(),
        };

        use ArmModeAluInstruction::*;
        match ArmModeAluInstruction::from(alu_opcode) {
            Mov => self.mov(rd as usize, op2),
            _ => todo!(),
        }

        // TODO: Returned CPSR flags
    }

    fn mov(&mut self, rd: usize, op2: u32) {
        self.registers[rd] = op2;
    }
}

#[cfg(test)]
mod tests {
    use crate::instruction::ArmModeInstruction;
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
        let mut opcode: u32 = 0b1110_0011_1010_0000_0000_0000_0000_0000;

        // bits [11-8] are ROR-Shift applied to nn
        let is = opcode & 0x00_00_0F_00;

        // MOV Rx,x
        let mut cpu = Arm7tdmi::new(vec![]);
        for rx in 0..=0xF {
            let register_for_op = rx << 12;
            let immediate_value = rx;

            // Rd parameter
            opcode = (opcode & 0xFF_FF_0F_FF) + register_for_op;
            // Immediate parameter
            opcode = (opcode & 0xFF_FF_FF_00) + immediate_value;

            let (condition, instruction_type) = cpu.decode(opcode);
            assert_eq!(condition as u32, Condition::AL as u32);
            assert_eq!(instruction_type, ArmModeInstruction::DataProcessing3);

            cpu.execute(opcode, instruction_type);
            assert_eq!(cpu.registers[rx as usize], rx.rotate_right(is * 2));
        }
    }
}
