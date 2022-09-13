use crate::condition::Condition;

pub trait Cpu {
    /// Size of Opcode: it can be changed
    type OpCodeType;
    type InstructionType;

    /// It generally takes the next instruction from PC
    fn fetch(&self) -> Self::OpCodeType;

    /// It decodes the instruction to understand the
    /// OpCode and the variables
    fn decode(&self, op_code: Self::OpCodeType) -> (Condition, Self::InstructionType);

    /// It executes the opcode and updates registers and memory
    fn execute(&mut self, op_code: u32, instruction_type: Self::InstructionType);

    /// Abstraction of what happens for every instruction in the cpu
    fn step(&mut self);
}
