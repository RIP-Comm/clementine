use std::convert::TryInto;

use crate::alu_instruction::ArmModeAluInstruction;
use crate::bitwise::Bits;
use crate::instruction::ArmModeInstruction;
use crate::internal_memory::InternalMemory;
use crate::io_device::IoDevice;
use crate::opcode::ArmModeOpcode;
use crate::{cpsr::Cpsr, cpu::Cpu};

/// Contains the 16 registers for the CPU, latest (R15) is special because
/// is the program counter.
#[derive(Default)]
struct Registers([u32; 16]);

impl Registers {
    pub fn program_counter(&self) -> usize {
        self.0[15].try_into().unwrap()
    }

    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_program_counter(&mut self, new_value: u32) {
        self.0[15] = new_value
    }

    pub fn advance_program_counter(&mut self, bytes: u32) {
        self.0[15] = self.0[15].wrapping_add(bytes);
    }

    #[allow(clippy::only_used_in_recursion)] // FIXME: Possible bug of clippy?
    pub fn set_register_at(&mut self, reg: usize, new_value: u32) {
        self.0[reg] = new_value;
    }

    pub const fn register_at(&self, reg: usize) -> u32 {
        self.0[reg]
    }

    pub fn to_vec(&self) -> Vec<u32> {
        self.0.as_slice().to_vec()
    }
}

pub struct Arm7tdmi {
    rom: Vec<u8>,

    registers: Registers,
    cpsr: Cpsr,

