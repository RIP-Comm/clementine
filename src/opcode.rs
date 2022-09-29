use crate::instruction::ArmModeInstruction;

pub struct Opcode {
    pub instruction: ArmModeInstruction,
    pub raw_value: u32,
}

impl TryFrom<u32> for Opcode {
    type Error = String;

    fn try_from(opcode: u32) -> Result<Self, Self::Error> {
        Ok(Self {
            instruction: ArmModeInstruction::try_from(opcode)?,
            raw_value: opcode,
        })
    }
}
