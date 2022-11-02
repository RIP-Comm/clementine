use std::cell::RefCell;
use std::convert::TryInto;
use std::rc::Rc;

use crate::bitwise::Bits;
use crate::instruction::ArmModeInstruction;
use crate::memory::internal_memory::InternalMemory;
use crate::memory::io_device::IoDevice;
use crate::opcode::ArmModeOpcode;
use crate::ppu::PixelProcessUnit;
use crate::{cpsr::Cpsr, cpu::Cpu};

/// Contains the 16 registers for the CPU, latest (R15) is special because
/// is the program counter.
#[derive(Default)]
pub struct Registers([u32; 16]);

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

    pub(crate) registers: Registers,
    pub(crate) cpsr: Cpsr,

    pub memory: Rc<RefCell<InternalMemory>>,

    pub ppu: PixelProcessUnit,
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
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        let should_advance_pc = match op_code.instruction {
            Branch => self.branch(op_code),
            BranchLink => self.branch_link(op_code),
            DataProcessing1 | DataProcessing2 | DataProcessing3 => self.data_processing(op_code),
            TransImm9 => self.single_data_transfer(op_code),
            BlockDataTransfer => self.block_data_transfer(op_code),
            Unknown => {
                todo!("implement this instruction")
            }
        };

        if should_advance_pc {
            self.registers.advance_program_counter(4); // FIXME: don't sure of this
        }
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
        let internal_memory = Rc::new(RefCell::new(InternalMemory::new()));
        Self {
            rom,
            registers: Registers::default(),
            cpsr: Cpsr::default(),
            memory: internal_memory.clone(),
            ppu: PixelProcessUnit::new(internal_memory),
        }
    }

    fn branch(&mut self, op_code: ArmModeOpcode) -> bool {
        let offset = op_code.get_bits(0..=23);

        self.registers.advance_program_counter(8 + offset * 4);

        // Never advance PC after B
        false
    }

    fn branch_link(&mut self, op_code: ArmModeOpcode) -> bool {
        let pc: u32 = self.registers.program_counter().try_into().unwrap();
        self.registers.set_register_at(14, pc.wrapping_add(4)); // R14 = LR

        let offset = op_code.get_bits(0..=23);

        self.registers.advance_program_counter(8 + offset * 4);

        // Never advance PC after BL
        false
    }

    fn block_data_transfer(&mut self, op_code: ArmModeOpcode) -> bool {
        let pre_post = op_code.get_bit(24);
        let up_down = op_code.get_bit(23);
        let s = op_code.get_bit(22);
        if s {
            todo!()
        }
        let write_back = op_code.get_bit(21);
        let load_store = op_code.get_bit(20);
        let rn = op_code.get_bits(16..=19);
        let reg_list = op_code.get_bits(0..=15);

        let memory_base = self.registers.register_at(rn.try_into().unwrap());
        let mut address = memory_base;

        if load_store {
            let transfer = |arm: &mut Self, address: u32, reg_destination: usize| {
                let part_0: u32 = arm.memory.borrow().read_at(address).try_into().unwrap();
                let part_1: u32 = arm.memory.borrow().read_at(address + 1).try_into().unwrap();
                let part_2: u32 = arm.memory.borrow().read_at(address + 2).try_into().unwrap();
                let part_3: u32 = arm.memory.borrow().read_at(address + 3).try_into().unwrap();
                let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                arm.registers.set_register_at(reg_destination, v);
            };

            self.exec_data_trasfer(reg_list, pre_post, &mut address, up_down, transfer);
        } else {
            let transfer = |arm: &mut Self, address: u32, reg_source: usize| {
                let value = arm.registers.register_at(reg_source);
                arm.memory
                    .borrow_mut()
                    .write_at(address, value.get_bits(0..=7) as u8);
                arm.memory
                    .borrow_mut()
                    .write_at(address + 1, value.get_bits(8..=15) as u8);
                arm.memory
                    .borrow_mut()
                    .write_at(address + 2, value.get_bits(16..=23) as u8);
                arm.memory
                    .borrow_mut()
                    .write_at(address + 3, value.get_bits(24..=31) as u8);
            };

            self.exec_data_trasfer(reg_list, pre_post, &mut address, up_down, transfer);
        }

        if write_back {
            self.registers
                .set_register_at(rn.try_into().unwrap(), address);
        };

        // If LDM and R15 is in register list we don't advance PC
        !(load_store && reg_list.is_bit_on(15))
    }

    fn exec_data_trasfer<F>(
        &mut self,
        reg_list: u32,
        pre_post: bool,
        address: &mut u32,
        up_down: bool,
        trasfer: F,
    ) where
        F: Fn(&mut Self, u32, usize),
    {
        let alignment = 4; // Since are word, the alignment is 4.
        let change_address = |address: u32| {
            if up_down {
                address.wrapping_add(alignment)
            } else {
                address.wrapping_sub(alignment)
            }
        };

        for reg_source in 0..16 {
            if reg_list.is_bit_on(reg_source) {
                if pre_post {
                    *address = change_address(*address);
                }

                trasfer(self, *address, reg_source.into());

                if !pre_post {
                    *address = change_address(*address);
                }
            }
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
    #[should_panic]
    fn check_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.instruction, ArmModeInstruction::Unknown);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute(op_code);
    }

    #[test]
    fn check_block_data_transfer() {
        {
            // STR
            let op_code = 0b1110_1000_1011_1101_1000_0000_0000_0111;
            let mut cpu = Arm7tdmi::new(vec![]);
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(13, 0x03000000); // fake mem address simulate dirty reg.
            cpu.memory.borrow_mut().write_at(0x03000000, 10);
            cpu.memory.borrow_mut().write_at(0x03000000 + 4, 11);
            cpu.memory.borrow_mut().write_at(0x03000000 + 8, 12);
            cpu.memory.borrow_mut().write_at(0x03000000 + 12, 13);

            assert_eq!(op_code.instruction, ArmModeInstruction::BlockDataTransfer);
            assert_eq!(op_code.condition, Condition::AL);
            cpu.execute(op_code);

            assert_eq!(cpu.registers.program_counter(), 13);
            assert_eq!(cpu.registers.register_at(0), 10);
            assert_eq!(cpu.registers.register_at(1), 11);
            assert_eq!(cpu.registers.register_at(2), 12);

            assert_eq!(cpu.registers.register_at(13), 0x3000010);
        }
        {
            // LDR
            let op_code: u32 = 0b1110_1001_0010_1101_0100_0000_0000_0011;
            let mut cpu = Arm7tdmi::new(vec![]);
            let op_code = cpu.decode(op_code);
            assert_eq!(op_code.instruction, ArmModeInstruction::BlockDataTransfer);
            assert_eq!(op_code.condition, Condition::AL);

            // fake dirty status of registers.
            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.execute(op_code);
            assert_eq!(0, cpu.memory.borrow().read_at(9));
            assert_eq!(1, cpu.memory.borrow().read_at(5));
            assert_eq!(14, cpu.memory.borrow().read_at(1));
        }
    }
}
