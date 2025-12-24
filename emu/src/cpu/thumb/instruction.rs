//! # Thumb Instruction Decoding
//!
//! This module handles decoding 16-bit Thumb instructions.
//!
//! ## Thumb Instruction Formats
//!
//! Thumb instructions are grouped into 19 formats, identified by their high bits:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    Thumb Instruction Formats                            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │  Format 1:  000 xx          Move shifted register                      │
//! │  Format 2:  00011           Add/subtract                               │
//! │  Format 3:  001 xx          Move/compare/add/subtract immediate        │
//! │  Format 4:  010000          ALU operations                             │
//! │  Format 5:  010001          Hi register operations / BX                │
//! │  Format 6:  01001           PC-relative load                           │
//! │  Format 7:  0101 xx0        Load/store with register offset            │
//! │  Format 8:  0101 xx1        Load/store sign-extended byte/halfword     │
//! │  Format 9:  011 xx          Load/store with immediate offset           │
//! │  Format 10: 1000 x          Load/store halfword                        │
//! │  Format 11: 1001 x          SP-relative load/store                     │
//! │  Format 12: 1010 x          Load address                               │
//! │  Format 13: 10110000        Add offset to stack pointer                │
//! │  Format 14: 1011 x10x       Push/pop registers                         │
//! │  Format 15: 1100 x          Multiple load/store                        │
//! │  Format 16: 1101 xxxx       Conditional branch                         │
//! │  Format 17: 11011111        Software interrupt                         │
//! │  Format 18: 11100           Unconditional branch                       │
//! │  Format 19: 1111 x          Long branch with link                      │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Register Restrictions
//!
//! Most Thumb instructions can only access R0-R7. To access R8-R15:
//! - Format 5 (Hi register ops): ADD, CMP, MOV with high registers
//! - BX: Can branch to any register
//! - PUSH/POP: Can include LR/PC via special bit
//!
//! ## Long Branch (BL)
//!
//! The BL instruction spans ±4MB but requires two 16-bit instructions:
//!
//! ```text
//! First:  1111 0xxx xxxx xxxx  ; LR = PC + (offset_hi << 12)
//! Second: 1111 1xxx xxxx xxxx  ; PC = LR + (offset_lo << 1), LR = old_PC | 1
//! ```

use crate::bitwise::Bits;
use crate::cpu::condition::Condition;
use crate::cpu::flags::{LoadStoreKind, OperandKind, Operation, ReadWriteKind, ShiftKind};
#[cfg(feature = "disassembler")]
use crate::cpu::registers::REG_PROGRAM_COUNTER;
use crate::cpu::thumb::alu_instructions::{ThumbHighRegisterOperation, ThumbModeAluInstruction};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum Instruction {
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
    LoadStoreImmOffset {
        load_store: LoadStoreKind,
        byte_word: ReadWriteKind,
        offset: u16,
        base_register: u16,
        destination_register: u16,
    },
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
    Nop, // For undefined/hint instructions that should be treated as NOP
}

impl TryFrom<u16> for Instruction {
    type Error = String;

