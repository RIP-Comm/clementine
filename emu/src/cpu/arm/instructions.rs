use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::{
    AluSecondOperandInfo, ArmModeAluInstruction, ShiftOperator,
};
use crate::cpu::arm7tdmi::HalfwordTransferKind;
use crate::cpu::condition::Condition;
use crate::cpu::flags::{
    HalfwordDataTransferOffsetKind, Indexing, LoadStoreKind, Offsetting, OperandKind,
    ReadWriteKind, ShiftKind,
};
use logger::log;

use super::alu_instruction::{PsrKind, PsrOpKind};

/// Possible operation on transfer data.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
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
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ArmModeInstruction {
    DataProcessing {
        condition: Condition,
        alu_instruction: ArmModeAluInstruction,
        set_conditions: bool,
        op_kind: OperandKind,
        rn: u32,
        destination: u32,
        op2: AluSecondOperandInfo,
    },
    Multiply {
        variant: ArmModeMultiplyVariant,
        condition: Condition,
        should_set_codes: bool,
        rd_destination_register: u32,
        rn_accumulate_register: u32,
        rs_operand_register: u32,
        rm_operand_register: u32,
    },
    MultiplyLong {
        variant: ArmModeMultiplyLongVariant,
        condition: Condition,
        should_set_codes: bool,
        rdhi_destination_register: u32,
        rdlo_destination_register: u32,
        rs_operand_register: u32,
        rm_operand_register: u32,
    },
    PSRTransfer {
        condition: Condition,
        psr_kind: PsrKind,
        kind: PsrOpKind,
    },
    SingleDataSwap,
    BranchAndExchange {
        condition: Condition,
        register: usize,
    },
    HalfwordDataTransfer {
        condition: Condition,
        indexing: Indexing,
        offsetting: Offsetting,
        write_back: bool,
        load_store_kind: LoadStoreKind,
        offset_kind: HalfwordDataTransferOffsetKind,
        base_register: u32,
        source_destination_register: u32,
        transfer_kind: HalfwordTransferKind,
    },
    SingleDataTransfer {
        condition: Condition,
        kind: SingleDataTransferKind,
        quantity: ReadWriteKind,
        write_back: bool,
        indexing: Indexing,
        rd: u32,
        base_register: u32,
        offset_info: SingleDataTransferOffsetInfo,
        offsetting: Offsetting,
    },
    Undefined,
    BlockDataTransfer {
        condition: Condition,
        indexing: Indexing,
        offsetting: Offsetting,
        load_psr: bool,
        write_back: bool,
        load_store: LoadStoreKind,
        rn: u32,
        register_list: u32,
    },
    Branch {
        condition: Condition,
        link: bool,
        offset: u32,
    },
    CoprocessorDataTransfer {
        condition: Condition,
        indexing: Indexing,
        offsetting: Offsetting,
        transfer_length: bool,
        write_back: bool,
        load_store: LoadStoreKind,
        rn: u32,
        crd: u32,
        cp_number: u32,
        offset: u32,
    },
    CoprocessorDataOperation,
    CoprocessorRegisterTransfer,
    SoftwareInterrupt,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArmModeMultiplyVariant {
    Mul,
    Mla,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArmModeMultiplyLongVariant {
    Umull,
    Umlal,
    Smull,
    Smlal,
}

impl From<u32> for ArmModeMultiplyVariant {
    fn from(op_code: u32) -> Self {
        let mul_op_code: u32 = op_code.get_bits(21..=24);
        match mul_op_code {
            0b0000 => Self::Mul,
            0b0001 => Self::Mla,
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for ArmModeMultiplyVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<u32> for ArmModeMultiplyLongVariant {
    fn from(op_code: u32) -> Self {
        let mul_op_code: u32 = op_code.get_bits(21..=24);
        match mul_op_code {
            0b0100 => Self::Umull,
            0b0101 => Self::Umlal,
            0b0110 => Self::Smull,
            0b0111 => Self::Smlal,
            _ => unreachable!(),
        }
    }
}

impl std::fmt::Display for ArmModeMultiplyLongVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ArmModeInstruction {
    pub(crate) fn disassembler(&self) -> String {
        match self {
            Self::DataProcessing {
                condition,
                alu_instruction,
                set_conditions,
                op_kind: _,
                rn,
                destination,
                op2,
            } => {
                let set_string = if *set_conditions { "S" } else { "" };
                match alu_instruction {
                    ArmModeAluInstruction::And
                    | ArmModeAluInstruction::Eor
                    | ArmModeAluInstruction::Sub
                    | ArmModeAluInstruction::Rsb
                    | ArmModeAluInstruction::Add
                    | ArmModeAluInstruction::Adc
                    | ArmModeAluInstruction::Sbc
                    | ArmModeAluInstruction::Rsc
                    | ArmModeAluInstruction::Orr
                    | ArmModeAluInstruction::Bic => {
                        format!(
                            "{alu_instruction}{condition}{set_string} R{destination}, R{rn}, {op2}"
                        )
                    }
                    ArmModeAluInstruction::Tst
                    | ArmModeAluInstruction::Teq
                    | ArmModeAluInstruction::Cmp
                    | ArmModeAluInstruction::Cmn => {
                        format!("{alu_instruction}{condition} R{rn}, {op2}")
                    }
                    ArmModeAluInstruction::Mov | ArmModeAluInstruction::Mvn => {
                        format!("{alu_instruction}{condition}{set_string} R{destination}, {op2}")
                    }
                }
            },
            Self::Multiply {
                variant,
                condition,
                should_set_codes,
                rd_destination_register,
                rn_accumulate_register,
                rs_operand_register,
                rm_operand_register,
            } => {
                match variant {
                    ArmModeMultiplyVariant::Mul =>
                        format!("MUL{condition}{should_set_codes} {rd_destination_register}, {rm_operand_register}, {rs_operand_register}"),
                    ArmModeMultiplyVariant::Mla =>
                        format!("MLA{condition}{should_set_codes} {rd_destination_register}, {rm_operand_register}, {rs_operand_register}, {rn_accumulate_register}"),
                }
            },
            Self::MultiplyLong {
                variant,
                condition,
                should_set_codes,
                rdhi_destination_register,
                rdlo_destination_register,
                rs_operand_register,
                rm_operand_register,
            } => {
                format!("{variant}{condition}{should_set_codes} {rdlo_destination_register}, {rdhi_destination_register}, {rm_operand_register}, {rs_operand_register}")
            },
            Self::PSRTransfer {
                condition,
                psr_kind,
                kind,
            } => match kind {
                PsrOpKind::Mrs {
                    destination_register,
                } => {
                    format!("MRS{condition} R{destination_register}, {psr_kind}")
                }
                PsrOpKind::Msr { source_register } => {
                    format!("MSR{condition} {psr_kind}, R{source_register}")
                }
                PsrOpKind::MsrFlg { operand } => {
                    format!("MSR{condition} {psr_kind}_flg, {operand}")
                }
            },
            Self::SingleDataSwap => panic!("SingleDataSwap not implemented"),
            Self::BranchAndExchange {
                condition,
                register,
            } => format!("BX{condition} R{register}"),
            Self::HalfwordDataTransfer {
                condition,
                indexing,
                offsetting,
                load_store_kind,
                transfer_kind,
                source_destination_register,
                offset_kind,
                base_register,
                write_back,
                ..
            } => {
                let sign = match offsetting {
                    Offsetting::Up => "+",
                    Offsetting::Down => "-",
                };

                let offset = match offset_kind {
                    HalfwordDataTransferOffsetKind::Immediate { offset } => {
                        if *offset == 0 {
                            String::new()
                        } else {
                            format!(",#{sign}{offset}")
                        }
                    }
                    HalfwordDataTransferOffsetKind::Register { register } => {
                        format!(",{sign}R{register}")
                    }
                };

                let w = if *write_back { "!" } else { "" };

                let address = match indexing {
                    Indexing::Pre => {
                        format!("[R{base_register}{offset}{w}]")
                    }
                    Indexing::Post => {
                        format!("[R{base_register}]{offset}")
                    }
                };

                format!("{load_store_kind}{condition}{transfer_kind} R{source_destination_register}, {address}")
            }
            Self::SingleDataTransfer {
                condition,
                kind,
                quantity,
                write_back,
                indexing,
                rd,
                base_register: _,
                offset_info,
                offsetting: _,
            } => {
                let b = match quantity {
                    ReadWriteKind::Word => "",
                    ReadWriteKind::Byte => "B",
                };

                let _ = write_back;
                let _ = offset_info;
                let _ = indexing;

                // let address = base_register;

                let op = match kind {
                    SingleDataTransferKind::Ldr => "LDR",
                    SingleDataTransferKind::Str => "STR",
                    SingleDataTransferKind::Pld => unimplemented!(),
                };

                format!("{op}{condition}{b} R{rd}, {offset_info}")
            }
            Self::Undefined => panic!("Undefined not implemented"),
            Self::BlockDataTransfer {
                condition,
                indexing,
                offsetting,
                load_psr,
                write_back,
                load_store,
                rn,
                register_list,
            } => {
                let op = match load_store {
                    LoadStoreKind::Store => "STM",
                    LoadStoreKind::Load => "LDM",
                };

                let offset_modifier = match offsetting {
                    Offsetting::Down => "D",
                    Offsetting::Up => "I",
                };
                let index_type = match indexing {
                    Indexing::Pre => "B",
                    Indexing::Post => "A",
                };

                let mut registers = String::new();
                for i in 0..=15 {
                    if register_list.get_bit(i) {
                        registers.push_str(&format!("R{i}, "));
                    }
                }

                let w = if *write_back { "!" } else { "" };
                let f = if *load_psr { "^" } else { "" };
                format!("{op}{condition}{offset_modifier}{index_type}, R{rn}{w} {{{registers}}}{f}")
            }
            Self::Branch {
                condition,
                link,
                offset,
            } => {
                let link = if *link { "L" } else { "" };
                format!("B{link}{condition} 0x{offset:08X}")
            }
            Self::CoprocessorDataTransfer {
                condition,
                indexing,
                offsetting,
                transfer_length,
                write_back,
                load_store,
                rn,
                crd,
                cp_number,
                offset,
            } => {
                // <LDC|STC>{cond}{L} p#,cd,<Address>
                let op = match load_store {
                    LoadStoreKind::Store => "SDC",
                    LoadStoreKind::Load => "LDC",
                };
                let long_transfer = if *transfer_length { "L" } else { "" };

                let address = offset;
                let _ = rn;
                let _ = write_back;
                let _ = offsetting;
                let _ = indexing;
                // FIXME: Finish address
                format!("{op}{condition}{long_transfer} p{cp_number},{crd},{address:08X}")
            }
            Self::CoprocessorDataOperation => panic!("CoprocessorDataOperation not implemented"),
            Self::CoprocessorRegisterTransfer => panic!("CoprocessorRegisterTransfer not implemented"),
            Self::SoftwareInterrupt => panic!("SoftwareInterrupt not implemented"),
        }
    }
}

impl From<u32> for ArmModeInstruction {
    fn from(op_code: u32) -> Self {
        use ArmModeInstruction::*;

        let condition = Condition::from(op_code.get_bits(28..=31) as u8);
        // NOTE: The order is based on how many bits are already know at decoding time.
        // It can happen `op_code` coalesced into one/two or more than two possible solution, that's because
        // we tried to order with this priority.
        if op_code.get_bits(4..=27) == 0b0001_0010_1111_1111_1111_0001 {
            let register = op_code.get_bits(0..=3) as usize;
            BranchAndExchange {
                condition,
                register,
            }
        } else if op_code.get_bits(23..=27) == 0b00010
            && op_code.get_bits(20..=21) == 0b00
            && op_code.get_bits(4..=11) == 0b0000_1001
        {
            SingleDataSwap
        } else if op_code.get_bits(23..=27) == 0b00001 && op_code.get_bits(4..=7) == 0b1001 {
            let variant = ArmModeMultiplyLongVariant::from(op_code);

            let should_set_codes = op_code.get_bit(20);

            let rm_operand_register = op_code.get_bits(0..=3);
            let rs_operand_register = op_code.get_bits(8..=11);
            let rdlo_destination_register = op_code.get_bits(12..=15);
            let rdhi_destination_register = op_code.get_bits(16..=19);

            MultiplyLong {
                variant,
                condition,
                should_set_codes,
                rdhi_destination_register,
                rdlo_destination_register,
                rm_operand_register,
                rs_operand_register,
            }
        } else if op_code.get_bits(22..=27) == 0b000000 && op_code.get_bits(4..=7) == 0b1001 {
            let variant = ArmModeMultiplyVariant::from(op_code);

            let should_set_codes = op_code.get_bit(20);

            let rm_operand_register = op_code.get_bits(0..=3);
            let rs_operand_register = op_code.get_bits(8..=11);
            let rn_accumulate_register = op_code.get_bits(12..=15);
            let rd_destination_register = op_code.get_bits(16..=19);

            Multiply {
                variant,
                condition,
                should_set_codes,
                rd_destination_register,
                rn_accumulate_register,
                rm_operand_register,
                rs_operand_register,
            }
        } else if op_code.get_bits(25..=27) == 0b000 && op_code.get_bit(7) && op_code.get_bit(4) {
            let indexing: Indexing = op_code.get_bit(24).into();
            let offsetting: Offsetting = op_code.get_bit(23).into();
            let write_back = op_code.get_bit(21);
            let load_store_kind: LoadStoreKind = op_code.get_bit(20).into();
            let base_register = op_code.get_bits(16..=19);
            let source_destination_register = op_code.get_bits(12..=15);
            let transfer_kind: HalfwordTransferKind = (op_code.get_bits(5..=6) as u8).into();
            let operand_kind: OperandKind = op_code.get_bit(22).into();

            HalfwordDataTransfer {
                condition,
                indexing,
                offsetting,
                write_back,
                load_store_kind,
                offset_kind: if operand_kind == OperandKind::Register {
                    HalfwordDataTransferOffsetKind::Register {
                        register: op_code.get_bits(0..=3),
                    }
                } else {
                    let immediate_offset_high = op_code.get_bits(8..=11);
                    let immediate_offset_low = op_code.get_bits(0..=3);

                    HalfwordDataTransferOffsetKind::Immediate {
                        offset: (immediate_offset_high << 4) | immediate_offset_low,
                    }
                },
                base_register,
                source_destination_register,
                transfer_kind,
            }
        } else if op_code.get_bits(25..=27) == 0b011 && op_code.get_bit(4) {
            log("undefined instruction decode...");
            Undefined
        } else if op_code.get_bits(24..=27) == 0b1111 {
            SoftwareInterrupt
        } else if op_code.get_bits(24..=27) == 0b1110 && op_code.get_bit(4) {
            CoprocessorRegisterTransfer
        } else if op_code.get_bits(24..=27) == 0b1110 && !op_code.get_bit(4) {
            CoprocessorDataOperation
        } else if op_code.get_bits(25..=27) == 0b110 {
            let indexing: Indexing = op_code.get_bit(24).into();
            let offsetting: Offsetting = op_code.get_bit(23).into();
            let transfer_length = op_code.get_bit(22);
            let write_back = op_code.get_bit(21);
            let load_store: LoadStoreKind = op_code.get_bit(20).into();

            let rn = op_code.get_bits(16..=19);
            let crd = op_code.get_bits(12..=15);
            let cp_number = op_code.get_bits(8..=11);
            let offset = op_code.get_bits(0..=7);

            CoprocessorDataTransfer {
                condition,
                indexing,
                offsetting,
                transfer_length,
                write_back,
                load_store,
                rn,
                crd,
                cp_number,
                offset,
            }
        } else if op_code.get_bits(25..=27) == 0b100 {
            let indexing = op_code.get_bit(24).into();
            let offsetting = op_code.get_bit(23).into();
            let load_psr = op_code.get_bit(22);
            let write_back = op_code.get_bit(21);
            let load_store = op_code.get_bit(20).into();
            let rn = op_code.get_bits(16..=19);
            let reg_list = op_code.get_bits(0..=15);

            BlockDataTransfer {
                condition,
                indexing,
                offsetting,
                load_psr,
                write_back,
                load_store,
                rn,
                register_list: reg_list,
            }
        } else if op_code.get_bits(25..=27) == 0b101 {
            let link = op_code.get_bit(24);
            let offset = op_code.get_bits(0..=23) << 2;
            Branch {
                condition,
                link,
                offset,
            }
        } else if op_code.get_bits(26..=27) == 0b01 {
            // NOTE: This bit is negated because the meaning is inverted in SingleDataTransfer then other istructions.
            let op_kind: OperandKind = (!op_code.get_bit(25)).into();
            let indexing: Indexing = op_code.get_bit(24).into(); // FIXME: should we use this?
            let offsetting: Offsetting = op_code.get_bit(23).into();
            let byte_or_word: ReadWriteKind = op_code.into(); // TODO: is this the same for all instruction?
            let load_store: SingleDataTransferKind = op_code.into(); // TODO: is this the same bit for all instruction?
            let write_back = op_code.get_bit(21);
            let rn = op_code.get_bits(16..=19);
            let rd = op_code.get_bits(12..=15);

            let offset_info = match op_kind {
                OperandKind::Immediate => {
                    let offset = op_code.get_bits(0..=11);
                    SingleDataTransferOffsetInfo::Immediate { offset }
                }
                OperandKind::Register => {
                    let shift_amount = op_code.get_bits(7..=11);
                    let shift_kind: ShiftKind = op_code.get_bits(5..=6).into();
                    let reg_offset = op_code.get_bits(0..=3);
                    SingleDataTransferOffsetInfo::RegisterImmediate {
                        shift_amount,
                        shift_kind,
                        reg_offset,
                    }
                }
            };

            SingleDataTransfer {
                condition,
                kind: load_store,
                quantity: byte_or_word,
                write_back,
                indexing,
                rd,
                base_register: rn,
                offset_info,
                offsetting,
            }
        } else if op_code.get_bits(26..=27) == 0b00 {
            let alu_instruction = op_code.get_bits(21..=24).into();
            let set_conditions = op_code.get_bit(20);
            let rn = op_code.get_bits(16..=19);
            let op_kind: OperandKind = op_code.get_bit(25).into();
            let rd = op_code.get_bits(12..=15);

            if matches!(
                alu_instruction,
                ArmModeAluInstruction::Tst
                    | ArmModeAluInstruction::Teq
                    | ArmModeAluInstruction::Cmp
                    | ArmModeAluInstruction::Cmn
            ) && !set_conditions
            {
                // PSR instruction
                return PSRTransfer {
                    condition,
                    psr_kind: PsrKind::from(op_code.get_bit(22)),
                    kind: PsrOpKind::from(op_code),
                };
            }

            let op2 = match op_kind {
                OperandKind::Immediate => {
                    let shift = op_code.get_bits(8..=11) * 2;
                    let base = op_code.get_bits(0..=7);
                    AluSecondOperandInfo::Immediate { base, shift }
                }
                OperandKind::Register => {
                    let shift_kind: ShiftKind = op_code.get_bits(5..=6).into();
                    let shift_by_register_bit = op_code.get_bit(4);
                    let register = op_code.get_bits(0..=3);
                    let shift_op = if shift_by_register_bit {
                        if op_code.get_bit(7) {
                            todo!("should be zero or need different work")
                        }
                        ShiftOperator::Register(op_code.get_bits(8..=11))
                    } else {
                        ShiftOperator::Immediate(op_code.get_bits(7..=11))
                    };
                    AluSecondOperandInfo::Register {
                        shift_op,
                        shift_kind,
                        register,
                    }
                }
            };

            DataProcessing {
                condition,
                alu_instruction,
                set_conditions,
                op_kind,
                rn,
                destination: rd,
                op2,
            }
        } else {
            log("not identified instruction");
            unimplemented!()
        }
    }
}

impl std::fmt::Display for ArmModeInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn decode_branch() {
        let output = ArmModeInstruction::from(0b1110_1011_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::AL,
                link: true,
                offset: 508,
            },
            output
        );
        assert_eq!("BL 0x000001FC", output.disassembler());

        let output = ArmModeInstruction::from(0b1110_1010_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::AL,
                link: false,
                offset: 508,
            },
            output
        );
        assert_eq!("B 0x000001FC", output.disassembler());

        let output = ArmModeInstruction::from(0b0000_1010_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::EQ,
                link: false,
                offset: 508,
            },
            output
        );
        assert_eq!("BEQ 0x000001FC", output.disassembler());

        let output = ArmModeInstruction::from(0b0000_1011_0000_0000_0000_0000_0111_1111);
        assert_eq!(
            ArmModeInstruction::Branch {
                condition: Condition::EQ,
                link: true,
                offset: 508,
            },
            output
        );
        assert_eq!("BLEQ 0x000001FC", output.disassembler());
    }

    #[test]
    fn decode_branch_and_exchange() {
        let output = ArmModeInstruction::from(0b1110_0001_0010_1111_1111_1111_0001_0001);
        assert_eq!(
            ArmModeInstruction::BranchAndExchange {
                condition: Condition::AL,
                register: 1
            },
            output
        );
        assert_eq!("BX R1", output.disassembler());

        let output = ArmModeInstruction::from(0b0000_0001_0010_1111_1111_1111_0001_0001);
        assert_eq!(
            ArmModeInstruction::BranchAndExchange {
                condition: Condition::EQ,
                register: 1
            },
            output
        );
        assert_eq!("BXEQ R1", output.disassembler());
    }

    #[test]
    fn decode_data_processing() {
        let output = ArmModeInstruction::from(0b1110_00_0_1011_0_1001_1111_000000001110);
        assert_eq!(
            ArmModeInstruction::PSRTransfer {
                condition: Condition::AL,
                psr_kind: PsrKind::Spsr,
                kind: PsrOpKind::Msr {
                    source_register: 14
                }
            },
            output
        );
        assert_eq!("MSR SPSR, R14", output.disassembler());
    }

    #[test]
    fn decode_half_word_data_transfer_immediate_offset() {
        let output = ArmModeInstruction::from(0b1110_0001_1100_0001_0000_0000_1011_0000);
        assert_eq!(
            ArmModeInstruction::HalfwordDataTransfer {
                condition: Condition::AL,
                indexing: Indexing::Pre,
                offsetting: Offsetting::Up,
                write_back: false,
                load_store_kind: LoadStoreKind::Store,
                offset_kind: HalfwordDataTransferOffsetKind::Immediate { offset: 0 },
                base_register: 1,
                source_destination_register: 0,
                transfer_kind: HalfwordTransferKind::UnsignedHalfwords,
            },
            output
        );
    }

    #[test]
    fn decode_half_word_data_transfer_register_offset() {
        let output = ArmModeInstruction::from(0b1110_0001_1000_0010_0000_0000_1011_0001);
        assert_eq!(
            ArmModeInstruction::HalfwordDataTransfer {
                condition: Condition::AL,
                indexing: Indexing::Pre,
                offsetting: Offsetting::Up,
                write_back: false,
                load_store_kind: LoadStoreKind::Store,
                offset_kind: HalfwordDataTransferOffsetKind::Register { register: 1 },
                base_register: 2,
                source_destination_register: 0,
                transfer_kind: HalfwordTransferKind::UnsignedHalfwords,
            },
            output
        );
    }
}
