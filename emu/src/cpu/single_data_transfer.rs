use crate::cpu::alu_instruction::shift;
use crate::cpu::flags::{Indexing, Offsetting, ShiftKind};
use crate::cpu::registers::REG_PROGRAM_COUNTER;
use crate::{bitwise::Bits, cpu::arm7tdmi::Arm7tdmi, memory::io_device::IoDevice};

use super::{arm7tdmi::SIZE_OF_ARM_INSTRUCTION, flags::ReadWriteKind};

/// Possible operation on transfer data.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleDataTransferOffsetInfo {
    Immediate {
        offset: u32,
    },
    RegisterImmediate {
        shift_amount: u32,
        shift_kind: ShiftKind,
        reg_offset: u32,
    },
}

impl std::fmt::Display for SingleDataTransferOffsetInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Immediate { offset } => {
                f.write_str("#")?;
                // FIXME: should we put the sign?
                write!(f, "{offset}")?;
            }
            Self::RegisterImmediate {
                shift_amount,
                shift_kind,
                reg_offset,
            } => {
                write!(f, "{reg_offset}, {shift_kind} #{shift_amount}")?;
            }
        };

        Ok(())
    }
}

impl Arm7tdmi {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn single_data_transfer(
        &mut self,
        kind: SingleDataTransferKind,
        quantity: ReadWriteKind,
        write_back: bool,
        indexing: Indexing,
        rd: u32,
        base_register: u32,
        offset_info: SingleDataTransferOffsetInfo,
        offsetting: Offsetting,
    ) -> Option<u32> {
        let address = if base_register == REG_PROGRAM_COUNTER {
            let pc: u32 = self.registers.program_counter().try_into().unwrap();
            pc + 8_u32
        } else {
            self.registers
                .register_at(base_register.try_into().unwrap())
        };

        let amount = match offset_info {
            SingleDataTransferOffsetInfo::Immediate { offset } => offset,
            SingleDataTransferOffsetInfo::RegisterImmediate {
                shift_amount,
                shift_kind,
                reg_offset,
            } => {
                let v = self.registers.register_at(reg_offset.try_into().unwrap());
                let r = shift(shift_kind, shift_amount, v, self.cpsr.carry_flag());
                r.result
            }
        };

        let address: usize = match offsetting {
            Offsetting::Down => address.wrapping_sub(amount).try_into().unwrap(),
            Offsetting::Up => address.wrapping_add(amount).try_into().unwrap(),
        };

        let v = match indexing {
            Indexing::Post => {
                todo!()
            }
            Indexing::Pre => {
                if write_back {
                    todo!()
                }
                (address, write_back)
            }
        };

        match kind {
            SingleDataTransferKind::Ldr => match quantity {
                ReadWriteKind::Byte => {
                    let value = self.memory.lock().unwrap().read_at(v.0) as u32;
                    self.registers
                        .set_register_at(rd.try_into().unwrap(), value)
                }
                ReadWriteKind::Word => {
                    let mem = self.memory.lock().unwrap();
                    let part_0: u32 = mem.read_at(address).try_into().unwrap();
                    let part_1: u32 = mem.read_at(address + 1).try_into().unwrap();
                    let part_2: u32 = mem.read_at(address + 2).try_into().unwrap();
                    let part_3: u32 = mem.read_at(address + 3).try_into().unwrap();
                    drop(mem);
                    let v = part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0;
                    self.registers.set_register_at(rd.try_into().unwrap(), v);
                }
            },
            SingleDataTransferKind::Str => match quantity {
                ReadWriteKind::Byte => self.memory.lock().unwrap().write_at(address, rd as u8),
                ReadWriteKind::Word => {
                    let mut v = self.registers.register_at(rd.try_into().unwrap());

                    // If R15 we get the value of the current instruction + 12
                    if rd == REG_PROGRAM_COUNTER {
                        v += 12;
                    }

                    self.memory
                        .lock()
                        .unwrap()
                        .write_at(address, v.get_bits(0..=7) as u8);
                    self.memory
                        .lock()
                        .unwrap()
                        .write_at(address + 1, v.get_bits(8..=15) as u8);
                    self.memory
                        .lock()
                        .unwrap()
                        .write_at(address + 2, v.get_bits(16..=23) as u8);
                    self.memory
                        .lock()
                        .unwrap()
                        .write_at(address + 3, v.get_bits(24..=31) as u8);
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
    use super::*;
    use crate::cpu::condition::Condition;
    use crate::cpu::instruction::ArmModeInstruction::SingleDataTransfer;
    use crate::cpu::opcode::ArmModeOpcode;
    use pretty_assertions::assert_eq;

    #[test]
    fn check_ldr() {
        {
            let op_code = 0b1110_01_0_1_1_1_0_1_1100_1100_001100000000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
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
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 768 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDRB R12, #768");
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000011010000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
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
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 208 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDR R13, #208");
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000010111000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
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
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 184 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDR R13, #184");
        }
        {
            let op_code = 0b1110_01_0_1_1_0_0_1_1111_1101_000011010000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
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
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 208 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDR R13, #208");
        }
        {
            let op_code = 0b1110_0101_1101_1111_1101_0000_0001_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Ldr,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 24 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "LDRB R13, #24");

            // because in this specific case address will be
            // then will be 0x03000050 + 8 (.wrapping_add(offset))
            cpu.registers.set_program_counter(0x03000050);

            // simulate mem already contains something.
            cpu.memory.lock().unwrap().write_at(0x03000070, 99);

            cpu.execute_arm(op_code);
            assert_eq!(cpu.registers.register_at(13), 99);
            assert_eq!(cpu.registers.program_counter(), 0x03000054);
        }
    }

    #[test]
    fn check_str() {
        {
            let op_code = 0b1110_01_0_1_1_1_0_0_0100_0100_001000001000;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
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
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 520 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "STRB R4, #520");
        }
        {
            let op_code: u32 = 0b1110_0101_1000_0001_0001_0000_0000_0000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Str,
                    quantity: ReadWriteKind::Word,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 1,
                    base_register: 1,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 0 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "STR R1, #0");

            cpu.registers.set_register_at(1, 16843009);

            // because in this specific case address will be
            // then will be 0x03000050 + 8 (.wrapping_sub(offset))
            cpu.registers.set_program_counter(0x03000050);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x01010101), 1);
            assert_eq!(memory.read_at(0x01010101 + 1), 1);
            assert_eq!(memory.read_at(0x01010101 + 2), 1);
            assert_eq!(memory.read_at(0x01010101 + 3), 1);
            assert_eq!(cpu.registers.program_counter(), 0x03000054);
        }
        {
            let op_code = 0b1110_0101_1100_1111_1101_0000_0001_1000;
            let mut cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                SingleDataTransfer {
                    condition: Condition::AL,
                    kind: SingleDataTransferKind::Str,
                    quantity: ReadWriteKind::Byte,
                    write_back: false,
                    indexing: Indexing::Pre,
                    rd: 13,
                    base_register: 15,
                    offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 24 },
                    offsetting: Offsetting::Up,
                }
            );
            let f = op_code.instruction.disassembler();
            assert_eq!(f, "STRB R13, #24");

            // because in this specific case address will be
            // then will be 0x03000050 + 8 (.wrapping_add(offset))
            cpu.registers.set_program_counter(0x03000050);

            cpu.execute_arm(op_code);

            let memory = cpu.memory.lock().unwrap();

            assert_eq!(memory.read_at(0x03000070), 13);
            assert_eq!(cpu.registers.program_counter(), 0x03000054);
        }
    }

    #[test]
    fn check_ldr_word() {
        let op_code = 0b1110_0101_1001_1111_1101_0000_0010_1000;
        let mut cpu = Arm7tdmi::default();
        let op_code: ArmModeOpcode = cpu.decode(op_code);
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
                offset_info: SingleDataTransferOffsetInfo::Immediate { offset: 40 },
                offsetting: Offsetting::Up,
            }
        );

        {
            let mut memory = cpu.memory.lock().unwrap();

            // simulate mem already contains something.
            // in u32 this is 16843009 00000001_00000001_00000001_00000001.
            memory.write_at(0x30, 1);
            memory.write_at(0x30 + 1, 1);
            memory.write_at(0x30 + 2, 1);
            memory.write_at(0x30 + 3, 1);
        }
        cpu.execute_arm(op_code);
        assert_eq!(cpu.registers.register_at(13), 16843009);
        assert_eq!(cpu.registers.program_counter(), 4);
    }
}