    memory: InternalMemory,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu for Arm7tdmi {
    type OpCodeType = ArmModeOpcode;

    fn fetch(&self) -> u32 {
        let instruction_index = self.registers.program_counter();
        let end_instruction = instruction_index + OPCODE_ARM_SIZE;
        let data_instruction: [u8; 4] = self.rom[instruction_index..end_instruction]
            .try_into()
            .expect("`istruction` conversion into [u8; 4]");

        u32::from_le_bytes(data_instruction)
    }

    fn decode(&self, op_code: u32) -> Self::OpCodeType {
        let op_code = ArmModeOpcode::try_from(op_code).unwrap();
        println!("{}", op_code);
        if op_code.instruction == ArmModeInstruction::Unknown {
            todo!("implement this instruction")
        }

        op_code
    }

    fn execute(&mut self, op_code: Self::OpCodeType) {
        use ArmModeInstruction::*;
        match op_code.instruction {
            Branch => {
                self.branch(op_code);
            }
            BranchLink => {
                self.branch_link(op_code);
            }
            DataProcessing1 | DataProcessing2 | DataProcessing3 => {
                self.data_processing(op_code);
            }
            TransImm9 => {
                self.single_data_transfer(op_code);
            }
            Unknown => {
                todo!("implement this instruction")
            }
        }

        self.registers.advance_program_counter(4);
    }

    fn step(&mut self) {
        let op_code = self.fetch();

        let op_code = self.decode(op_code);
        if self.cpsr.can_execute(op_code.condition) {
            self.execute(op_code)
        }
    }

    fn registers(&self) -> Vec<u32> {
        self.registers.to_vec()
    }
}

impl Arm7tdmi {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            registers: Registers::default(),
            cpsr: Cpsr::default(),
            memory: InternalMemory::new(),
        }
    }

    fn branch(&mut self, op_code: ArmModeOpcode) {
        let offset = op_code.get_bits(0..=23);

        self.registers.advance_program_counter(8 + offset * 4);
    }

    fn branch_link(&mut self, op_code: ArmModeOpcode) {
        let pc: u32 = self.registers.program_counter().try_into().unwrap();
        self.registers.set_register_at(14, pc.wrapping_add(4)); // R14 = LR

        let offset = op_code.get_bits(0..=23);

        self.registers.advance_program_counter(8 + offset * 4);
    }

    fn data_processing(&mut self, op_code: ArmModeOpcode) {
        // bit [25] is I = Immediate Flag
        let i: bool = op_code.get_bit(25);
        // bits [24-21]
        let alu_op_code = op_code.get_bits(21..=24);
        // bit [20] is sets condition codes
        let s = op_code.get_bit(20);
        // bits [15-12] are the Rd
        let rd = op_code.get_bits(12..=15);
        // bits [19-16] are the Rn
        let rn = op_code.get_bits(16..=19);

        let op2 = match i {
            // Register as 2nd Operand
            false => {
                // bits [6-5] - Shift Type (0=LSL, 1=LSR, 2=ASR, 3=ROR)
                let shift_type = op_code.get_bits(5..=6);
                // bit [4] - is Shift by Register Flag (0=Immediate, 1=Register)
                let r = op_code.get_bit(4);
                // bits [0-3] 2nd Operand Register (R0..R15) (including PC=R15)
                let mut op2 = op_code.get_bits(0..=3);

                match r {
                    // 0=Immediate, 1=Register
                    // Shift by amount
                    false => {
                        // bits [7-11] - Shift amount
                        let shift_amount = op_code.get_bits(7..=11);
                        op2 = self.shift(shift_type, shift_amount, op2);
                    }
                    // Shift by register
                    true => {
                        // bits [11-8] - Shift register (R0-R14) - only lower 8bit 0-255 used
                        let rs = op_code.get_bits(8..=11);
                        let shift_amount = self
                            .registers
                            .register_at(rs.try_into().unwrap())
                            .get_bits(0..=7);
                        op2 = self.shift_immediate(shift_amount, shift_type, op2);
                    }
                };

                op2
            }
            // Immediate as 2nd Operand
            true => {
                // bits [11-8] are ROR-Shift applied to nn
                let is = op_code.get_bits(8..=11);
                // bits [7-0] are the immediate value
                let nn = op_code.get_bits(0..=7);

                // I'm not sure about `* 2`
                nn.rotate_right(is * 2) // TODO: review "ROR-Shift applied to nn (0-30, in steps of 2)"
            }
        };

        match ArmModeAluInstruction::from(alu_op_code) {
            ArmModeAluInstruction::Mov => self.mov(rd.try_into().unwrap(), op2),
            ArmModeAluInstruction::Teq => {
                if s {
                    self.teq(rn, op2)
                }
            }
            ArmModeAluInstruction::Cmp => {
                if s {
                    self.cmp(rn, op2)
                }
            }
            ArmModeAluInstruction::Add => self.add(
                rd.try_into().unwrap(),
                self.registers.register_at(rn.try_into().unwrap()),
                op2,
                s,
            ),
            ArmModeAluInstruction::Orr => self.orr(
                rd.try_into().unwrap(),
                self.registers.register_at(rn.try_into().unwrap()),
                op2,
                s,
            ),
            _ => todo!("implement alu operation: {}", alu_op_code),
        }
    }

    fn single_data_transfer(&mut self, op_code: ArmModeOpcode) {
        let immediate = op_code.get_bit(25);
        let up_down = op_code.get_bit(23);

        // bits [19-16] - Base register
        let rn = op_code.get_bits(16..=19);

        // 0xF is register of PC
        let address = if rn == 0xF {
            let pc: u32 = self.registers.program_counter().try_into().unwrap();
            pc + 8_u32
        } else {
            self.registers.register_at(rn.try_into().unwrap())
        };

        // bits [15-12] - Source/Destination Register
        let rd = op_code.get_bits(12..=15);

        let offset: u32 = if immediate {
            todo!()
        } else {
            op_code.get_bits(0..=11)
        };

        let load_store: SingleDataTransfer = op_code
            .raw
            .try_into()
            .expect("convert to Single Data Transfer");

        let value: u32 = self
            .memory
            .read_at(if up_down {
                address.wrapping_sub(offset)
            } else {
                address.wrapping_add(offset)
            })
            .try_into()
            .unwrap(); // FIXME: is this right? Or we should read a WORD (u32)

        match load_store {
            SingleDataTransfer::Ldr => self
                .registers
                .set_register_at(rd.try_into().unwrap(), value),
            _ => todo!("implement single data transfer operation"),
        }
    }

    fn add(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        // we do the sum in 64bits so that the 32nd bit is the carry
        let result_and_carry: u64 = (rn as u64).wrapping_add(op2 as u64);
        let result: u32 = result_and_carry as u32;

        self.registers.set_register_at(rd, result);

        if s {
            let sign_op1: bool = rn.get_bit(31);
            let sign_op2: bool = op2.get_bit(31);
            let sign_r: bool = result.get_bit(31);

            let carry: bool = (result_and_carry & 0x100000000) >> 32 == 1;

            // overflow only occurs when operands have the same sign and result has the opposite one
            let same_sign: bool = sign_op1 == sign_op2;
            self.cpsr
                .set_overflow_flag(same_sign && (sign_op1 != sign_r));
            self.cpsr.set_carry_flag(carry);
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.is_bit_on(31));
        }
    }

    fn orr(&mut self, rd: usize, rn: u32, op2: u32, s: bool) {
        let result: u32 = rn | op2;

        self.registers.set_register_at(rd, result);

        if s {
            self.cpsr.set_zero_flag(result == 0);
            self.cpsr.set_sign_flag(result.is_bit_on(31));
        }
    }

    fn mov(&mut self, rd: usize, op2: u32) {
        self.registers.set_register_at(rd, op2);
    }

    fn teq(&mut self, rn: u32, op2: u32) {
        let value = self.registers.register_at(rn.try_into().unwrap()) ^ op2;
        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    fn cmp(&mut self, rn: u32, op2: u32) {
        let value = self.registers.register_at(rn.try_into().unwrap()) - op2;
        self.cpsr.set_sign_flag(value.is_bit_on(31));
        self.cpsr.set_zero_flag(value == 0);
    }

    fn shift(&mut self, shift_type: u32, shift_amount: u32, mut value: u32) -> u32 {
        match shift_amount {
            0 => match shift_type {
                // LSL#0: No shift performed, ie. directly value=Rm, the C flag is NOT affected.
                0 => (), // TODO: It's better to implement the logical instruction in order to execute directly LSL#0?
                // LSR#0: Interpreted as LSR#32, ie. value becomes zero, C becomes Bit 31 of Rm.
                1 => {
                    // TODO: It's better to implement the logical instruction in order to execute directly LSR#0?
                    let rm = self.registers.register_at(value.try_into().unwrap());
                    self.cpsr.set_sign_flag(rm.get_bit(31));
                    value = 0;
                }
                // ASR#0: Interpreted as ASR#32, ie. value and C are filled by Bit 31 of Rm.
                2 => {
                    // TODO: It's better to implement the logical instruction in order to execute directly ASR#0?
                    let rm = self.registers.register_at(value.try_into().unwrap());
                    match rm.get_bit(31) {
                        true => {
                            value = 1;
                            self.cpsr.set_sign_flag(true)
                        }
                        false => {
                            value = 0;
                            self.cpsr.set_sign_flag(true)
                        }
                    }
                }
                // ROR#0: Interpreted as RRX#1 (RCR), like ROR#1, but value Bit 31 set to old C.
                3 => {
                    // TODO: It's better to implement the logical instruction in order to execute directly RRX#0?
                    todo!("value Bit 31 set to old C"); // I'm not sure what "old C" means
                }
                _ => unreachable!(),
            },
            shift_amount => value = self.shift_immediate(shift_type, shift_amount, value),
        };

        value
    }

    fn shift_immediate(&self, shift_type: u32, shift_amount: u32, mut value: u32) -> u32 {
        match shift_type {
            // Logical Shift Left
            0 => value <<= shift_amount,
            // Logical Shift Right
            1 => value >>= shift_amount,
            // Arithmetic Shift Right
            2 => value = ((value as i32) >> shift_amount) as u32, // TODO: Review rust arithmetic shift right
            // Rotate Right
            3 => value = value.rotate_right(shift_amount as u32),
            _ => unreachable!(),
        }

        value
    }
}

