use std::convert::TryInto;
use std::sync::{Arc, Mutex};

use logger::log;

use crate::arm::cpu_modes::Mode;
use crate::arm::instruction::ArmModeInstruction;
use crate::arm::opcode::ArmModeOpcode;
use crate::arm::psr::Psr;
use crate::arm::register_bank::RegisterBank;
use crate::bitwise::Bits;
use crate::cpu::Cpu;
use crate::memory::internal_memory::InternalMemory;
use crate::memory::io_device::IoDevice;

use super::psr::CpuState;
use super::registers::Registers;

pub const REG_PROGRAM_COUNTER: u32 = 0xF;
const SIZE_OF_ARM_INSTRUCTION: u32 = 4;

pub struct Arm7tdmi {
    pub(crate) rom: Arc<Mutex<Vec<u8>>>,
    pub(crate) memory: Arc<Mutex<InternalMemory>>,

    pub cpsr: Psr,
    pub registers: Registers,

    pub register_bank: RegisterBank,
}

impl Default for Arm7tdmi {
    fn default() -> Self {
        let mut s = Self {
            rom: Arc::new(Mutex::new(Vec::default())),
            memory: Arc::new(Mutex::new(InternalMemory::default())),
            cpsr: Psr::from(Mode::User), // FIXME: Starting as User? Not sure
            registers: Registers::default(),
            register_bank: RegisterBank::default(),
        };

        // Setting ARM mode at startup
        s.cpsr.set_cpu_state(CpuState::Arm);

        s
    }
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
        if !self.cpsr.can_execute(op_code.condition) {
            return;
        }

        use ArmModeInstruction::*;
        // Instruction functions should return whether PC has to be advanced
        // after instruction executed.
        match op_code.instruction {
            DataProcessing => self.data_processing(op_code),
            Multiply => todo!(),
            MultiplyLong => todo!(),
            SingleDataSwap => todo!(),
            BranchAndExchange => todo!(),
            HalfwordDataTransferRegisterOffset => self.data_transfer_register_offset(op_code),
            HalfwordDataTransferImmediateOffset => self.data_transfer_immediate_offset(op_code),
            SingleDataTransfer => self.single_data_transfer(op_code),
            Undefined => todo!(),
            BlockDataTransfer => self.block_data_transfer(op_code),
            Branch => self.branch(op_code),
            CoprocessorDataTransfer => todo!(),
            CoprocessorDataOperation => todo!(),
            CoprocessorRegisterTrasfer => todo!(),
            SoftwareInterrupt => todo!(),
        };
    }

    fn step(&mut self) {
        let op_code = self.fetch();
        self.registers
            .advance_program_counter(SIZE_OF_ARM_INSTRUCTION);

        let op_code = self.decode(op_code);
        self.execute(op_code);
    }
}

impl Arm7tdmi {
    pub fn new(rom: Arc<Mutex<Vec<u8>>>, memory: Arc<Mutex<InternalMemory>>) -> Self {
        Self {
            memory,
            rom,
            ..Default::default()
        }
    }

    pub fn get_spsr(&self) -> Psr {
        match self.cpsr.mode() {
            Mode::User | Mode::System => panic!("Trying to access a SPSR in either User or System state which do not have banked SPSR."),
            Mode::Fiq => self.register_bank.spsr_fiq,
            Mode::Irq => self.register_bank.spsr_irq,
            Mode::Abort => self.register_bank.spsr_abt,
            Mode::Supervisor => self.register_bank.spsr_svc,
            Mode::Undefined => self.register_bank.spsr_und
        }
    }

    pub fn get_spsr_as_ref_mut(&mut self) -> &mut Psr {
        match self.cpsr.mode() {
            Mode::User | Mode::System => panic!("Trying to access a SPSR in either User or System state which do not have banked SPSR."),
            Mode::Fiq => &mut self.register_bank.spsr_fiq,
            Mode::Irq => &mut self.register_bank.spsr_irq,
            Mode::Abort => &mut self.register_bank.spsr_abt,
            Mode::Supervisor => &mut self.register_bank.spsr_svc,
            Mode::Undefined => &mut self.register_bank.spsr_und
        }
    }

    fn data_transfer_register_offset(&mut self, op_code: ArmModeOpcode) {
        let pre_post = op_code.get_bit(24);
        let up_down = op_code.get_bit(23);
        let _write_back = op_code.get_bit(21);
        let load_store = op_code.get_bit(20);
        let rn_base_register = op_code.get_bits(16..=19);
        let rd_source_destination_register = op_code.get_bits(12..=15);
        let transfer_type = HalfwordTransferType::from(op_code.get_bits(5..=6) as u8);
        let offset_register = op_code.get_bits(0..=3);

        let offset = self.registers.register_at(offset_register as usize);
        let mut address = self
            .registers
            .register_at(rn_base_register.try_into().unwrap());

        if rn_base_register == REG_PROGRAM_COUNTER {
            // prefetching
            let v: u32 = self.registers.program_counter().try_into().unwrap();
            address = address.wrapping_add(v + 8);
        }

        if pre_post {
            if up_down {
                address = address.wrapping_add(offset);
            } else {
                address = address.wrapping_sub(offset);
            }
        }

        if load_store {
            todo!()
        } else {
            let value = if rd_source_destination_register == 0xF {
                let v: u32 = self.registers.program_counter().try_into().unwrap();
                v + 12
            } else {
                self.registers
                    .register_at(rd_source_destination_register.try_into().unwrap())
            };

            match transfer_type {
                HalfwordTransferType::UnsignedHalfwords => {
                    if let Ok(mut mem) = self.memory.lock() {
                        mem.write_at(address, value.get_bits(0..=7) as u8);
                        mem.write_at(address + 1, value.get_bits(8..=15) as u8);
                    }
                }
                _ => unreachable!("HS flags invalid for STORE (L=0)"),
            };
        }

        if !pre_post {
            todo!()
        }
    }