    #[allow(clippy::too_many_lines)]
    fn try_from(op_code: u16) -> Result<Self, Self::Error> {
        use Instruction::{
            AddOffsetSP, AddSubtract, AluOp, CondBranch, HiRegisterOpBX, LoadAddress,
            LoadStoreHalfword, LoadStoreImmOffset, LoadStoreRegisterOffset,
            LoadStoreSignExtByteHalfword, LongBranchLink, MoveCompareAddSubtractImm,
            MoveShiftedRegister, MultipleLoadStore, Nop, PCRelativeLoad, PushPopReg,
            SPRelativeLoadStore, Swi, UncondBranch,
        };

        if op_code.get_bits(8..=15) == 0b1101_1111 {
            Ok(Swi)
        } else if op_code.get_bits(8..=15) == 0b1011_0000 {
            Ok(AddOffsetSP {
                // 0 - positive, 1 - negative TODO
                s: op_code.get_bit(7),
                // The offset supplied in #Imm is a full 10-bit address,
                // but must always be word-aligned (ie bits 1:0 set to 0),
                // since the assembler places #Imm >> 2 in the Word8 field.
                word7: op_code.get_bits(0..=6) << 2,
            })
        } else if op_code.get_bits(10..=15) == 0b01_0000 {
            Ok(AluOp {
                alu_operation: op_code.get_bits(6..=9).into(),
                source_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            })
        } else if op_code.get_bits(10..=15) == 0b01_0001 {
            let h1 = op_code.get_bit(7);
            let rd_hd = op_code.get_bits(0..=2);
            let destination_register = if h1 { rd_hd | (1 << 3) } else { rd_hd };
            let source_register = op_code.get_bits(3..=6);

            Ok(HiRegisterOpBX {
                register_operation: op_code.get_bits(8..=9).into(),
                source_register,
                destination_register,
            })
        } else if op_code.get_bits(12..=15) == 0b1011 && op_code.get_bits(9..=10) == 0b10 {
            Ok(PushPopReg {
                load_store: op_code.get_bit(11).into(),
                pc_lr: op_code.get_bit(8),
                register_list: op_code.get_bits(0..=7),
            })
        } else if op_code.get_bits(11..=15) == 0b00011 {
            Ok(AddSubtract {
                operation_kind: op_code.get_bit(10).into(),
                // 0 - Add, 1 - Sub TODO
                op: op_code.get_bit(9),
                rn_offset3: op_code.get_bits(6..=8),
                source_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            })
        } else if op_code.get_bits(11..=15) == 0b01001 {
            Ok(PCRelativeLoad {
                destination_register: op_code.get_bits(8..=10),
                immediate_value: op_code.get_bits(0..=7) << 2,
            })
        } else if op_code.get_bits(12..=15) == 0b0101 && !op_code.get_bit(9) {
            Ok(LoadStoreRegisterOffset {
                load_store: op_code.get_bit(11).into(),
                byte_word: op_code.get_bit(10).into(),
                ro: op_code.get_bits(6..=8),
                base_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            })
        } else if op_code.get_bits(12..=15) == 0b0101 && op_code.get_bit(9) {
            Ok(LoadStoreSignExtByteHalfword {
                h: op_code.get_bit(11),
                sign_extend_flag: op_code.get_bit(10),
                offset_register: op_code.get_bits(6..=8) as u32,
                base_register: op_code.get_bits(3..=5) as u32,
                destination_register: op_code.get_bits(0..=2) as u32,
            })
        } else if op_code.get_bits(11..=15) == 0b11100 {
            Ok(UncondBranch {
                offset: (op_code.get_bits(0..=10) << 1) as u32,
            })
        } else if op_code.get_bits(12..=15) == 0b1000 {
            Ok(LoadStoreHalfword {
                load_store: op_code.get_bit(11).into(),
                offset: op_code.get_bits(6..=10) << 1,
                base_register: op_code.get_bits(3..=5),
                source_destination_register: op_code.get_bits(0..=2),
            })
        } else if op_code.get_bits(12..=15) == 0b1001 {
            Ok(SPRelativeLoadStore {
                load_store: op_code.get_bit(11).into(),
                destination_register: op_code.get_bits(8..=10),
                // The offset supplied in #Imm is a full 10-bit address,
                // but must always be word-aligned (ie bits 1:0 set to 0),
                // since the assembler places #Imm >> 2 in the Word8 field.
                word8: op_code.get_bits(0..=7) << 2,
            })
        } else if op_code.get_bits(12..=15) == 0b1010 {
            Ok(LoadAddress {
                sp: op_code.get_bit(11),
                destination_register: op_code.get_bits(8..=10) as u32,
                offset: (op_code.get_bits(0..=7) as u32) << 2,
            })
        } else if op_code.get_bits(12..=15) == 0b1100 {
            Ok(MultipleLoadStore {
                load_store: op_code.get_bit(11).into(),
                base_register: op_code.get_bits(8..=10),
                register_list: op_code.get_bits(0..=7),
            })
        } else if op_code.get_bits(12..=15) == 0b1101 {
            // 9 bits signed offset (assembler puts `label` >> 1 in this field so we should <<1)
            let offset = (op_code.get_bits(0..=7) << 1) as u32;
            let immediate_offset = offset.sign_extended(9) as i32;

            Ok(CondBranch {
                condition: Condition::from(op_code.get_bits(8..=11) as u8),
                immediate_offset,
            })
        } else if op_code.get_bits(12..=15) == 0b1111 || op_code.get_bits(12..=15) == 0b1110 {
            // Format 19: Long branch with link (BL/BLX)
            // 1111 0xxx xxxx xxxx - H=0: Setup high offset in LR
            // 1111 1xxx xxxx xxxx - H=1: Add low offset and branch to LR
            // 1110 1xxx xxxx xxxx - BLX variant (ARMv5+, exchanges to ARM mode)
            Ok(LongBranchLink {
                h: op_code.get_bit(11),
                offset: op_code.get_bits(0..=10) as u32,
            })
        } else if op_code.get_bits(13..=15) == 0b000 {
            Ok(MoveShiftedRegister {
                shift_operation: op_code.get_bits(11..=12).into(),
                offset5: op_code.get_bits(6..=10),
                source_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            })
        } else if op_code.get_bits(13..=15) == 0b001 {
            Ok(MoveCompareAddSubtractImm {
                operation: op_code.get_bits(11..=12).into(),
                destination_register: op_code.get_bits(8..=10),
                offset: op_code.get_bits(0..=7).into(),
            })
        } else if op_code.get_bits(13..=15) == 0b011 {
            let byte_word: ReadWriteKind = op_code.get_bit(12).into();
            let offset = match byte_word {
                // For word transfers, offset is word-aligned (assembler puts offset >> 2)
                ReadWriteKind::Word => op_code.get_bits(6..=10) << 2,
                // For byte transfers, offset is in bytes
                ReadWriteKind::Byte => op_code.get_bits(6..=10),
            };

            Ok(LoadStoreImmOffset {
                load_store: op_code.get_bit(11).into(),
                byte_word,
                offset,
                base_register: op_code.get_bits(3..=5),
                destination_register: op_code.get_bits(0..=2),
            })
        } else if op_code.get_bits(12..=15) == 0b1011 {
            // Other 0xBxxx instructions that don't match specific patterns
            tracing::debug!("Treating undefined/hint instruction 0x{op_code:04X} as NOP",);
            Ok(Nop)
        } else {
            Err(format!(
                "Unimplemented Thumb instruction: 0x{:04X} (bits 15-13: 0b{:03b}, bits 12-11: 0b{:02b})",
                op_code,
                op_code.get_bits(13..=15),
                op_code.get_bits(11..=12)
            ))
        }
    }
}

