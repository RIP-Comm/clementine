use std::fmt::{Display, Formatter};

use logger::log;

use crate::bitwise::Bits;
use crate::cpu::alu_instruction::ArmModeAluInstruction;
use crate::cpu::arm7tdmi::Arm7tdmi;
use crate::cpu::flags::{Indexing, Offsetting, OperandKind, ReadWriteKind};
use crate::cpu::single_data_transfer::SingleDataTransferKind;

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
    },
    Multiply,
    MultiplyLong,
    SingleDataSwap,
    BranchAndExchange(Condition, usize),
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
        offset: u32,
        offsetting: Offsetting,
    },
    Undefined,
    BlockDataTransfer,
    Branch(Condition, bool, u32),
    CoprocessorDataTransfer,
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
            } => {
                let set_string = if *set_conditions { "s" } else { "" };
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
                        format!("{alu_instruction}{condition}{condition} R{destination}, R{rn}")
                    }
                    ArmModeAluInstruction::Tst
                    | ArmModeAluInstruction::Teq
                    | ArmModeAluInstruction::Cmp
                    | ArmModeAluInstruction::Cmn => {
                        format!("{alu_instruction}{condition} R{rn}")
                    }
                    ArmModeAluInstruction::Mov | ArmModeAluInstruction::Mvn => {
                        format!("{alu_instruction}{condition}{set_string} R{destination}")
                    }
                }
            }
            Self::Multiply => "".to_owned(),
            Self::MultiplyLong => "".to_owned(),
            Self::SingleDataSwap => "".to_owned(),
            Self::BranchAndExchange(_, reg) => format!("bx R{reg}"),
            Self::HalfwordDataTransferRegisterOffset => "".to_owned(),
            Self::HalfwordDataTransferImmediateOffset => "".to_owned(),
            Self::SingleDataTransfer {
                condition,
                kind,
                quantity,
                write_back,
                indexing,
                rd,
                base_register,
                offset,
                offsetting: _,
            } => {
                let q = match quantity {
                    ReadWriteKind::Word => "W",
                    ReadWriteKind::Byte => "B",
                };

                let w = if *write_back { "!" } else { "" };

                // let i = match indexing {
                //     Indexing::Post => format!("[{base_register}], {offset}"),
                //     Indexing::Pre => format!("[{base_register}, {offset}]"),
                // };
                let _ = offset;
                let _ = indexing;
                // FIXME: finish the address
                let address = base_register;

                let op = match kind {
                    SingleDataTransferKind::Ldr => "LDR",
                    SingleDataTransferKind::Str => "STR",
                    SingleDataTransferKind::Pld => unimplemented!(),
                };

                format!("{op}{condition}{q} {rd},{address} {w}")
            }
            Self::Undefined => "".to_owned(),
            Self::BlockDataTransfer => "".to_owned(),
            Self::Branch(cond, is_link, address) => {
                let link = if *is_link { "l" } else { "" };
                format!("b{cond}{link} 0x{address:08X}")
            }
            Self::CoprocessorDataTransfer => "".to_owned(),
            Self::CoprocessorDataOperation => "".to_owned(),
            Self::CoprocessorRegisterTrasfer => "".to_owned(),
            Self::SoftwareInterrupt => "".to_owned(),
        }
    }
}

