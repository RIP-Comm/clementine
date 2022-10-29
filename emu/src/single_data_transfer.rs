use crate::{
    arm7tdmi::Arm7tdmi, bitwise::Bits, memory::io_device::IoDevice, opcode::ArmModeOpcode,
};

/// Possible opeartion on transfer data.
#[derive(PartialEq)]
enum SingleDataTransfer {
    /// Load from memory into a register.
    Ldr,

    /// Store from a register into memory.
    Str,
    Pld,
}

impl From<ArmModeOpcode> for SingleDataTransfer {
    fn from(op_code: ArmModeOpcode) -> Self {
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

/// There two different kind of write or read for memory.
#[derive(Default)]
enum ReadWriteKind {
    /// Word is a u32 value for ARM mode and u16 for Thumb mode.
    #[default]
    Word,

    /// Byte is a u8 value.
    Byte,
}

impl From<bool> for ReadWriteKind {
    fn from(value: bool) -> Self {
        if value {
            Self::Byte
        } else {
            Self::Word
        }
    }
}

impl From<&ArmModeOpcode> for ReadWriteKind {
    fn from(op_code: &ArmModeOpcode) -> Self {
        op_code.get_bit(22).into()
    }
}

impl Arm7tdmi {
    pub(crate) fn single_data_transfer(&mut self, op_code: ArmModeOpcode) -> bool {
        let immediate = op_code.get_bit(25);
        let up_down = op_code.get_bit(23);
        let byte_or_word: ReadWriteKind = (&op_code).into();

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

        let offset = if immediate {
            todo!()
        } else {
            op_code.get_bits(0..=11)
        };

        let load_store: SingleDataTransfer = op_code.into();

        let address = if up_down {
            address.wrapping_sub(offset)
        } else {
            address.wrapping_add(offset)
        };

        match load_store {
            SingleDataTransfer::Ldr => match byte_or_word {
                ReadWriteKind::Byte => self
                    .registers
                    .set_register_at(rd.try_into().unwrap(), self.memory.read_at(address) as u32),
                ReadWriteKind::Word => todo!(),
            },
            SingleDataTransfer::Str => match byte_or_word {
                ReadWriteKind::Byte => self.memory.write_at(address, rd as u8),
                ReadWriteKind::Word => todo!(),
            },
            _ => todo!("implement single data transfer operation"),
        }

        // If LDR and Rd == R15 we don't increase the PC
        !(load_store == SingleDataTransfer::Ldr && rd == 0xF)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        arm7tdmi::Arm7tdmi, cpu::Cpu, instruction::ArmModeInstruction, memory::io_device::IoDevice,
    };

    #[test]
    fn check_ldr_byte() {
        let op_code = 0b1110_0101_1101_1111_1101_0000_0001_1000;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code_type = cpu.decode(op_code);
        assert_eq!(op_code_type.instruction, ArmModeInstruction::TransImm9);

        let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
            .try_into()
            .expect("conversion `rd` to u8");

        assert_eq!(rd, 13);

        // because in this specific case address will be
        // then will be 0x03000050 + 8 (.wrapping_sub(offset))
        cpu.registers.set_program_counter(0x03000050);

        // simulate mem already contains something.
        cpu.memory.write_at(0x03000040, 99);

        cpu.execute(op_code_type);
        assert_eq!(cpu.registers.register_at(13), 99);
        assert_eq!(cpu.registers.program_counter(), 0x03000054);
    }

    #[test]
    fn check_str_byte() {
        let op_code = 0b1110_0101_1100_1111_1101_0000_0001_1000;
        let mut cpu = Arm7tdmi::new(vec![]);

        let op_code_type = cpu.decode(op_code);
        assert_eq!(op_code_type.instruction, ArmModeInstruction::TransImm9);

        let rd: u8 = ((op_code & 0b0000_0000_0000_0000_1111_0000_0000_0000) >> 12)
            .try_into()
            .expect("conversion `rd` to u8");

        assert_eq!(rd, 13);

        // because in this specific case address will be
        // then will be 0x03000050 + 8 (.wrapping_sub(offset))
        cpu.registers.set_program_counter(0x03000050);

        cpu.execute(op_code_type);
        assert_eq!(cpu.memory.read_at(0x03000040), 13);
        assert_eq!(cpu.registers.program_counter(), 0x03000054);
    }
}