enum SingleDataTransfer {
    Ldr,
    Str,
    Pld,
}

impl From<u32> for SingleDataTransfer {
    fn from(op_code: u32) -> Self {
        let must_for_pld = op_code.are_bits_on(28..=31);
        if op_code.get_bit(20) {
            if must_for_pld {
                Self::Pld
            } else {
                Self::Ldr
            }
        } else {
            Self::Str
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::condition::Condition;
    use crate::instruction::ArmModeInstruction;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn decode_branch() {
        let output: ArmModeOpcode = 0b1110_1010_0000_0000_0000_0000_0111_1111
            .try_into()
            .unwrap();
        assert_eq!(output.instruction, ArmModeInstruction::Branch);
    }

    #[test]
    fn decode_branch_link() {
        let output: ArmModeOpcode = 0b1110_1011_0000_0000_0000_0000_0111_1111
            .try_into()
            .unwrap();
        assert_eq!(output.instruction, ArmModeInstruction::BranchLink);
    }

    #[test]
    fn test_registers_14_after_branch_link() {
        let mut cpu: Arm7tdmi = Arm7tdmi::new(vec![]);
        cpu.registers = Registers([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let pc: u32 = cpu.registers.program_counter().try_into().unwrap();
        cpu.branch_link(0b0_u32.try_into().unwrap());
        assert_eq!(cpu.registers.register_at(14), pc.wrapping_add(4));
    }

    #[test]
    fn check_mov_rx_immediate() {
        // MOV R0, 0
        let mut op_code: u32 = 0b1110_0011_1010_0000_0000_0000_0000_0000;

        // bits [11-8] are ROR-Shift applied to nn
        let is = op_code & 0b0000_0000_0000_0000_0000_1111_0000_0000;

        // MOV Rx,x
        let mut cpu = Arm7tdmi::new(vec![]);
        for rx in 0..=0xF {
            let register_for_op = rx << 12;
            let immediate_value = rx;

            // Rd parameter
            op_code = (op_code & 0b1111_1111_1111_1111_0000_1111_1111_1111) + register_for_op;
            // Immediate parameter
            op_code = (op_code & 0b1111_1111_1111_1111_1111_1111_0000_0000) + immediate_value;

            let op_code = cpu.decode(op_code);
            assert_eq!(op_code.condition, Condition::AL);
            assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing3);

            cpu.execute(op_code);
            let rotated = rx.rotate_right(is * 2);
            if rotated == 15 {
                // NOTE: since is R15 you should also consider the advance of 4 bytes after execution.
                assert_eq!(
                    cpu.registers.register_at(rx.try_into().unwrap()),
                    rotated + 4
                );
            } else {
                assert_eq!(cpu.registers.register_at(rx.try_into().unwrap()), rotated);
            }
        }
    }

    #[test]
    fn check_teq() {
        // This case cover S=0 then it will skip the execution of TEQ.
        {
            let op_code = 0b1110_0001_0010_1001_0011_0000_0000_0000;
            let mut cpu = Arm7tdmi::new(vec![]);
            let op_code = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);
            let rn = 9_usize;
            cpu.registers.set_register_at(rn, 100);
            cpu.cpsr.set_sign_flag(true); // set for later verify.
            cpu.execute(op_code);
            assert!(cpu.cpsr.sign_flag());
            assert!(!cpu.cpsr.zero_flag());
        }
    }

    #[test]
    fn check_cmp_s1() {
        let op_code: u32 = 0b1110_0001_0011_1010_0011_0000_0000_0000;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);
        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 1);
        cpu.execute(op_code);
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_cmp_s0() {
        let op_code: u32 = 0b1110_0001_0010_1010_0011_0000_0000_0000;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);
        let rn = 9_usize;
        cpu.registers.set_register_at(rn, 1);
        cpu.cpsr.set_sign_flag(true); // set for later verify.
        cpu.execute(op_code);
        assert!(cpu.cpsr.sign_flag());
        assert!(!cpu.cpsr.zero_flag());
    }

