use crate::bitwise::Bits;
use crate::cpu::condition::Condition;
use crate::cpu::flags::{LoadStoreKind, OperandKind, Operation, ReadWriteKind, ShiftKind};
use crate::cpu::registers::REG_PROGRAM_COUNTER;
use crate::cpu::thumb::alu_instructions::{ThumbHighRegisterOperation, ThumbModeAluInstruction};
use logger::log;

#[derive(Debug, PartialEq, Eq)]
pub enum ThumbModeInstruction {
    MoveShiftedRegister {
        shift_operation: ShiftKind,
        offset5: u16,
        source_register: u16,
        destination_register: u16,
    },
    AddSubtract {
        operation_kind: OperandKind,
        op: bool,
        rn_offset3: u16,
        source_register: u16,
        destination_register: u16,
    },
    MoveCompareAddSubtractImm {
        operation: Operation,
        destination_register: u16,
        offset: u32,
    },
    AluOp {
        alu_operation: ThumbModeAluInstruction,
        source_register: u16,
        destination_register: u16,
    },
    HiRegisterOpBX {
        register_operation: ThumbHighRegisterOperation,
        source_register: u16,
        destination_register: u16,
    },
    PCRelativeLoad {
        destination_register: u16,
        immediate_value: u16,
    },
    LoadStoreRegisterOffset {
        load_store: LoadStoreKind,
        byte_word: ReadWriteKind,
        ro: u16,
        base_register: u16,
        destination_register: u16,
    },
    LoadStoreSignExtByteHalfword {
        h: bool,
        sign_extend_flag: bool,
        offset_register: u32,
        base_register: u32,
        destination_register: u32,
    },
    LoadStoreImmOffset,
    LoadStoreHalfword {
        load_store: LoadStoreKind,
        offset: u16,
        base_register: u16,
        source_destination_register: u16,
    },
    SPRelativeLoadStore {
        load_store: LoadStoreKind,
        destination_register: u16,
        word8: u16,
    },
    LoadAddress {
        sp: bool,
        destination_register: u32,
        offset: u32,
    },
    AddOffsetSP {
        s: bool,
        word7: u16,
    },
    PushPopReg {
        load_store: LoadStoreKind,
        pc_lr: bool,
        register_list: u16,
    },
    MultipleLoadStore {
        load_store: LoadStoreKind,
        base_register: u16,
        register_list: u16,
    },
    CondBranch {
        condition: Condition,
        immediate_offset: i32,
    },
    Swi,
    UncondBranch {
        offset: u32,
    },
    LongBranchLink {
        h: bool,
        offset: u32,
    },
}

