use crate::bitwise::Bits;
use crate::cpu::condition::Condition;
use crate::cpu::instruction::{ArmModeInstruction, ThumbModeInstruction};
use std::fmt::{Display, Formatter};
use std::ops::Deref;

pub struct ArmModeOpcode {
    pub instruction: ArmModeInstruction,
    pub condition: Condition,
    pub raw: u32,
}

impl TryFrom<u32> for ArmModeOpcode {
    type Error = String;

    fn try_from(op_code: u32) -> Result<Self, Self::Error> {
        Ok(Self {
            instruction: ArmModeInstruction::from(op_code),
            condition: Condition::from(op_code.get_bits(28..=31) as u8),
            raw: op_code,
        })
    }
}

impl Deref for ArmModeOpcode {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl Display for ArmModeOpcode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let instruction = self.instruction.to_string();
        let instruction = format!("INS: {}\n", instruction);

        let bytes_pos1 = "POS: |..3 ..................2 ..................1 ..................0|\n";
        let bytes_pos2 = "     |1_0_9_8_7_6_5_4_3_2_1_0_9_8_7_6_5_4_3_2_1_0_9_8_7_6_5_4_3_2_1_0|\n";

        let op_code_format: &str = match &self.instruction {
            ArmModeInstruction::DataProcessing => {
                "FMT: |_Cond__|0_0|I|_code__|S|__Rn___|__Rd___|_______operand2________|"
            }
            ArmModeInstruction::Multiply => "FMT: |_Cond__|",
            ArmModeInstruction::MultiplyLong => "FMT: |_Cond__|",
            ArmModeInstruction::SingleDataSwap => "FMT: |_Cond__|",
            ArmModeInstruction::BranchAndExchange => {
                "FMT: |_Cond__|0_0_0_1|0_0_1_0|1_1_1_1|1_1_1_1|1_1_1_1|0_0_0_1|__Rn___|"
            }
            ArmModeInstruction::HalfwordDataTransferRegisterOffset => {
                "FMT: |_Cond__|0_0_0|P|U|0|W|L|__Rn___|__Rd___|0_0_0_0|1|S|H|1|__Rm___|"
            }
            ArmModeInstruction::HalfwordDataTransferImmediateOffset => {
                "FMT: |_Cond__|0_0_0|P|U|1|W|L|__Rn___|__Rd___|_Offset|1|S|H|1|_Offset|"
            }
            ArmModeInstruction::SingleDataTransfer => {
                "FMT: |_Cond__|0_1|I|P|U|B|W|L|__Rn___|__Rd___|________Offset_________|"
            }
            ArmModeInstruction::Undefined => "FMT: |_Cond__|",
            ArmModeInstruction::BlockDataTransfer => {
                "FMT: |_Cond__|1_0_0|P|U|S|W|L|__Rn___|_____________Reg_List__________|"
            }
            ArmModeInstruction::Branch => {
                "FMT: |_Cond__|1_0_1|L|______________________Offset___________________|"
            }
            ArmModeInstruction::CoprocessorDataTransfer => {
                "FMT: |_Cond__|1_1_0|P|U|N|W|L|__Rn___|__CRd__|__Cp#__|____Offset_____|"
            }
            ArmModeInstruction::CoprocessorDataOperation => "FMT: |_Cond__|",
            ArmModeInstruction::CoprocessorRegisterTrasfer => "FMT: |_Cond__|",
            ArmModeInstruction::SoftwareInterrupt => "FMT: |_Cond__|",
        };

        let mut raw_bits = String::new();
        for i in format!("{:#034b}", self.raw).chars().skip(2) {
            raw_bits.push(i);
            raw_bits.push('_');
        }
        raw_bits.pop();
        let raw_bits = format!("RAW: |{}|\n", raw_bits);

        writeln!(
            f,
            "{instruction}{bytes_pos1}{bytes_pos2}{raw_bits}{op_code_format}"
        )
    }
}

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

impl Deref for ThumbModeOpcode {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl Display for ThumbModeOpcode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let instruction = self.instruction.to_string();
        let instruction = format!("INS: {}\n", instruction);

        let bytes_pos1 = "POS: |..........1 ..................0|\n";
        let bytes_pos2 = "     |5_4_3_2_1_0_9_8_7_6_5_4_3_2_1_0|\n";

        let op_code_format: &str = match &self.instruction {
            ThumbModeInstruction::MoveShiftedRegister => "FMT: |0_0_1|Op_|__Offset_|_Rs__|_Rd__|",
            ThumbModeInstruction::AddSubtract => "FMT: |0_0_0_1_1|I|O|RnOff|_Rs__|_Rd__|",
            ThumbModeInstruction::MoveCompareAddSubtractImm => {
                "FMT: |0_0_1|Op_|_Rn__|____Offset_____|"
            }
            ThumbModeInstruction::AluOp => todo!(),
            ThumbModeInstruction::HiRegisterOpBX => todo!(),
            ThumbModeInstruction::PCRelativeLoad => "FMT: |0_1_0_0_1|_Rn__|_____Word8_____|",
            ThumbModeInstruction::LoadStoreRegisterOffset => {
                "FMT: |0_1_0_1|L|B|0|_Ro__|_Rb__|_Rd__|"
            }
            ThumbModeInstruction::LoadStoreSignExtByteHalfword => todo!(),
            ThumbModeInstruction::LoadStoreImmOffset => todo!(),
            ThumbModeInstruction::LoadStoreHalfword => todo!(),
            ThumbModeInstruction::SPRelativeLoadStore => todo!(),
            ThumbModeInstruction::LoadAddress => todo!(),
            ThumbModeInstruction::AddOffsetSP => todo!(),
            ThumbModeInstruction::PushPopReg => todo!(),
            ThumbModeInstruction::MultipleLoadStore => todo!(),
            ThumbModeInstruction::CondBranch => todo!(),
            ThumbModeInstruction::Swi => todo!(),
            ThumbModeInstruction::UncondBranch => todo!(),
            ThumbModeInstruction::LongBranchLink => todo!(),
        };

        let mut raw_bits = String::new();
        for i in format!("{:#018b}", self.raw).chars().skip(2) {
            raw_bits.push(i);
            raw_bits.push('_');
        }
        raw_bits.pop();
        let raw_bits = format!("RAW: |{}|\n", raw_bits);

        writeln!(
            f,
            "{instruction}{bytes_pos1}{bytes_pos2}{raw_bits}{op_code_format}"
        )
    }
}
