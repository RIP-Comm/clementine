use crate::{
    condition::Condition,
    cpsr::Cpsr,
    cpu::{Cpu, InstructionKind},
};

pub(crate) struct Arm7tdmi {
    data: Vec<u8>,
    program_counter: usize,

    cpsr: Cpsr,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu for Arm7tdmi {
    type OpCodeType = u32;

    fn fetch(&self) -> Self::OpCodeType {
        let end_instruction = self.program_counter + OPCODE_ARM_SIZE;
        let data_instruction: [u8; 4] = self.data[self.program_counter..end_instruction]
            .try_into()
            .unwrap();

        let op_code = u32::from_le_bytes(data_instruction);
        println!();
        println!("opcode -> {:b}", op_code);

        op_code
    }

    fn decode(&self, op_code: Self::OpCodeType) -> (Condition, InstructionKind) {
        let condition = (op_code >> 28) as u8; // bit 31..=28
        let instruction = if (op_code & 0x0A_00_00_00) != 0 {
            if (op_code & 0x01_00_00_00) != 0 {
                InstructionKind::BranchLink
            } else {
                InstructionKind::Branch
            }
        } else {
            todo!()
        };
        println!("condition -> {:x}", condition);
        println!("instruction -> {:?}", instruction);
        (condition.into(), instruction)
    }

    fn execute(&self) {}

    fn step(&self) {
        let op_code = self.fetch();

        let condition = self.decode(op_code);
        if self.cpsr.can_execute(condition.0) {
            todo!("we can execute now")
        }
    }
}

impl Arm7tdmi {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            program_counter: 0,
            cpsr: Cpsr::default(),
        }
    }
}