impl From<u16> for ThumbModeInstruction {
    fn from(op_code: u16) -> Self {
        use ThumbModeInstruction::*;

        if op_code.get_bits(8..=15) == 0b11011111 {
            Swi
        } else if op_code.get_bits(8..=15) == 0b10110000 {
            AddOffsetSP {
                // 0 - positive, 1 - negative TODO
                s: op_code.get_bit(7),
                // The offset supplied in #Imm is a full 10-bit address,
                // but must always be word-aligned (ie bits 1:0 set to 0),
                // since the assembler places #Imm >> 2 in the Word8 field.
                word7: op_code.get_bits(0..=6) << 2,
            }
        } else if op_code.get_bits(10..=15) == 0b010000 {
            AluOp {
                alu_operation: op_code.get_bits(6..=9).into(),
                source_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            }
        } else if op_code.get_bits(10..=15) == 0b010001 {
            let h1 = op_code.get_bit(7);
            let rd_hd = op_code.get_bits(0..=2);
            let destination_register = if h1 { rd_hd | (1 << 3) } else { rd_hd };

            HiRegisterOpBX {
                register_operation: op_code.get_bits(8..=9).into(),
                source_register: op_code.get_bits(3..=6),
                destination_register,
            }
        } else if op_code.get_bits(12..=15) == 0b1011 && op_code.get_bits(9..=10) == 0b10 {
            PushPopReg {
                load_store: op_code.get_bit(11).into(),
                pc_lr: op_code.get_bit(8),
                register_list: op_code.get_bits(0..=7),
            }
        } else if op_code.get_bits(11..=15) == 0b00011 {
            AddSubtract {
                operation_kind: op_code.get_bit(10).into(),
                // 0 - Add, 1 - Sub TODO
                op: op_code.get_bit(9),
                rn_offset3: op_code.get_bits(6..=8),
                source_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            }
        } else if op_code.get_bits(11..=15) == 0b01001 {
            PCRelativeLoad {
                destination_register: op_code.get_bits(8..=10),
                immediate_value: op_code.get_bits(0..=7) << 2,
            }
        } else if op_code.get_bits(12..=15) == 0b0101 && !op_code.get_bit(9) {
            LoadStoreRegisterOffset {
                load_store: op_code.get_bit(11).into(),
                byte_word: op_code.get_bit(10).into(),
                ro: op_code.get_bits(6..=8),
                base_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            }
        } else if op_code.get_bits(12..=15) == 0b0101 && op_code.get_bit(9) {
            LoadStoreSignExtByteHalfword {
                h: op_code.get_bit(11),
                sign_extend_flag: op_code.get_bit(10),
                offset_register: op_code.get_bits(6..=8) as u32,
                base_register: op_code.get_bits(3..=5) as u32,
                destination_register: op_code.get_bits(0..=2) as u32,
            }
        } else if op_code.get_bits(11..=15) == 0b11100 {
            UncondBranch {
                offset: (op_code.get_bits(0..=10) << 1) as u32,
            }
        } else if op_code.get_bits(12..=15) == 0b1000 {
            LoadStoreHalfword {
                load_store: op_code.get_bit(11).into(),
                offset: op_code.get_bits(6..=10) << 1,
                base_register: op_code.get_bits(3..=5),
                source_destination_register: op_code.get_bits(0..=2),
            }
        } else if op_code.get_bits(12..=15) == 0b1001 {
            SPRelativeLoadStore {
                load_store: op_code.get_bit(11).into(),
                destination_register: op_code.get_bits(8..=10),
                // The offset supplied in #Imm is a full 10-bit address,
                // but must always be word-aligned (ie bits 1:0 set to 0),
                // since the assembler places #Imm >> 2 in the Word8 field.
                word8: op_code.get_bits(0..=7) << 2,
            }
        } else if op_code.get_bits(12..=15) == 0b1010 {
            LoadAddress {
                sp: op_code.get_bit(11),
                destination_register: op_code.get_bits(8..=10) as u32,
                offset: (op_code.get_bits(0..=7) as u32) << 2,
            }
        } else if op_code.get_bits(12..=15) == 0b1100 {
            MultipleLoadStore {
                load_store: op_code.get_bit(11).into(),
                base_register: op_code.get_bits(8..=10),
                register_list: op_code.get_bits(0..=7),
            }
        } else if op_code.get_bits(12..=15) == 0b1101 {
            // 9 bits signed offset (assembler puts `label` >> 1 in this field so we should <<1)
            let offset = (op_code.get_bits(0..=7) << 1) as u32;
            let immediate_offset = offset.sign_extended(9) as i32;

            CondBranch {
                condition: Condition::from(op_code.get_bits(8..=11) as u8),
                immediate_offset,
            }
        } else if op_code.get_bits(12..=15) == 0b1111 {
            LongBranchLink {
                h: op_code.get_bit(11),
                offset: op_code.get_bits(0..=10) as u32,
            }
        } else if op_code.get_bits(13..=15) == 0b000 {
            MoveShiftedRegister {
                shift_operation: op_code.get_bits(11..=12).into(),
                offset5: op_code.get_bits(6..=10),
                source_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            }
        } else if op_code.get_bits(13..=15) == 0b001 {
            MoveCompareAddSubtractImm {
                operation: op_code.get_bits(11..=12).into(),
                destination_register: op_code.get_bits(8..=10),
                offset: op_code.get_bits(0..=7).into(),
            }
        } else if op_code.get_bits(13..=15) == 0b011 {
            LoadStoreImmOffset
        } else {
            log(format!("not identified instruction {op_code} "));
            unimplemented!()
        }
    }
}

