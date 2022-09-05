#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArmModeInstruction {
    Branch = 0x0A_00_00_00,
    BranchLink = 0x0B_00_00_00,

    /// 27-26 must be 0b00
    /// 24-21 must be 0b1101
    /// 19-16 must be 0b0000
    Mov = 0x01_A0_00_00,
}

impl TryFrom<u32> for ArmModeInstruction {
    type Error = String;

    fn try_from(op_code: u32) -> Result<Self, Self::Error> {
        use ArmModeInstruction::*;

        if Self::check(Branch, op_code) {
            Ok(Branch)
        } else if Self::check(BranchLink, op_code) {
            Ok(BranchLink)
        } else if Self::check(Mov, op_code) {
            Ok(Mov)
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
            Mov => 0x0D_EF_00_00,
            _ => todo!(),
        }
    }
}
