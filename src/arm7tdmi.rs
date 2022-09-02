use crate::{condition::Condition, cpsr::Cpsr, cpu::Cpu};

pub(crate) struct Arm7tdmi {
    data: Vec<u8>,
    program_counter: usize,

    cpsr: Cpsr,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu for Arm7tdmi {
    type OpCodeType = u32;

    fn fetch(&self) -> Self::OpCodeType{
        let end_instruction = self.program_counter + OPCODE_ARM_SIZE;
        let data_instruction = self.data[self.program_counter..end_instruction].to_vec();
        
        let mut op_code: Self::OpCodeType = 0;
        op_code += (data_instruction[0] as u32) << 24;
        op_code += (data_instruction[1] as u32) << 16;
        op_code += (data_instruction[2] as u32) << 8;
        op_code += data_instruction[3] as u32;

        println!("{:b}", op_code);

        op_code
    }

    fn decode(&self, op_code: Self::OpCodeType) -> Condition{
        let condition: u8 = (op_code & 0x00_00_00_0F) as u8; // latest 4 bit (28..32)
        println!("condition -> {:x}", condition);

        condition.into()
    }

    fn execute(&self) {
        
    }

    fn step(&self) {
        let op_code = self.fetch();

        let condition = self.decode(op_code);
        if self.cpsr.can_execute(condition) {
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