impl std::fmt::Display for ThumbModeInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl ThumbModeInstruction {
    pub(crate) fn disassembler(&self) -> String {
        match self {
            Self::MoveShiftedRegister {
                shift_operation: op,
                offset5,
                source_register,
                destination_register,
            } => {
                format!("{op} R{destination_register}, R{source_register}, #{offset5}")
            }
            Self::AddSubtract {
                operation_kind,
                op,
                rn_offset3,
                source_register: rs,
                destination_register: rd,
            } => {
                let o = match *op {
                    true => "SUB",
                    false => "ADD",
                };

                let rr = match operation_kind {
                    OperandKind::Immediate => format!("#{rn_offset3}"),
                    OperandKind::Register => format!("R{rn_offset3}"),
                };

                format!("{o} R{rd}, R{rs}, {rr}")
            }
            Self::MoveCompareAddSubtractImm {
                operation: op,
                destination_register: r_destination,
                offset,
            } => {
                format!("{op} R{r_destination}, #{offset}")
            }
            Self::AluOp {
                alu_operation: op,
                source_register: rs,
                destination_register: rd,
            } => {
                let op = match *op {
                    ThumbModeAluInstruction::And => "AND",
                    ThumbModeAluInstruction::Eor => "EOR",
                    ThumbModeAluInstruction::Lsl => "LSL",
                    ThumbModeAluInstruction::Lsr => "LSR",
                    ThumbModeAluInstruction::Asr => "ASR",
                    ThumbModeAluInstruction::Adc => "ADC",
                    ThumbModeAluInstruction::Sbc => "SBC",
                    ThumbModeAluInstruction::Ror => "ROR",
                    ThumbModeAluInstruction::Tst => "TST",
                    ThumbModeAluInstruction::Neg => "NEG",
                    ThumbModeAluInstruction::Cmp => "CMP",
                    ThumbModeAluInstruction::Cmn => "CMN",
                    ThumbModeAluInstruction::Orr => "ORR",
                    ThumbModeAluInstruction::Mul => "MUL",
                    ThumbModeAluInstruction::Bic => "BIC",
                    ThumbModeAluInstruction::Mvn => "MVN",
                };

                format!("{op} R{rd}, R{rs}")
            }
            Self::HiRegisterOpBX {
                register_operation: op,
                source_register,
                destination_register,
            } => {
                format!("{op} R{destination_register}, R{source_register}")
            }
            Self::PCRelativeLoad {
                destination_register: r_destination,
                immediate_value,
            } => {
                let r = if *r_destination as u32 == REG_PROGRAM_COUNTER {
                    "PC"
                } else {
                    "R"
                };
                let immediate_value = immediate_value << 2;
                format!("LDR R{r_destination}, [{r}, #{immediate_value}]")
            }
            Self::LoadStoreRegisterOffset {
                load_store,
                byte_word,
                ro,
                base_register: rb,
                destination_register: rd,
            } => {
                let instr = match (load_store, byte_word) {
                    (LoadStoreKind::Load, ReadWriteKind::Byte) => "LDRB",
                    (LoadStoreKind::Load, ReadWriteKind::Word) => "LDR",
                    (LoadStoreKind::Store, ReadWriteKind::Byte) => "STRB",
                    (LoadStoreKind::Store, ReadWriteKind::Word) => "STR",
                };
                format!("{instr} R{rd}, [R{rb}, R{ro}]")
            }
            Self::LoadStoreSignExtByteHalfword {
                h: h_flag,
                sign_extend_flag,
                offset_register: r_offset,
                base_register: r_base,
                destination_register: r_destination,
            } => {
                let instr = match (sign_extend_flag, h_flag) {
                    (false, false) => "STRH",
                    (false, true) => "LDRH",
                    (true, false) => "LDSB",
                    (true, true) => "LDSH",
                };

                format!("{instr} R{r_destination}, [R{r_base}, R{r_offset}]")
            }
            Self::LoadStoreImmOffset => "".to_string(),
            Self::LoadStoreHalfword {
                load_store,
                offset,
                base_register,
                source_destination_register,
            } => {
                let instr = match load_store {
                    LoadStoreKind::Load => "LDRH",
                    LoadStoreKind::Store => "STRH",
                };

                format!("{instr} R{source_destination_register}, [R{base_register}, #{offset}]")
            }
            Self::SPRelativeLoadStore {
                load_store,
                destination_register: r_destination,
                word8,
            } => {
                let instr = match load_store {
                    LoadStoreKind::Load => "LDR",
                    LoadStoreKind::Store => "STR",
                };

                format!("{instr} R{r_destination}, [SP, #{word8}]")
            }
            Self::LoadAddress {
                sp,
                destination_register: r_destination,
                offset,
            } => {
                let source = match sp {
                    false => "PC",
                    true => "SP",
                };

                format!("ADD R{r_destination}, {source}, #{offset}")
            }
            Self::AddOffsetSP { s, word7 } => {
                let op = match s {
                    false => "ADD",
                    true => "SUB",
                };
                format!("{op} SP, #{word7}")
            }
            Self::PushPopReg {
                load_store,
                pc_lr,
                register_list,
            } => {
                let instr = match load_store {
                    LoadStoreKind::Load => "POP",
                    LoadStoreKind::Store => "PUSH",
                };

                let mut registers = String::new();
                for i in 0..=7 {
                    if register_list.get_bit(i) {
                        registers.push_str(&format!("R{i}, "));
                    }
                }

                if *pc_lr {
                    registers.push_str("PC");
                } else {
                    registers.push_str("LR");
                }

                format!("{instr} {{{registers}}}")
            }
            Self::MultipleLoadStore {
                load_store,
                base_register,
                register_list,
            } => {
                let instr = match load_store {
                    LoadStoreKind::Load => "LDMIA",
                    LoadStoreKind::Store => "STMIA",
                };

                let mut registers = String::new();
                for i in 0..=7 {
                    if register_list.get_bit(i) {
                        registers.push_str(&format!("R{i}, "));
                    }
                }

                format!("{instr} R{base_register}!, {{{registers}}}")
            }
            Self::CondBranch {
                condition,
                immediate_offset,
            } => {
                format!("B{condition} #{immediate_offset}")
            }
            Self::Swi => "".to_string(),
            Self::UncondBranch { offset } => {
                format!("B #{offset}")
            }
            Self::LongBranchLink { h, offset } => {
                let offset = offset << 1;
                let h = if *h { "H" } else { "" };
                format!("BL{h} #{offset}")
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn decode_multiple_load_store() {
        let output = ThumbModeInstruction::from(0b1100_1001_1010_0000);
        assert_eq!(
            ThumbModeInstruction::MultipleLoadStore {
                load_store: LoadStoreKind::Load,
                base_register: 1,
                register_list: 160,
            },
            output
        );
        assert_eq!("LDMIA R1!, {R5, R7, }", output.disassembler());
    }

    #[test]
    fn decode_pc_relative_load() {
        let output = ThumbModeInstruction::from(0b0100_1001_0101_1000);
        assert_eq!(
            ThumbModeInstruction::PCRelativeLoad {
                destination_register: 1,
                immediate_value: 352,
            },
            output
        );
        assert_eq!("LDR R1, [R, #1408]", output.disassembler());
    }

    #[test]
    fn decode_load_store_register_offset() {
        let output = ThumbModeInstruction::from(0b0101_00_0_000_001_010);
        assert_eq!(
            ThumbModeInstruction::LoadStoreRegisterOffset {
                load_store: LoadStoreKind::Store,
                byte_word: Default::default(),
                ro: 0,
                base_register: 1,
                destination_register: 2,
            },
            output
        );
        assert_eq!("STR R2, [R1, R0]", output.disassembler());
    }

    #[test]
    fn decode_uncond_branch() {
        let output = ThumbModeInstruction::from(0b1110_0001_0010_1111);
        assert_eq!(ThumbModeInstruction::UncondBranch { offset: 606 }, output);
        assert_eq!("B #606", output.disassembler()); // FIXME: Should this be decimal or hex?
    }

    #[test]
    fn decode_hi_reg_operation() {
        let output = ThumbModeInstruction::from(0b0100_0111_0111_0000);
        assert_eq!(
            ThumbModeInstruction::HiRegisterOpBX {
                register_operation: ThumbHighRegisterOperation::BxOrBlx,
                source_register: 14,
                destination_register: 0,
            },
            output
        );
        assert_eq!("BX R0, R14", output.disassembler());

        let output = ThumbModeInstruction::from(0b010001_00_0_1_000_001);
        assert_eq!(
            ThumbModeInstruction::HiRegisterOpBX {
                register_operation: ThumbHighRegisterOperation::Add,
                source_register: 8,
                destination_register: 1,
            },
            output
        );
        assert_eq!("ADD R1, R8", output.disassembler());
    }

    #[test]
    fn decode_push_pop_register() {
        let output = ThumbModeInstruction::from(0b1011_0101_1111_0000);
        assert_eq!(
            ThumbModeInstruction::PushPopReg {
                load_store: LoadStoreKind::Store,
                pc_lr: true,
                register_list: 240,
            },
            output
        );

        assert_eq!("PUSH {R4, R5, R6, R7, PC}", output.disassembler());
    }

    #[test]
    fn decode_alu_operation() {
        let output = ThumbModeInstruction::from(0b0100_0011_0110_0000);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Mul,
                source_register: 4,
                destination_register: 0,
            },
            output
        );
        assert_eq!("MUL R0, R4", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0000_0001_1000);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::And,
                source_register: 3,
                destination_register: 0,
            },
            output
        );
        assert_eq!("AND R0, R3", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0010_0011_1110);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Tst,
                source_register: 7,
                destination_register: 6,
            },
            output
        );
        assert_eq!("TST R6, R7", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0011_0010_1010);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Orr,
                source_register: 5,
                destination_register: 2,
            },
            output
        );
        assert_eq!("ORR R2, R5", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0011_1100_1111);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Mvn,
                source_register: 1,
                destination_register: 7,
            },
            output
        );
        assert_eq!("MVN R7, R1", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0001_1110_0011);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Ror,
                source_register: 4,
                destination_register: 3,
            },
            output
        );
        assert_eq!("ROR R3, R4", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0000_0101_0011);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Eor,
                source_register: 2,
                destination_register: 3,
            },
            output
        );
        assert_eq!("EOR R3, R2", output.disassembler());

        let output = ThumbModeInstruction::from(0b0100_0010_0100_0000);
        assert_eq!(
            ThumbModeInstruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Neg,
                source_register: 0,
                destination_register: 0,
            },
            output
        );
        assert_eq!("NEG R0, R0", output.disassembler());
    }

    #[test]
    fn decode_load_store_half_word() {
        let output = ThumbModeInstruction::from(0b1000_1_00001_000_001);
        assert_eq!(
            ThumbModeInstruction::LoadStoreHalfword {
                load_store: LoadStoreKind::Load,
                offset: 2,
                base_register: 0,
                source_destination_register: 1,
            },
            output
        );
        assert_eq!("LDRH R1, [R0, #2]", output.disassembler());

        let output = ThumbModeInstruction::from(0b1000_0_00001_000_001);
        assert_eq!(
            ThumbModeInstruction::LoadStoreHalfword {
                load_store: LoadStoreKind::Store,
                offset: 2,
                base_register: 0,
                source_destination_register: 1,
            },
            output
        );
        assert_eq!("STRH R1, [R0, #2]", output.disassembler());
    }
}
