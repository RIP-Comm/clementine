use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum ArmModeInstruction {
    DataProcessing1 = 0b0000_0000_0000_0000_0000_0000_0000_0000,
    DataProcessing2 = 0b0000_0000_0000_0000_0000_0000_0001_0000,
    DataProcessing3 = 0b0000_0010_0000_0000_0000_0000_0000_0000,
    Branch = 0b0000_1010_0000_0000_0000_0000_0000_0000,
    BranchLink = 0b0000_1011_0000_0000_0000_0000_0000_0000,
    TransImm9 = 0b0000_0100_0000_0000_0000_0000_0000_0000,
    BlockDataTransfer = 0b0000_1000_0000_0000_0000_0000_0000_0000,
    Unknown,
}

impl From<u32> for ArmModeInstruction {
    fn from(op_code: u32) -> Self {
        use ArmModeInstruction::*;

        if Self::check(DataProcessing1, op_code) {
            DataProcessing1
        } else if Self::check(DataProcessing2, op_code) {
            DataProcessing2
        } else if Self::check(DataProcessing3, op_code) {
            DataProcessing3
        } else if Self::check(Branch, op_code) {
            Branch
        } else if Self::check(BranchLink, op_code) {
            BranchLink
        } else if Self::check(TransImm9, op_code) {
            TransImm9
        } else if Self::check(BlockDataTransfer, op_code) {
            BlockDataTransfer
        } else {
            println!("{op_code:b}");
            Unknown
        }
    }
}

impl ArmModeInstruction {
    const fn check(instruction_type: Self, op_code: u32) -> bool {
        (Self::get_mask(&instruction_type) & op_code) == instruction_type as u32
    }

    const fn get_mask(instruction_type: &Self) -> u32 {
        use ArmModeInstruction::*;

        match instruction_type {
            Branch | BranchLink => 0b0000_1111_0000_0000_0000_0000_0000_0000,
            DataProcessing1 | DataProcessing2 => 0b0000_1110_0000_0000_0000_0000_0001_0000,
            DataProcessing3 => 0b0000_1110_0000_0000_0000_0000_0000_0000,
            TransImm9 => 0b0000_1110_0000_0000_0000_0000_0000_0000,
            BlockDataTransfer => 0b0000_1110_0000_0000_0000_0000_0000_0000,
            Unknown => 0b1111_1111_1111_1111_1111_1111_1111_1111,
        }
    }
}

impl Display for ArmModeInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