impl Arm7tdmi {
    pub(crate) fn decode_instruction(
        &mut self,
        op_code: u32,
        condition: Condition,
    ) -> ArmModeInstruction {
        use ArmModeInstruction::*;

        // NOTE: The order is based on how many bits are already know at decoding time.
        // It can happen `op_code` coalesced into one/two or more than two possible solution, that's because
        // we tried to order with this priority.
        if op_code.get_bits(4..=27) == 0b0001_0010_1111_1111_1111_0001 {
            let rn = op_code.get_bits(0..=3) as usize;
            BranchAndExchange(condition, rn)
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
            CoprocessorDataTransfer
        } else if op_code.get_bits(25..=27) == 0b100 {
            BlockDataTransfer
        } else if op_code.get_bits(25..=27) == 0b101 {
            let is_link = op_code.get_bit(24);
            let offset = op_code.get_bits(0..=23) << 2;
            Branch(condition, is_link, offset)
        } else if op_code.get_bits(26..=27) == 0b01 {
            let op_kind: OperandKind = op_code.get_bit(25).into();
            let indexing: Indexing = op_code.get_bit(24).into(); // FIXME: should we use this?
            let offsetting: Offsetting = op_code.get_bit(23).into();
            let byte_or_word: ReadWriteKind = op_code.into(); // TODO: is this the same for all instruction?
            let load_store: SingleDataTransferKind = op_code.into(); // TODO: is this the same bit for all instruction?
            let write_back = op_code.get_bit(21);
            let rn = op_code.get_bits(16..=19);
            let rd = op_code.get_bits(12..=15);

            let offset = match op_kind {
                OperandKind::Immediate => todo!(),
                OperandKind::Register => op_code.get_bits(0..=11),
            };

            SingleDataTransfer {
                condition,
                kind: load_store,
                quantity: byte_or_word,
                write_back,
                indexing,
                rd,
                base_register: rn,
                offset,
                offsetting,
            }
        } else if op_code.get_bits(26..=27) == 0b00 {
            let alu_instruction = op_code.get_bits(21..=24).into();
            let set_conditions = op_code.get_bit(20);
            let rn = op_code.get_bits(16..=19);
            let op_kind: OperandKind = op_code.get_bit(25).into();
            let rd = op_code.get_bits(12..=15);

            DataProcessing {
                condition,
                alu_instruction,
                set_conditions,
                op_kind,
                rn,
                destination: rd,
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
    MoveShiftedRegister,
    AddSubtract,
    MoveCompareAddSubtractImm,
    AluOp,
    HiRegisterOpBX,
    PCRelativeLoad,
    LoadStoreRegisterOffset,
    LoadStoreSignExtByteHalfword,
    LoadStoreImmOffset,
    LoadStoreHalfword,
    SPRelativeLoadStore,
    LoadAddress,
    AddOffsetSP,
    PushPopReg,
    MultipleLoadStore,
    CondBranch,
    Swi,
    UncondBranch,
    LongBranchLink,
}

impl From<u16> for ThumbModeInstruction {
    fn from(op_code: u16) -> Self {
        use ThumbModeInstruction::*;

        if op_code.get_bits(8..=15) == 0b11011111 {
            Swi
        } else if op_code.get_bits(8..=15) == 0b10110000 {
            AddOffsetSP
        } else if op_code.get_bits(10..=15) == 0b010000 {
            AluOp
        } else if op_code.get_bits(10..=15) == 0b010001 {
            HiRegisterOpBX
        } else if op_code.get_bits(12..=15) == 0b1011 && op_code.get_bits(9..=10) == 0b10 {
            PushPopReg
        } else if op_code.get_bits(11..=15) == 0b00011 {
            AddSubtract
        } else if op_code.get_bits(11..=15) == 0b01001 {
            PCRelativeLoad
        } else if op_code.get_bits(12..=15) == 0b0101 && !op_code.get_bit(9) {
            LoadStoreRegisterOffset
        } else if op_code.get_bits(12..=15) == 0b0101 && op_code.get_bit(9) {
            LoadStoreSignExtByteHalfword
        } else if op_code.get_bits(11..=15) == 0b11100 {
            UncondBranch
        } else if op_code.get_bits(12..=15) == 0b1000 {
            LoadStoreHalfword
        } else if op_code.get_bits(12..=15) == 0b1001 {
            SPRelativeLoadStore
        } else if op_code.get_bits(12..=15) == 0b1010 {
            LoadAddress
        } else if op_code.get_bits(12..=15) == 0b1100 {
            MultipleLoadStore
        } else if op_code.get_bits(12..=15) == 0b1101 {
            CondBranch
        } else if op_code.get_bits(12..=15) == 0b1111 {
            LongBranchLink
        } else if op_code.get_bits(13..=15) == 0b000 {
            MoveShiftedRegister
        } else if op_code.get_bits(13..=15) == 0b001 {
            MoveCompareAddSubtractImm
        } else if op_code.get_bits(13..=15) == 0b011 {
            LoadStoreImmOffset
        } else {
            log("not identified instruction");
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

    #[test]
    fn decode_half_word_data_transfer_immediate_offset() {
        let mut cpu = Arm7tdmi::default();
        let output: ArmModeInstruction =
            cpu.decode_instruction(0b1110_0001_1100_0001_0000_0000_1011_0000, Condition::AL);
        assert_eq!(
            ArmModeInstruction::HalfwordDataTransferImmediateOffset,
            output
        );
    }

    // FIXME: Not sure about this, just because `BranchAndExchange` if is first.
    #[test]
    fn decode_branch_and_exchange() {
        let mut cpu = Arm7tdmi::default();
        let output: ArmModeInstruction =
            cpu.decode_instruction(0b1110_0001_0010_1111_1111_1111_0001_0001, Condition::AL);
        assert_eq!(
            ArmModeInstruction::BranchAndExchange(Condition::AL, 1),
            output
        );
    }

    #[test]
    fn decode_branch_link() {
        let mut cpu = Arm7tdmi::default();
        let output: ArmModeInstruction =
            cpu.decode_instruction(0b1110_1011_0000_0000_0000_0000_0111_1111, Condition::AL);
        assert_eq!(ArmModeInstruction::Branch(Condition::AL, true, 508), output);
    }
}