impl std::fmt::Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(feature = "disassembler")]
impl Instruction {
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
                    "PC".to_owned()
                } else {
                    format!("R{r_destination}")
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
            Self::LoadStoreImmOffset {
                load_store,
                byte_word,
                offset,
                base_register: rb,
                destination_register: rd,
            } => {
                let instr = match (load_store, byte_word) {
                    (LoadStoreKind::Load, ReadWriteKind::Byte) => "LDRB",
                    (LoadStoreKind::Load, ReadWriteKind::Word) => "LDR",
                    (LoadStoreKind::Store, ReadWriteKind::Byte) => "STRB",
                    (LoadStoreKind::Store, ReadWriteKind::Word) => "STR",
                };
                format!("{instr} R{rd}, [R{rb}, #{offset}]")
            }
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

                let mut regs = Vec::new();
                for i in 0..=7 {
                    if register_list.get_bit(i) {
                        regs.push(format!("R{i}"));
                    }
                }
                if *pc_lr {
                    regs.push("PC".to_string());
                } else {
                    regs.push("LR".to_string());
                }
                let registers = regs.join("-");
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

                let mut regs = Vec::new();
                for i in 0..=7 {
                    if register_list.get_bit(i) {
                        regs.push(format!("R{i}"));
                    }
                }
                let registers = regs.join("-");
                format!("{instr} R{base_register}!, {{{registers}}}")
            }
            Self::CondBranch {
                condition,
                immediate_offset,
            } => {
                format!("B{condition} #{immediate_offset}")
            }
            Self::Swi => panic!("not implemented"),
            Self::UncondBranch { offset } => {
                format!("B #{offset}")
            }
            Self::LongBranchLink { h, offset } => {
                let offset = offset << 1;
                let h = if *h { "H" } else { "" };
                format!("BL{h} #{offset}")
            }
            Self::Nop => "NOP".to_owned(),
        }
    }
}

