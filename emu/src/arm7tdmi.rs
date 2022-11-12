use std::convert::TryInto;
use std::sync::{Arc, Mutex};

use logger::log;

use crate::bitwise::Bits;
use crate::instruction::ArmModeInstruction;
use crate::memory::internal_memory::InternalMemory;
use crate::memory::io_device::IoDevice;
use crate::opcode::ArmModeOpcode;
use crate::{cpsr::Cpsr, cpu::Cpu};

/// Contains the 16 registers for the CPU, latest (R15) is special because
/// is the program counter.
#[derive(Default)]
pub struct Registers([u32; 16]);

impl Registers {
    pub fn program_counter(&self) -> usize {
        self.0[15].try_into().unwrap()
    }

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

#[derive(Default)]
pub struct Arm7tdmi {
    pub(crate) rom: Arc<Mutex<Vec<u8>>>,
    pub(crate) memory: Arc<Mutex<InternalMemory>>,

    pub cpsr: Cpsr,
    pub registers: Registers,
}

const OPCODE_ARM_SIZE: usize = 4;

impl Cpu for Arm7tdmi {
    type OpCodeType = ArmModeOpcode;

    fn fetch(&self) -> u32 {
        let instruction_index = self.registers.program_counter();
        let end_instruction = instruction_index + OPCODE_ARM_SIZE;
        let data_instruction: [u8; 4] = self.rom.lock().unwrap()
            [instruction_index..end_instruction]
            .try_into()
            .expect("`istruction` conversion into [u8; 4]");

        u32::from_le_bytes(data_instruction)
    }

    fn decode(&self, op_code: u32) -> Self::OpCodeType {
        let op_code = ArmModeOpcode::try_from(op_code).unwrap();
        log(format!("{op_code}"));
        op_code
    }

    fn execute(&mut self, op_code: Self::OpCodeType) {
        use ArmModeInstruction::*;
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        let should_advance_pc = match op_code.instruction {
            DataProcessing => self.data_processing(op_code),
            Multiply => todo!(),
            MultiplyLong => todo!(),
            SingleDataSwap => todo!(),
            BranchAndExchange => todo!(),
            HalfwordDataTransferRegisterOffset => todo!(),
            HalfwordDataTransferImmediateOffset => todo!(),
            SingleDataTransfer => self.single_data_transfer(op_code),
            Undefined => todo!(),
            BlockDataTransfer => self.block_data_transfer(op_code),
            Branch => self.branch(op_code),
            CoprocessorDataTransfer => todo!(),
            CoprocessorDataOperation => todo!(),
            CoprocessorRegisterTrasfer => todo!(),
            SoftwareInterrupt => todo!(),
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
        } else {
            self.registers.advance_program_counter(4);
        }
    }
}

impl Arm7tdmi {
    pub fn new(rom: Arc<Mutex<Vec<u8>>>, memory: Arc<Mutex<InternalMemory>>) -> Self {
        Self {
            rom,
            registers: Registers::default(),
            cpsr: Cpsr::default(),
            memory,
        }
    }

    fn branch(&mut self, op_code: ArmModeOpcode) -> bool {
        let offset = op_code.get_bits(0..=23) << 2;

        // We need to sign-extend the 26 bit number into a 32 bit.
        // We can't just do `offset as i32` since it would just do a
        // zero extension.

        let mask = 1 << 25;
        let offset = (offset as i32 ^ mask) - mask;

        let old_pc: u32 = self.registers.program_counter().try_into().unwrap();
        let is_link = op_code.get_bit(24);
        if is_link {
            self.registers.set_register_at(14, old_pc.wrapping_add(4));
        }

        // 8 is for the prefetch
        let new_pc = self.registers.program_counter() as i32 + offset + 8;
        self.registers.set_program_counter(new_pc as u32);

        // Never advance PC after B
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
                let memory = arm.memory.lock().unwrap();

                let part_0: u32 = memory.read_at(address).try_into().unwrap();
                let part_1: u32 = memory.read_at(address + 1).try_into().unwrap();
                let part_2: u32 = memory.read_at(address + 2).try_into().unwrap();
                let part_3: u32 = memory.read_at(address + 3).try_into().unwrap();
                let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                arm.registers.set_register_at(reg_destination, v);
            };

