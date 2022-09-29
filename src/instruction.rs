use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum ArmModeInstruction {
    DataProcessing1 = 0b0000_0000_0000_0000_0000_0000_0000_0000,
    DataProcessing2 = 0b0000_0000_0000_0000_0000_0000_0001_0000,
    DataProcessing3 = 0b0000_0010_0000_0000_0000_0000_0000_0000,
    Branch = 0b0000_1010_0000_0000_0000_0000_0000_0000,
    BranchLink = 0b0000_1011_0000_0000_0000_0000_0000_0000,
    DataTransfer = 0b0000_0100_0000_0000_0000_0000_0000_0000,
}

impl TryFrom<u32> for ArmModeInstruction {
    type Error = String;

    fn try_from(op_code: u32) -> Result<Self, Self::Error> {
        use ArmModeInstruction::*;

        if Self::check(DataProcessing1, op_code) {
            Ok(DataProcessing1)
        } else if Self::check(DataProcessing2, op_code) {
            Ok(DataProcessing2)
        } else if Self::check(DataProcessing3, op_code) {
            Ok(DataProcessing3)
        } else if Self::check(Branch, op_code) {
            Ok(Branch)
        } else if Self::check(BranchLink, op_code) {
            Ok(BranchLink)
        } else if Self::check(DataTransfer, op_code) {
            Ok(DataTransfer)
        } else {
            Err("instruction not implemented :(.".to_owned())
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
            DataTransfer => 0b0000_1100_0000_0000_0000_0000_0000_0000,
        }
    }
}

impl Display for ArmModeInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let instruction_str = match self {
            Self::DataProcessing1 => "DataProcessing1",
            Self::DataProcessing2 => "DataProcessing2",
            Self::DataProcessing3 => "DataProcessing3",
            Self::Branch => "Branch",
            Self::BranchLink => "BranchLink",
            Self::DataTransfer => "DataTransfer",
        };

        write!(f, "{}", instruction_str)
    }
}
