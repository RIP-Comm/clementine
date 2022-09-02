use crate::{condition::Condition, cpsr::Cpsr};

pub(crate) struct Cpu {
    data: Vec<u8>,
    program_counter: usize,

    cpsr: Cpsr,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            program_counter: 0,
            cpsr: Cpsr::default(),
        }
    }

    pub(crate) fn step(&mut self) {
        let opcode = self.fetch();
        let condition = self.decode(opcode);
        if self.cpsr.can_execute(condition) {
            todo!("we can execute now")
        }
    }

    fn fetch(&mut self) -> Vec<u8> {
        let start_pc = self.program_counter;
        self.program_counter += OPCODE_ARM_SIZE;
        let op = self.data[start_pc..self.program_counter].to_vec();
        for n in &op {
            println!("fetch -> {:x}", n);
        }

        op
    }

    fn decode(&mut self, op: Vec<u8>) -> Condition {
        let condition = op[3] & 0x0F; // latest 4 bit (28..32)
        println!("condition -> {:b}", condition);
        condition.into()
    }
}