            self.exec_data_trasfer(reg_list, pre_post, &mut address, up_down, transfer);
        } else {
            let transfer = |arm: &mut Self, address: u32, reg_source: usize| {
                let mut value = arm.registers.register_at(reg_source);

                // If R15 we get the value of the current instruction + 12
                if reg_source == 0xF {
                    value += 12;
                }
                let mut memory = arm.memory.lock().unwrap();

                memory.write_at(address, value.get_bits(0..=7) as u8);
                memory.write_at(address + 1, value.get_bits(8..=15) as u8);
                memory.write_at(address + 2, value.get_bits(16..=23) as u8);
                memory.write_at(address + 3, value.get_bits(24..=31) as u8);
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

        // If we are decreasing we still want to store the lowest reg to the lowest
        // memory address. For this reason we reverse the loop order.
        let range_registers: Box<dyn Iterator<Item = u8>> = if up_down {
            Box::new(0..=15)
        } else {
            Box::new((0..=15).rev())
        };

        for reg_source in range_registers {
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
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_branch() {
        // Covers a positive offset

        // 15(1111b) << 2 = 60 bytes
        let op_code = 0b1110_1010_0000_0000_0000_0000_0000_1111;
        let mut cpu = Arm7tdmi::default();
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.program_counter(), 68);

        // Covers a negative offset

        // -9 << 2 = -36 bytes
        let op_code = 0b1110_1010_1111_1111_1111_1111_1111_0111;
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.program_counter(), 68 + 8 - 36);

        // Covers link

        let op_code = 0b1110_1011_0000_0000_0000_0000_0000_1111;
        let op_code = cpu.decode(op_code);

        cpu.execute(op_code);

        assert_eq!(cpu.registers.register_at(14), 44);
    }

    #[test]
    #[should_panic]
    fn check_unknown_instruction() {
        let op_code = 0b1110_1111_1111_1111_1111_1111_1111_1111;
        let mut cpu = Arm7tdmi::default();

        let op_code = cpu.decode(op_code);
        assert_eq!(op_code.condition, Condition::AL);

        cpu.execute(op_code);
    }

    #[test]
    fn check_block_data_transfer() {
        {
            // LDM with post-increment
            let op_code = 0b1110_100_0_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x1000, 1);
                memory.write_at(0x1004, 5);
                memory.write_at(0x1008, 7);
            }
            cpu.execute(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // LDM with pre-increment
            let op_code = 0b1110_100_1_1_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x1004, 1);
                memory.write_at(0x1008, 5);
                memory.write_at(0x100C, 7);
            }
            cpu.execute(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // LDM with post-decrement
            let op_code = 0b1110_100_0_0_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x1000, 7);
                memory.write_at(0x0FFC, 5);
                memory.write_at(0x0FF8, 1);
            }
            cpu.execute(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x0FF4);
        }
        {
            // LDM with pre-decrement
            let op_code = 0b1110_100_1_0_0_1_1_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            cpu.registers.set_register_at(13, 0x1000);
            {
                let mut memory = cpu.memory.lock().unwrap();

                memory.write_at(0x0FFC, 7);
                memory.write_at(0x0FF8, 5);
                memory.write_at(0x0FF4, 1);
            }
            cpu.execute(op_code);

            assert_eq!(cpu.registers.register_at(1), 1);
            assert_eq!(cpu.registers.register_at(5), 5);
            assert_eq!(cpu.registers.register_at(7), 7);
            assert_eq!(cpu.registers.register_at(13), 0x0FF4);
        }
        {
            // STM with post-increment
            let op_code = 0b1110_100_0_1_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 1);
            assert_eq!(memory.read_at(0x1004), 5);
            assert_eq!(memory.read_at(0x1008), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // STM with pre-increment
            let op_code = 0b1110_100_1_1_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 0);
            assert_eq!(memory.read_at(0x1004), 1);
            assert_eq!(memory.read_at(0x1008), 5);
            assert_eq!(memory.read_at(0x100C), 7);
            assert_eq!(cpu.registers.register_at(13), 0x100C);
        }
        {
            // STM with post-decrement
            let op_code = 0b1110_100_0_0_0_1_0_1101_0000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 7);
            assert_eq!(memory.read_at(0x0FFC), 5);
            assert_eq!(memory.read_at(0x0FF8), 1);
            assert_eq!(cpu.registers.register_at(13), 0x0FF4);
        }
        {
            // STM with pre-decrement and storing R15

            let op_code = 0b1110_100_1_0_0_1_0_1101_1000000010100010;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            for r in 0..16 {
                cpu.registers.set_register_at(r, r as u32);
            }

            cpu.registers.set_register_at(13, 0x1000);

            cpu.execute(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x1000), 0);
            assert_eq!(memory.read_at(0x0FFC), 15 + 12);
            assert_eq!(memory.read_at(0x0FF8), 7);
            assert_eq!(memory.read_at(0x0FF4), 5);
            assert_eq!(memory.read_at(0x0FF0), 1);
            assert_eq!(cpu.registers.register_at(13), 0x0FF0);
        }
    }
}