    #[test]
    fn check_add() {
        let op_code = 0b1110_0010_1000_1111_0000_0000_0010_0000;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing3);
        cpu.registers.set_register_at(15, 15);
        cpu.execute(op_code);
        assert_eq!(cpu.registers.register_at(0), 15 + 32);
    }

    #[test]
    fn check_add_carry_bit() {
        let op_code: u32 = 0b1110_0000_1001_1111_0000_0000_0000_1110;
        let mut cpu = Arm7tdmi::new(vec![]);
        let op_code = cpu.decode(op_code);

        assert_eq!(op_code.instruction, ArmModeInstruction::DataProcessing1);

        cpu.registers.set_register_at(15, 1 << 31);
        cpu.registers.set_register_at(14, 1 << 31);
        cpu.execute(op_code);
        assert_eq!(cpu.registers.register_at(0), 0);
        assert!(cpu.cpsr.carry_flag());
        assert!(cpu.cpsr.overflow_flag());
        assert!(!cpu.cpsr.sign_flag());
        assert!(cpu.cpsr.zero_flag());
    }

    // TODO: this is only one case of these kind of instruction.
    // create other cases or other tests :).
    #[test]
    fn check_single_data_transfer() {
        let op_code = 0b1110_0101_1001_1111_1101_0000_0001_1000;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code_type = cpu.decode(op_code);
        assert_eq!(op_code_type.instruction, ArmModeInstruction::TransImm9);

        let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
            .try_into()
            .expect("conversion `rd` to u8");

        assert_eq!(rd, 13);

        // because in this specific case address will be
        // then will be 92 + 8 (.wrapping_sub(offset))
        cpu.registers.set_program_counter(92);

        // simulate mem already contains something.
        cpu.memory.write_at(76, 99);

        cpu.execute(op_code_type);
        assert_eq!(cpu.registers.register_at(13), 99);
        assert_eq!(cpu.registers.program_counter(), 96);
    }

    #[test]
    #[should_panic]
    fn check_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::Unknown);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute(op_code);
    }
}
