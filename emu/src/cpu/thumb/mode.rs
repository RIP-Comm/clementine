use crate::cpu::thumb::instruction::ThumbModeInstruction;

#[derive(Copy, Clone)]
pub struct ThumbModeOpcode {
    pub instruction: ThumbModeInstruction,
    pub raw: u16,
}

impl TryFrom<u16> for ThumbModeOpcode {
    type Error = String;

    fn try_from(op_code: u16) -> Result<Self, Self::Error> {
        Ok(Self {
            instruction: ThumbModeInstruction::from(op_code),
            raw: op_code,
        })
    }
}

impl std::ops::Deref for ThumbModeOpcode {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl std::fmt::Display for ThumbModeOpcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let instruction = self.instruction.to_string();
        let instruction = format!("INS: {instruction}\n");

        let bytes_pos1 = "POS: |..........1 ..................0|\n";
        let bytes_pos2 = "     |5_4_3_2_1_0_9_8_7_6_5_4_3_2_1_0|\n";

        let op_code_format: &str = match &self.instruction {
            ThumbModeInstruction::MoveShiftedRegister { .. } => {
                "FMT: |0_0_1|Op_|__Offset_|_Rs__|_Rd__|"
            }
            ThumbModeInstruction::AddSubtract { .. } => "FMT: |0_0_0_1_1|I|O|RnOff|_Rs__|_Rd__|",
            ThumbModeInstruction::MoveCompareAddSubtractImm { .. } => {
                "FMT: |0_0_1|Op_|_Rn__|____Offset_____|"
            }
            ThumbModeInstruction::AluOp { .. } => "FMT: |0_1_0_0_0_0|__Op___|_Rs__|_Rd__|",
            ThumbModeInstruction::HiRegisterOpBX { .. } => "FMT: |0_1_0_0_0_1|_Op|H|H|Rs/Hs|Rd/Hd|",
            ThumbModeInstruction::PCRelativeLoad { .. } => "FMT: |0_1_0_0_1|_Rn__|_____Word8_____|",
            ThumbModeInstruction::LoadStoreRegisterOffset { .. } => {
                "FMT: |0_1_0_1|L|B|0|_Ro__|_Rb__|_Rd__|"
            }
            ThumbModeInstruction::LoadStoreSignExtByteHalfword { .. } => {
                "FMT: |0_1_0_1|H|S|1|_Ro__|_Rb__|_Rd__|"
            }
            ThumbModeInstruction::LoadStoreImmOffset => "FMT: |0_1_1|B|L|_Offset5_|_Rb__|_Rd__|",
            ThumbModeInstruction::LoadStoreHalfword { .. } => {
                "FMT: |1_0_0_0|L|_Offset5_|_Rb__|_Rd__|"
            }
            ThumbModeInstruction::SPRelativeLoadStore { .. } => {
                "FMT: |1_0_0_1|L|_Rd__|_____Word8_____|"
            }
            ThumbModeInstruction::LoadAddress { .. } => "FMT: |1_0_0_1|S|_Rd__|_____Word8_____|",
            ThumbModeInstruction::AddOffsetSP { .. } => "FMT: |1_0_1_1_0_0_0_0|S|____Word7____|",
            ThumbModeInstruction::PushPopReg { .. } => "FMT: |1_0_1_1|L|1_0|R|_____Rlist_____|",
            ThumbModeInstruction::MultipleLoadStore { .. } => {
                "FMT: |1_1_0_0|L|_Rb__|_____Rlist_____|"
            }
            ThumbModeInstruction::CondBranch { .. } => "FMT: |1_1_0_1|_Cond__|_____Offset____|",
            ThumbModeInstruction::Swi => todo!(),
            ThumbModeInstruction::UncondBranch { .. } => "FMT: |1_1_1_0_0|________Offset11_____|",
            ThumbModeInstruction::LongBranchLink { .. } => "FMT: |1_1_1_1|H|_______Offset________|",
        };

        let mut raw_bits = String::new();
        for i in format!("{:#018b}", self.raw).chars().skip(2) {
            raw_bits.push(i);
            raw_bits.push('_');
        }
        raw_bits.pop();
        let raw_bits = format!("RAW: |{raw_bits}|\n");

        writeln!(
            f,
            "{instruction}{bytes_pos1}{bytes_pos2}{raw_bits}{op_code_format}"
        )
    }
}
