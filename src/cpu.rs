pub(crate) struct CPU {
    data: Vec<u8>,
    program_counter: usize,
}

const OPCODE_ARM_SIZE: usize = 4;

impl CPU {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            program_counter: 0,
        }
    }

    pub(crate) fn step(&mut self) {
        self.fetch();
    }

    fn fetch(&mut self) {
        let start_pc = self.program_counter;
        self.program_counter += OPCODE_ARM_SIZE;
        let op = self.data[start_pc..self.program_counter].to_vec();
        for n in &op {
            println!("fetch -> {:x}", n);
        }

        let condition = op[3] & 0x0F; // latest 4 bit (28..32)
        println!("condition -> {:b}", condition);
    }
}