#[cfg(feature = "disassembler")]
#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn decode_multiple_load_store() {
        let output = Instruction::try_from(0b1100_1001_1010_0000).unwrap();
        assert_eq!(
            Instruction::MultipleLoadStore {
                load_store: LoadStoreKind::Load,
                base_register: 1,
                register_list: 160,
            },
            output
        );
        assert_eq!("LDMIA R1!, {R5-R7}", output.disassembler());
    }

    #[test]
    fn decode_pc_relative_load() {
        let output = Instruction::try_from(0b0100_1001_0101_1000).unwrap();
        assert_eq!(
            Instruction::PCRelativeLoad {
                destination_register: 1,
                immediate_value: 352,
            },
            output
        );
        assert_eq!("LDR R1, [R1, #1408]", output.disassembler());
    }

    #[test]
    fn decode_load_store_register_offset() {
        let output = Instruction::try_from(0b0101_00_0_000_001_010).unwrap();
        assert_eq!(
            Instruction::LoadStoreRegisterOffset {
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
        let output = Instruction::try_from(0b1110_0001_0010_1111).unwrap();
        assert_eq!(Instruction::UncondBranch { offset: 606 }, output);
        assert_eq!("B #606", output.disassembler()); // FIXME: Should this be decimal or hex?
    }

    #[test]
    fn decode_hi_reg_operation() {
        let output = Instruction::try_from(0b0100_0111_0111_0000).unwrap();
        assert_eq!(
            Instruction::HiRegisterOpBX {
                register_operation: ThumbHighRegisterOperation::BxOrBlx,
                source_register: 14,
                destination_register: 0,
            },
            output
        );
        assert_eq!("BX R0, R14", output.disassembler());

        let output = Instruction::try_from(0b010001_00_0_1_000_001).unwrap();
        assert_eq!(
            Instruction::HiRegisterOpBX {
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
        let output = Instruction::try_from(0b1011_0101_1111_0000).unwrap();
        assert_eq!(
            Instruction::PushPopReg {
                load_store: LoadStoreKind::Store,
                pc_lr: true,
                register_list: 240,
            },
            output
        );

        assert_eq!("PUSH {R4-R5-R6-R7-PC}", output.disassembler());
    }

    #[test]
    fn decode_alu_operation() {
        let output = Instruction::try_from(0b0100_0011_0110_0000).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Mul,
                source_register: 4,
                destination_register: 0,
            },
            output
        );
        assert_eq!("MUL R0, R4", output.disassembler());

        let output = Instruction::try_from(0b0100_0000_0001_1000).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::And,
                source_register: 3,
                destination_register: 0,
            },
            output
        );
        assert_eq!("AND R0, R3", output.disassembler());

        let output = Instruction::try_from(0b0100_0010_0011_1110).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Tst,
                source_register: 7,
                destination_register: 6,
            },
            output
        );
        assert_eq!("TST R6, R7", output.disassembler());

        let output = Instruction::try_from(0b0100_0011_0010_1010).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Orr,
                source_register: 5,
                destination_register: 2,
            },
            output
        );
        assert_eq!("ORR R2, R5", output.disassembler());

        let output = Instruction::try_from(0b0100_0011_1100_1111).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Mvn,
                source_register: 1,
                destination_register: 7,
            },
            output
        );
        assert_eq!("MVN R7, R1", output.disassembler());

        let output = Instruction::try_from(0b0100_0001_1110_0011).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Ror,
                source_register: 4,
                destination_register: 3,
            },
            output
        );
        assert_eq!("ROR R3, R4", output.disassembler());

        let output = Instruction::try_from(0b0100_0000_0101_0011).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Eor,
                source_register: 2,
                destination_register: 3,
            },
            output
        );
        assert_eq!("EOR R3, R2", output.disassembler());

        let output = Instruction::try_from(0b0100_0010_0100_0000).unwrap();
        assert_eq!(
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Neg,
                source_register: 0,
                destination_register: 0,
            },
            output
        );
        assert_eq!("NEG R0, R0", output.disassembler());

        let output = Instruction::try_from(0b0100_0000_1000_1000).unwrap();
        assert_eq!(
            output,
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Lsl,
                source_register: 1,
                destination_register: 0,
            },
        );
        assert_eq!("LSL R0, R1", output.disassembler());

        let output = Instruction::try_from(0b0100_0001_0000_1000).unwrap();
        assert_eq!(
            output,
            Instruction::AluOp {
                alu_operation: ThumbModeAluInstruction::Asr,
                source_register: 1,
                destination_register: 0,
            },
        );
        assert_eq!("ASR R0, R1", output.disassembler());
    }

    #[test]
    fn decode_load_store_half_word() {
        let output = Instruction::try_from(0b1000_1_00001_000_001).unwrap();
        assert_eq!(
            Instruction::LoadStoreHalfword {
                load_store: LoadStoreKind::Load,
                offset: 2,
                base_register: 0,
                source_destination_register: 1,
            },
            output
        );
        assert_eq!("LDRH R1, [R0, #2]", output.disassembler());

        let output = Instruction::try_from(0b1000_0_00001_000_001).unwrap();
        assert_eq!(
            Instruction::LoadStoreHalfword {
                load_store: LoadStoreKind::Store,
                offset: 2,
                base_register: 0,
                source_destination_register: 1,
            },
            output
        );
        assert_eq!("STRH R1, [R0, #2]", output.disassembler());
    }

    #[test]
    fn decode_load_store_imm_offset() {
        // LDR R2, [R1, #8] - word load with immediate offset
        // Format: 011 B L offset5 Rb Rd
        // B=0 (word), L=1 (load), offset=2 (<<2 = 8), Rb=1, Rd=2
        let output = Instruction::try_from(0b0110_1_00010_001_010).unwrap();
        assert_eq!(
            Instruction::LoadStoreImmOffset {
                load_store: LoadStoreKind::Load,
                byte_word: ReadWriteKind::Word,
                offset: 8, // offset is stored as word offset, shifted << 2
                base_register: 1,
                destination_register: 2,
            },
            output
        );
        assert_eq!("LDR R2, [R1, #8]", output.disassembler());

        // STR R3, [R4, #16] - word store with immediate offset
        // B=0 (word), L=0 (store), offset=4 (<<2 = 16), Rb=4, Rd=3
        let output = Instruction::try_from(0b0110_0_00100_100_011).unwrap();
        assert_eq!(
            Instruction::LoadStoreImmOffset {
                load_store: LoadStoreKind::Store,
                byte_word: ReadWriteKind::Word,
                offset: 16,
                base_register: 4,
                destination_register: 3,
            },
            output
        );
        assert_eq!("STR R3, [R4, #16]", output.disassembler());

        // LDRB R5, [R6, #7] - byte load with immediate offset
        // B=1 (byte), L=1 (load), offset=7, Rb=6, Rd=5
        let output = Instruction::try_from(0b0111_1_00111_110_101).unwrap();
        assert_eq!(
            Instruction::LoadStoreImmOffset {
                load_store: LoadStoreKind::Load,
                byte_word: ReadWriteKind::Byte,
                offset: 7, // byte offset is not shifted
                base_register: 6,
                destination_register: 5,
            },
            output
        );
        assert_eq!("LDRB R5, [R6, #7]", output.disassembler());

        // STRB R0, [R1, #0] - byte store with zero offset
        // B=1 (byte), L=0 (store), offset=0, Rb=1, Rd=0
        let output = Instruction::try_from(0b0111_0_00000_001_000).unwrap();
        assert_eq!(
            Instruction::LoadStoreImmOffset {
                load_store: LoadStoreKind::Store,
                byte_word: ReadWriteKind::Byte,
                offset: 0,
                base_register: 1,
                destination_register: 0,
            },
            output
        );
        assert_eq!("STRB R0, [R1, #0]", output.disassembler());
    }

    #[test]
    fn decode_long_branch_link() {
        // BL instruction consists of two parts:
        // First: 1111 0xxx xxxx xxxx (H=0, sets up high offset in LR)
        // Second: 1111 1xxx xxxx xxxx (H=1, completes the call)

        // First half: setup high offset
        let output = Instruction::try_from(0b1111_0_000_0000_0001).unwrap();
        assert_eq!(
            Instruction::LongBranchLink {
                h: false,
                offset: 1,
            },
            output
        );

        // Second half: complete branch
        let output = Instruction::try_from(0b1111_1_000_0000_0010).unwrap();
        assert_eq!(Instruction::LongBranchLink { h: true, offset: 2 }, output);
    }
}
