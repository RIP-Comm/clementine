use crate::cpu::flags::{Indexing, Offsetting};
use crate::{bitwise::Bits, cpu::arm7tdmi::Arm7tdmi, memory::io_device::IoDevice};

use super::{
    arm7tdmi::{REG_PROGRAM_COUNTER, SIZE_OF_ARM_INSTRUCTION},
    flags::ReadWriteKind,
};

/// Possible opeartion on transfer data.
#[derive(Debug, Eq, PartialEq)]
pub enum SingleDataTransferKind {
    /// Load from memory into a register.
    Ldr,

    /// Store from a register into memory.
    Str,
    Pld,
}

impl From<u32> for SingleDataTransferKind {
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

impl Arm7tdmi {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn single_data_transfer(
        &mut self,
        kind: SingleDataTransferKind,
        quantity: ReadWriteKind,
        _write_back: bool,   // FIXME: should we use this?
        _indexing: Indexing, // FIXME: should we use this?
        rd: u32,
        base_register: u32,
        offset: u32,
        offsetting: Offsetting,
    ) -> Option<u32> {
        let address = if base_register == REG_PROGRAM_COUNTER {
            let pc: u32 = self.registers.program_counter().try_into().unwrap();
            pc + 8_u32
        } else {
            self.registers
                .register_at(base_register.try_into().unwrap())
        };

        let address: usize = match offsetting {
            Offsetting::Down => address.wrapping_sub(offset).try_into().unwrap(),
            Offsetting::Up => address.wrapping_add(offset).try_into().unwrap(),
        };

        let mut memory = self.memory.lock().unwrap();
        match kind {
            SingleDataTransferKind::Ldr => match quantity {
                ReadWriteKind::Byte => self
                    .registers
                    .set_register_at(rd.try_into().unwrap(), memory.read_at(address) as u32),
                ReadWriteKind::Word => {
                    let part_0: u32 = memory.read_at(address).try_into().unwrap();
                    let part_1: u32 = memory.read_at(address + 1).try_into().unwrap();
                    let part_2: u32 = memory.read_at(address + 2).try_into().unwrap();
                    let part_3: u32 = memory.read_at(address + 3).try_into().unwrap();

                    let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                    self.registers.set_register_at(rd.try_into().unwrap(), v);
                }
            },
            SingleDataTransferKind::Str => match quantity {
                ReadWriteKind::Byte => memory.write_at(address, rd as u8),
                ReadWriteKind::Word => {
                    let mut v = self.registers.register_at(rd.try_into().unwrap());

                    // If R15 we get the value of the current instruction + 12
                    if rd == REG_PROGRAM_COUNTER {
                        v += 12;
                    }

                    memory.write_at(address, v.get_bits(0..=7) as u8);
                    memory.write_at(address + 1, v.get_bits(8..=15) as u8);
                    memory.write_at(address + 2, v.get_bits(16..=23) as u8);
                    memory.write_at(address + 3, v.get_bits(24..=31) as u8);
                }
            },
            _ => todo!("implement single data transfer operation"),
        }

        // If LDR and Rd == R15 we don't increase the PC
        if !(kind == SingleDataTransferKind::Ldr && rd == REG_PROGRAM_COUNTER) {
            Some(SIZE_OF_ARM_INSTRUCTION)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{cpu::arm7tdmi::Arm7tdmi, cpu::instruction::ArmModeInstruction};

    use crate::cpu::condition::Condition;
    use crate::cpu::flags::{Indexing, Offsetting, ReadWriteKind};
    use crate::cpu::single_data_transfer::SingleDataTransferKind;
    use pretty_assertions::assert_eq;
    use ArmModeInstruction::SingleDataTransfer;

    #[test]
    fn check_ldr() {
        {
            let op_code = 0b1110_01_0_1_1_1_0_1_1100_1100_001100000000;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode_arm_mode_opcode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 12,
                    base_register: 12,
                    offset: 768,
                    offsetting: Offsetting::Up,
                }
            );
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000011010000;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode_arm_mode_opcode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset: 208,
                    offsetting: Offsetting::Up,
                }
            );
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000010111000;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode_arm_mode_opcode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset: 184,
                    offsetting: Offsetting::Up,
                }
            );
        }

        //     let op_code = 0b1110_0101_1101_1111_1101_0000_0001_1000;
        //
        //     let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
        //         .try_into()
        //         .expect("conversion `rd` to u8");
        //
        //     assert_eq!(rd, 13);
        //
        //     // because in this specific case address will be
        //     // then will be 0x03000050 + 8 (.wrapping_add(offset))
        //     cpu.registers.set_program_counter(0x03000050);
        //
        //     // simulate mem already contains something.
        //     cpu.memory.lock().unwrap().write_at(0x03000070, 99);
        //
        //     cpu.execute_arm(op_code_type);
        //     assert_eq!(cpu.registers.register_at(13), 99);
        //     assert_eq!(cpu.registers.program_counter(), 0x03000054);
    }

    #[test]
    fn check_str_byte() {
        {
            let op_code = 0b1110_01_0_1_1_1_0_0_0100_0100_001000001000;
            let mut cpu = Arm7tdmi::default();
            let op_code = cpu.decode_arm_mode_opcode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Str,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 4,
                    base_register: 4,
                    offset: 520,
                    offsetting: Offsetting::Up,
                }
            );
        }
        //     let op_code = 0b1110_0101_1100_1111_1101_0000_0001_1000;
        //     let mut cpu = Arm7tdmi::default();
        //     let op_code_type = cpu.decode_arm_mode_opcode(op_code);
        //     assert_eq!(
        //         op_code_type.instruction,
        //         SingleDataTransfer {
        //             condition: Condition::EQ,
        //             kind: SingleDataTransferKind::Ldr,
        //             quantity: Default::default(),
        //             write_back: false,
        //             indexing: Indexing::Post,
        //             rd: 0,
        //             base_register: 0,
        //             offset: 0,
        //             offsetting: Offsetting::Down,
        //         }
        //     );
        //
        //     let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
        //         .try_into()
        //         .expect("conversion `rd` to u8");
        //
        //     assert_eq!(rd, 13);
        //
        //     // because in this specific case address will be
        //     // then will be 0x03000050 + 8 (.wrapping_add(offset))
        //     cpu.registers.set_program_counter(0x03000050);
        //
        //     cpu.execute_arm(op_code_type);
        //
        //     let memory = cpu.memory.lock().unwrap();
        //
        //     assert_eq!(memory.read_at(0x03000070), 13);
        //     assert_eq!(cpu.registers.program_counter(), 0x03000054);
    }
    //
    // #[test]
    // fn check_ldr_word() {
    //     let op_code = 0b1110_0101_1001_1111_1101_0000_0010_1000;
    //     let mut cpu = Arm7tdmi::default();
    //     let op_code_type = cpu.decode_arm_mode_opcode(op_code);
    //     assert_eq!(
    //         op_code_type.instruction,
    //         SingleDataTransfer {
    //             condition: Condition::EQ,
    //             kind: SingleDataTransferKind::Ldr,
    //             quantity: Default::default(),
    //             write_back: false,
    //             indexing: Indexing::Post,
    //             rd: 0,
    //             base_register: 0,
    //             offset: 0,
    //             offsetting: Offsetting::Down,
    //         }
    //     );
    //
    //     let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
    //         .try_into()
    //         .expect("conversion `rd` to u8");
    //
    //     assert_eq!(rd, 13);
    //
    //     {
    //         let mut memory = cpu.memory.lock().unwrap();
    //
    //         // simulate mem already contains something.
    //         // in u32 this is 16843009 00000001_00000001_00000001_00000001.
    //         memory.write_at(0x30, 1);
    //         memory.write_at(0x30 + 1, 1);
    //         memory.write_at(0x30 + 2, 1);
    //         memory.write_at(0x30 + 3, 1);
    //     }
    //     cpu.execute_arm(op_code_type);
    //     assert_eq!(cpu.registers.register_at(13), 16843009);
    //     assert_eq!(cpu.registers.program_counter(), 4);
    // }
    //
    // #[test]
    // fn check_str_word() {
    //     let op_code: u32 = 0b1110_0101_1000_0001_0001_0000_0000_0000;
    //     let mut cpu = Arm7tdmi::default();
    //     let op_code_type = cpu.decode_arm_mode_opcode(op_code);
    //     assert_eq!(
    //         op_code_type.instruction,
    //         SingleDataTransfer {
    //             condition: Condition::EQ,
    //             kind: SingleDataTransferKind::Ldr,
    //             quantity: Default::default(),
    //             write_back: false,
    //             indexing: Indexing::Post,
    //             rd: 0,
    //             base_register: 0,
    //             offset: 0,
    //             offsetting: Offsetting::Down,
    //         }
    //     );
    //
    //     let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
    //         .try_into()
    //         .expect("conversion `rd` to u8");
    //
    //     assert_eq!(rd, 1);
    //     cpu.registers.set_register_at(1, 16843009);
    //
    //     // because in this specific case address will be
    //     // then will be 0x03000050 + 8 (.wrapping_sub(offset))
    //     cpu.registers.set_program_counter(0x03000050);
    //
    //     cpu.execute_arm(op_code_type);
    //
    //     let memory = cpu.memory.lock().unwrap();
    //
    //     assert_eq!(memory.read_at(0x01010101), 1);
    //     assert_eq!(memory.read_at(0x01010101 + 1), 1);
    //     assert_eq!(memory.read_at(0x01010101 + 2), 1);
    //     assert_eq!(memory.read_at(0x01010101 + 3), 1);
    //     assert_eq!(cpu.registers.program_counter(), 0x03000054);
    // }
}