    fn data_transfer_immediate_offset(&mut self, op_code: ArmModeOpcode) {
        let pre_post = op_code.get_bit(24);
        let up_down = op_code.get_bit(23);
        let _write_back = op_code.get_bit(21);
        let load_store = op_code.get_bit(20);
        let rn_base_register = op_code.get_bits(16..=19);
        let rd_source_destination_register = op_code.get_bits(12..=15);
        let transfer_type = HalfwordTransferType::from(op_code.get_bits(5..=6) as u8);
        let immediate_offset_high = op_code.get_bits(8..=11);
        let immediate_offset_low = op_code.get_bits(0..=3);

        let offset = (immediate_offset_high << 4) | immediate_offset_low;
        let mut address = self
            .registers
            .register_at(rn_base_register.try_into().unwrap());

        if rn_base_register == REG_PROGRAM_COUNTER {
            // prefetching
            let v: u32 = self.registers.program_counter().try_into().unwrap();
            address = address.wrapping_add(v + 8);
        }

        if pre_post {
            if up_down {
                address = address.wrapping_add(offset);
            } else {
                address = address.wrapping_sub(offset);
            }
        }

        if load_store {
            todo!("load from mem")
        } else {
            let value = if rd_source_destination_register == REG_PROGRAM_COUNTER {
                let pc: u32 = self.registers.program_counter().try_into().unwrap();
                pc + 12
            } else {
                self.registers
                    .register_at(rd_source_destination_register as usize)
            };

            match transfer_type {
                HalfwordTransferType::UnsignedHalfwords => {
                    if let Ok(mut mem) = self.memory.lock() {
                        mem.write_at(address, value.get_bits(0..=7) as u8);
                        mem.write_at(address + 1, value.get_bits(8..=15) as u8);
                    }
                }
                _ => unreachable!("HS flags can't be != from 01 for STORE (L=0)"),
            };
        }

        if !pre_post {
            // TODO: ignore write back (should be 0 in this case but...)
            todo!()
        }
    }

    /// Stores the banked registers of the current mode to the register bank.
    /// It should be used before leaving the current mode.
    pub fn store_registers_in_bank(&mut self) {
        match self.cpsr.mode() {
            Mode::User | Mode::System => {}
            Mode::Fiq => {
                self.register_bank.r8_fiq = self.registers.register_at(8);
                self.register_bank.r9_fiq = self.registers.register_at(9);
                self.register_bank.r10_fiq = self.registers.register_at(10);
                self.register_bank.r11_fiq = self.registers.register_at(11);
                self.register_bank.r12_fiq = self.registers.register_at(12);
                self.register_bank.r13_fiq = self.registers.register_at(13);
                self.register_bank.r14_fiq = self.registers.register_at(14);
                self.register_bank.spsr_fiq = self.cpsr;
            }
            Mode::Supervisor => {
                self.register_bank.r13_svc = self.registers.register_at(13);
                self.register_bank.r14_svc = self.registers.register_at(14);
                self.register_bank.spsr_svc = self.cpsr;
            }
            Mode::Abort => {
                self.register_bank.r13_abt = self.registers.register_at(13);
                self.register_bank.r14_abt = self.registers.register_at(14);
                self.register_bank.spsr_abt = self.cpsr;
            }
            Mode::Irq => {
                self.register_bank.r13_irq = self.registers.register_at(13);
                self.register_bank.r14_irq = self.registers.register_at(14);
                self.register_bank.spsr_irq = self.cpsr;
            }
            Mode::Undefined => {
                self.register_bank.r13_und = self.registers.register_at(13);
                self.register_bank.r14_und = self.registers.register_at(14);
                self.register_bank.spsr_und = self.cpsr;
            }
        }
    }

