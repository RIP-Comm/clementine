#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArmModeInstruction {
    DataProcessing1 = 0x00_00_00_00,
    DataProcessing2 = 0x00_00_00_10,
    DataProcessing3 = 0x02_00_00_00,
    Branch = 0x0A_00_00_00,
    BranchLink = 0x0B_00_00_00,
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
        } else {
            Err("instruction not implemented :(.".to_owned())
        }
    }
}

impl ArmModeInstruction {
    fn check(instruction_type: ArmModeInstruction, op_code: u32) -> bool {
        (Self::get_mask(&instruction_type) & op_code) == instruction_type as u32
    }

    fn get_mask(instruction_type: &ArmModeInstruction) -> u32 {
        use ArmModeInstruction::*;

        match instruction_type {
            Branch | BranchLink => 0x0F_00_00_00,
            DataProcessing1 | DataProcessing2 => 0x0E_00_00_10,
            DataProcessing3 => 0x0E_00_00_00,
        }
    }
}
