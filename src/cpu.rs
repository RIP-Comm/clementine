use crate::condition::Condition;

pub(crate) trait Cpu {
    
    // Size of Opcode: it can to change.
    type OpCodeType;

    // It generally takes the next instruction from PC
    fn fetch(&self) -> Self::OpCodeType;

    // It decodes the instruction to understand the 
    // OpCode and the variables
    fn decode(&self, op_code: Self::OpCodeType) -> Condition;

    // It executes the opcode and updates registers and memory
    fn execute(&self);

    // Abstraction of what happens for every instruction in the cpu
    fn step(&self);
}