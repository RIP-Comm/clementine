use std::fmt::{Display, Formatter};

use logger::log;

use crate::bitwise::Bits;
use crate::cpu::alu_instruction::{ArmModeAluInstruction, ShiftKind, ThumbModeAluInstruction};
use crate::cpu::arm7tdmi::REG_PROGRAM_COUNTER;
use crate::cpu::data_processing::{AluSecondOperandInfo, ShiftOperator};
use crate::cpu::flags::{Indexing, LoadStoreKind, Offsetting, OperandKind, ReadWriteKind};
use crate::cpu::move_compare_add_sub::{Operation, ThumbHighRegisterOperation};
use crate::cpu::single_data_transfer::{SingleDataTransferKind, SingleDataTransferOffsetInfo};

use super::condition::Condition;

#[derive(Debug, PartialEq, Eq)]
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
    Multiply,
    MultiplyLong,
    SingleDataSwap,
    BranchAndExchange {
        condition: Condition,
        register: usize,
    },
    HalfwordDataTransferRegisterOffset,
    HalfwordDataTransferImmediateOffset,
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
    CoprocessorRegisterTrasfer,
    SoftwareInterrupt,
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
            }
            Self::Multiply => "".to_owned(),
            Self::MultiplyLong => "".to_owned(),
            Self::SingleDataSwap => "".to_owned(),
            Self::BranchAndExchange {
                condition,
                register,
            } => format!("BX{condition} R{register}"),
            Self::HalfwordDataTransferRegisterOffset => "".to_owned(),
            Self::HalfwordDataTransferImmediateOffset => "".to_owned(),
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
            Self::Undefined => "".to_owned(),
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
            Self::CoprocessorDataOperation => "".to_owned(),
            Self::CoprocessorRegisterTrasfer => "".to_owned(),
            Self::SoftwareInterrupt => "".to_owned(),
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
        } else if op_code.get_bits(22..=27) == 0b000000 && op_code.get_bits(4..=7) == 0b1001 {
            Multiply
        } else if op_code.get_bits(23..=27) == 0b00001 && op_code.get_bits(4..=7) == 0b1001 {
            MultiplyLong
        } else if op_code.get_bits(25..=27) == 0b000
            && !op_code.get_bit(22)
            && op_code.get_bits(7..=11) == 0b00001
            && op_code.get_bit(4)
        {
            HalfwordDataTransferRegisterOffset
        } else if op_code.get_bits(25..=27) == 0b000
            && op_code.get_bit(22)
            && op_code.get_bit(7)
            && op_code.get_bit(4)
        {
            HalfwordDataTransferImmediateOffset
        } else if op_code.get_bits(25..=27) == 0b011 && op_code.get_bit(4) {
            log("undefined instruction decode...");
            Undefined
        } else if op_code.get_bits(24..=27) == 0b1111 {
            SoftwareInterrupt
        } else if op_code.get_bits(24..=27) == 0b1110 && op_code.get_bit(4) {
            CoprocessorRegisterTrasfer
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

impl Display for ArmModeInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ThumbModeInstruction {
    MoveShiftedRegister {
        op: ShiftKind,
        offset5: u16,
        rs: u16,
        rd: u16,
    },
    AddSubtract {
        operation_kind: OperandKind,
        op: bool,
        rn_offset3: u16,
        rs: u16,
        rd: u16,
    },
    MoveCompareAddSubtractImm {
        op: Operation,
        r_destination: u16,
        offset: u32,
    },
    AluOp {
        op: ThumbModeAluInstruction,
        rs: u16,
        rd: u16,
    },
    HiRegisterOpBX {
        op: ThumbHighRegisterOperation,
        source_register: u16,
        destination_register: u16,
    },
    PCRelativeLoad {
        r_destination: u16,
        immediate_value: u16,
    },
    LoadStoreRegisterOffset {
        load_store: LoadStoreKind,
        byte_word: ReadWriteKind,
        ro: u16,
        rb: u16,
        rd: u16,
    },
    LoadStoreSignExtByteHalfword {
        h_flag: bool,
        sign_extend_flag: bool,
        r_offset: u32,
        r_base: u32,
        r_destination: u32,
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
        r_destination: u16,
        word8: u16,
    },
    LoadAddress {
        sp: bool,
        r_destination: u32,
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

impl ThumbModeInstruction {
    pub(crate) fn disassembler(&self) -> String {
        match self {
            Self::MoveShiftedRegister {
                op,
                offset5,
                rs,
                rd,
            } => {
                format!("{op} R{rd}, R{rs}, #{offset5}")
            }
            Self::AddSubtract {
                operation_kind,
                op,
                rn_offset3,
                rs,
                rd,
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
                op,
                r_destination,
                offset,
            } => {
                format!("{op} R{r_destination}, #{offset}")
            }
            Self::AluOp { op, rs, rd } => {
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
                op,
                source_register,
                destination_register,
            } => {
                format!("{op} R{destination_register}, R{source_register}")
            }
            Self::PCRelativeLoad {
                r_destination,
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
                rb,
                rd,
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
                h_flag,
                sign_extend_flag,
                r_offset,
                r_base,
                r_destination,
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
                r_destination,
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
                r_destination,
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

impl From<u16> for ThumbModeInstruction {
    fn from(op_code: u16) -> Self {
        use ThumbModeInstruction::*;

        if op_code.get_bits(8..=15) == 0b11011111 {
            Swi
        } else if op_code.get_bits(8..=15) == 0b10110000 {
            // 0 - positive, 1 - negative
            let s = op_code.get_bit(7);
            // The offset supplied in #Imm is a full 10-bit address,
            // but must always be word-aligned (ie bits 1:0 set to 0),
            // since the assembler places #Imm >> 2 in the Word8 field.
            let word7 = op_code.get_bits(0..=6) << 2;

            AddOffsetSP { s, word7 }
        } else if op_code.get_bits(10..=15) == 0b010000 {
            let op: ThumbModeAluInstruction = op_code.get_bits(6..=9).into();
            let rs = op_code.get_bits(3..=5);
            let rd = op_code.get_bits(0..=2);

            AluOp { op, rs, rd }
        } else if op_code.get_bits(10..=15) == 0b010001 {
            let op = op_code.get_bits(8..=9).into();
            let h1 = op_code.get_bit(7);
            let source_register = op_code.get_bits(3..=6);
            let rd_hd = op_code.get_bits(0..=2);

            let destination_register = if h1 { rd_hd | (1 << 3) } else { rd_hd };

            HiRegisterOpBX {
                op,
                source_register,
                destination_register,
            }
        } else if op_code.get_bits(12..=15) == 0b1011 && op_code.get_bits(9..=10) == 0b10 {
            let load_store: LoadStoreKind = op_code.get_bit(11).into();
            let pc_lr = op_code.get_bit(8);
            let register_list = op_code.get_bits(0..=7);

            PushPopReg {
                load_store,
                pc_lr,
                register_list,
            }
        } else if op_code.get_bits(11..=15) == 0b00011 {
            let operation_kind: OperandKind = op_code.get_bit(10).into();
            // 0 - Add, 1 - Sub
            let op = op_code.get_bit(9);
            let rn_offset3 = op_code.get_bits(6..=8);
            let rs = op_code.get_bits(3..=5);
            let rd = op_code.get_bits(0..=2);

            AddSubtract {
                operation_kind,
                op,
                rn_offset3,
                rs,
                rd,
            }
        } else if op_code.get_bits(11..=15) == 0b01001 {
            let r_destination = op_code.get_bits(8..=10);
            let immediate_value = op_code.get_bits(0..=7) << 2;

            PCRelativeLoad {
                r_destination,
                immediate_value,
            }
        } else if op_code.get_bits(12..=15) == 0b0101 && !op_code.get_bit(9) {
            let load_store: LoadStoreKind = op_code.get_bit(11).into();
            let byte_word: ReadWriteKind = op_code.get_bit(10).into();
            let ro = op_code.get_bits(6..=8);
            let rb = op_code.get_bits(3..=5);
            let rd = op_code.get_bits(0..=2);

            LoadStoreRegisterOffset {
                load_store,
                byte_word,
                ro,
                rb,
                rd,
            }
        } else if op_code.get_bits(12..=15) == 0b0101 && op_code.get_bit(9) {
            LoadStoreSignExtByteHalfword {
                h_flag: op_code.get_bit(11),
                sign_extend_flag: op_code.get_bit(10),
                r_offset: op_code.get_bits(6..=8) as u32,
                r_base: op_code.get_bits(3..=5) as u32,
                r_destination: op_code.get_bits(0..=2) as u32,
            }
        } else if op_code.get_bits(11..=15) == 0b11100 {
            let offset = (op_code.get_bits(0..=10) << 1) as u32;
            UncondBranch { offset }
        } else if op_code.get_bits(12..=15) == 0b1000 {
            let load_store: LoadStoreKind = op_code.get_bit(11).into();
            let offset = op_code.get_bits(6..=10) << 1;
            let base_register = op_code.get_bits(3..=5);
            let source_destination_register = op_code.get_bits(0..=2);

            LoadStoreHalfword {
                load_store,
                offset,
                base_register,
                source_destination_register,
            }
        } else if op_code.get_bits(12..=15) == 0b1001 {
            let load_store: LoadStoreKind = op_code.get_bit(11).into();
            let r_destination = op_code.get_bits(8..=10);
            // The offset supplied in #Imm is a full 10-bit address,
            // but must always be word-aligned (ie bits 1:0 set to 0),
            // since the assembler places #Imm >> 2 in the Word8 field.
            let word8 = op_code.get_bits(0..=7) << 2;
            SPRelativeLoadStore {
                load_store,
                r_destination,
                word8,
            }
        } else if op_code.get_bits(12..=15) == 0b1010 {
            LoadAddress {
                sp: op_code.get_bit(11),
                r_destination: op_code.get_bits(8..=10) as u32,
                offset: (op_code.get_bits(0..=7) as u32) << 2,
            }
        } else if op_code.get_bits(12..=15) == 0b1100 {
            let load_store = op_code.get_bit(11).into();
            let base_register = op_code.get_bits(8..=10);
            let register_list = op_code.get_bits(0..=7);
            MultipleLoadStore {
                load_store,
                base_register,
                register_list,
            }
        } else if op_code.get_bits(12..=15) == 0b1101 {
            let condition = op_code.get_bits(8..=11) as u8;
            // 9 bits signed offset (assembler puts `label` >> 1 in this field so we should <<1)
            let offset = (op_code.get_bits(0..=7) << 1) as u32;
            let immediate_offset = offset.sign_extended(9) as i32;
            let condition = Condition::from(condition);

            CondBranch {
                condition,
                immediate_offset,
            }
        } else if op_code.get_bits(12..=15) == 0b1111 {
            let h = op_code.get_bit(11);
            let offset = op_code.get_bits(0..=10) as u32;

            LongBranchLink { h, offset }
        } else if op_code.get_bits(13..=15) == 0b000 {
            let op = op_code.get_bits(11..=12);
            let offset5 = op_code.get_bits(6..=10);
            let rs = op_code.get_bits(3..=5);
            let rd = op_code.get_bits(0..=2);
            MoveShiftedRegister {
                op: op.into(),
                offset5,
                rs,
                rd,
            }
        } else if op_code.get_bits(13..=15) == 0b001 {
            let op: Operation = op_code.get_bits(11..=12).into();
            let r_destination = op_code.get_bits(8..=10);
            let offset = op_code.get_bits(0..=7) as u32;

            MoveCompareAddSubtractImm {
                op,
                r_destination,
                offset,
            }
        } else if op_code.get_bits(13..=15) == 0b011 {
            LoadStoreImmOffset
        } else {
            log(format!("not identified instruction {op_code} "));
            unimplemented!()
        }
    }
}

impl Display for ThumbModeInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::arm7tdmi::Arm7tdmi;
    use crate::cpu::opcode::ArmModeOpcode;
    use pretty_assertions::assert_eq;

    #[test]
    fn decode_branch() {
        {
            let cpu = Arm7tdmi::default();
            let output: ArmModeInstruction = cpu.decode(0b1110_1011_0000_0000_0000_0000_0111_1111);
            assert_eq!(
                ArmModeInstruction::Branch {
                    condition: Condition::AL,
                    link: true,
                    offset: 508,
                },
                output
            );
        }
        {
            let cpu = Arm7tdmi::default();
            let output: ArmModeInstruction = cpu.decode(0b1110_1010_0000_0000_0000_0000_0111_1111);
            assert_eq!(
                ArmModeInstruction::Branch {
                    condition: Condition::AL,
                    link: false,
                    offset: 508,
                },
                output
            );
        }
        {
            let cpu = Arm7tdmi::default();
            let output: ArmModeInstruction = cpu.decode(0b0000_1010_0000_0000_0000_0000_0111_1111);
            assert_eq!(
                ArmModeInstruction::Branch {
                    condition: Condition::EQ,
                    link: false,
                    offset: 508,
                },
                output
            );
        }
    }

    #[test]
    fn decode_branch_and_exchange() {
        {
            let cpu = Arm7tdmi::default();
            let output: ArmModeInstruction = cpu.decode(0b1110_0001_0010_1111_1111_1111_0001_0001);
            assert_eq!(
                ArmModeInstruction::BranchAndExchange {
                    condition: Condition::AL,
                    register: 1
                },
                output
            );
        }
        {
            let cpu = Arm7tdmi::default();
            let output: ArmModeInstruction = cpu.decode(0b0000_0001_0010_1111_1111_1111_0001_0001);
            assert_eq!(
                ArmModeInstruction::BranchAndExchange {
                    condition: Condition::EQ,
                    register: 1
                },
                output
            );
        }
    }

    #[test]
    fn decode_data_processing() {
        {
            let op_code = 0b1110_00_0_1011_0_1001_1111_000000001110;
            let cpu = Arm7tdmi::default();
            let op_code: ArmModeOpcode = cpu.decode(op_code);
            assert_eq!(
                op_code.instruction,
                ArmModeInstruction::DataProcessing {
                    condition: Condition::AL,
                    alu_instruction: ArmModeAluInstruction::Cmn,
                    set_conditions: false,
                    op_kind: OperandKind::Register,
                    rn: 9,
                    destination: 15,
                    op2: AluSecondOperandInfo::Register {
                        shift_op: ShiftOperator::Immediate(0),
                        shift_kind: ShiftKind::Lsl,
                        register: 14,
                    }
                }
            );

            let asm = op_code.instruction.disassembler();
            assert_eq!(asm, "CMN R9, R14");
        }
    }

    #[test]
    fn decode_half_word_data_transfer_immediate_offset() {
        let cpu = Arm7tdmi::default();
        let output: ArmModeInstruction = cpu.decode(0b1110_0001_1100_0001_0000_0000_1011_0000);
        assert_eq!(
            ArmModeInstruction::HalfwordDataTransferImmediateOffset,
            output
        );
    }

    #[test]
    fn decode_multiple_load_store() {
        let cpu = Arm7tdmi::default();
        let output: ThumbModeInstruction = cpu.decode(0b1100_1001_1010_0000);
        assert_eq!(
            ThumbModeInstruction::MultipleLoadStore {
                load_store: LoadStoreKind::Load,
                base_register: 1,
                register_list: 160,
            },
            output
        );
    }
}