    /// Restore the banked registers of the current mode from the register bank.
    /// It should be used after changing the mode.
    pub fn restore_registers_from_bank(&mut self) {
        match self.cpsr.mode() {
            Mode::User | Mode::System => {}
            Mode::Fiq => {
                self.registers.set_register_at(8, self.register_bank.r8_fiq);
                self.registers.set_register_at(8, self.register_bank.r8_fiq);
                self.registers.set_register_at(8, self.register_bank.r8_fiq);
                self.registers.set_register_at(9, self.register_bank.r9_fiq);
                self.registers
                    .set_register_at(10, self.register_bank.r10_fiq);
                self.registers
                    .set_register_at(11, self.register_bank.r11_fiq);
                self.registers
                    .set_register_at(12, self.register_bank.r12_fiq);
                self.registers
                    .set_register_at(13, self.register_bank.r13_fiq);
                self.registers
                    .set_register_at(14, self.register_bank.r14_fiq);
                self.cpsr = self.register_bank.spsr_fiq;
            }
            Mode::Supervisor => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_svc);
                self.registers
                    .set_register_at(14, self.register_bank.r14_svc);
                self.cpsr = self.register_bank.spsr_svc;
            }
            Mode::Abort => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_abt);
                self.registers
                    .set_register_at(14, self.register_bank.r14_abt);
                self.cpsr = self.register_bank.spsr_abt;
            }
            Mode::Irq => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_irq);
                self.registers
                    .set_register_at(14, self.register_bank.r14_irq);
                self.cpsr = self.register_bank.spsr_irq;
            }
            Mode::Undefined => {
                self.registers
                    .set_register_at(13, self.register_bank.r13_und);
                self.registers
                    .set_register_at(14, self.register_bank.r14_und);
                self.cpsr = self.register_bank.spsr_und;
            }
        }
    }

    fn branch(&mut self, op_code: ArmModeOpcode) {
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
    }

    fn block_data_transfer(&mut self, op_code: ArmModeOpcode) {
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
                if reg_source == REG_PROGRAM_COUNTER.try_into().unwrap() {
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

pub enum HalfwordTransferType {
    UnsignedHalfwords,
    SignedByte,
    SignedHalfwords,
}

impl From<u8> for HalfwordTransferType {
    fn from(value: u8) -> Self {
        match value.get_bits(0..=1) {
            0b01 => Self::UnsignedHalfwords,
            0b10 => Self::SignedByte,
            0b11 => Self::SignedHalfwords,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::arm::condition::Condition;
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

    #[test]
    fn check_data_transfer_register_offset() {
        {
            let op_code = 0b1110_0001_1000_0010_0000_0000_1011_0001;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::HalfwordDataTransferRegisterOffset
            );

            cpu.registers.set_register_at(0, 16843009);
            cpu.execute(op_code);

            let memory = cpu.memory.lock().unwrap();
            assert_eq!(memory.read_at(0), 1);
            assert_eq!(memory.read_at(1), 1);
            // because we store halfword = 16bit
            assert_eq!(memory.read_at(2), 0);
            assert_eq!(memory.read_at(3), 0);
        }
    }

    #[test]
    fn check_data_transfer_immediate_offset() {
        {
            // Store halfword
            let op_code = 0b1110_0001_1100_0001_0000_0000_1011_0000;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode(op_code);

            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::HalfwordDataTransferImmediateOffset
            );

            cpu.registers.set_register_at(0, 16843009);
            cpu.execute(op_code);

            let memory = cpu.memory.lock().unwrap();
            assert_eq!(memory.read_at(0), 1);
            assert_eq!(memory.read_at(1), 1);
            // because we store halfword = 16bit
            assert_eq!(memory.read_at(2), 0);
            assert_eq!(memory.read_at(3), 0);
        }
    }

    #[test]
    fn check_store_in_bank() {
        let mut cpu = Arm7tdmi::default();
        cpu.cpsr.set_mode(Mode::Fiq);

        for i in 0..=15 {
            cpu.registers.set_register_at(i, i.try_into().unwrap());
        }

        cpu.store_registers_in_bank();

        assert_eq!(cpu.register_bank.r8_fiq, 8);
        assert_eq!(cpu.register_bank.r9_fiq, 9);
        assert_eq!(cpu.register_bank.r10_fiq, 10);
        assert_eq!(cpu.register_bank.r11_fiq, 11);
        assert_eq!(cpu.register_bank.r12_fiq, 12);
        assert_eq!(cpu.register_bank.r13_fiq, 13);
        assert_eq!(cpu.register_bank.r14_fiq, 14);
    }

    #[test]
    fn check_restore_registers() {
        let mut cpu = Arm7tdmi::default();
        cpu.register_bank.r8_fiq = 8;
        cpu.register_bank.r9_fiq = 9;
        cpu.register_bank.r10_fiq = 10;
        cpu.register_bank.r11_fiq = 11;
        cpu.register_bank.r12_fiq = 12;
        cpu.register_bank.r13_fiq = 13;
        cpu.register_bank.r14_fiq = 14;

        cpu.cpsr.set_mode(Mode::Fiq);
        cpu.restore_registers_from_bank();

        assert_eq!(cpu.registers.register_at(8), 8);
        assert_eq!(cpu.registers.register_at(9), 9);
        assert_eq!(cpu.registers.register_at(10), 10);
        assert_eq!(cpu.registers.register_at(11), 11);
        assert_eq!(cpu.registers.register_at(12), 12);
        assert_eq!(cpu.registers.register_at(13), 13);
        assert_eq!(cpu.registers.register_at(14), 14);
    }
}
