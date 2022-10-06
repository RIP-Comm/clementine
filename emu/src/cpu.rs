pub trait Cpu {
    /// Size of Opcode: it can be changed
    type OpCodeType;

    /// It generally takes the next instruction from PC
    fn fetch(&self) -> u32;

    /// It decodes the instruction to understand the
    /// OpCode and the variables
    fn decode(&self, op_code: u32) -> Self::OpCodeType;

    /// It executes the opcode and updates registers and memory
    fn execute(&mut self, op_code: Self::OpCodeType);

    /// Abstraction of what happens for every instruction in the cpu
    fn step(&mut self);

    /// Get the value of all registers
    fn registers(&self) -> Vec<u32>;
}
