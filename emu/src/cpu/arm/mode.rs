use crate::bitwise::Bits;
use crate::cpu::arm::instructions::ArmModeInstruction;
use crate::cpu::condition::Condition;

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

impl std::ops::Deref for ArmModeOpcode {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl std::fmt::Display for ArmModeOpcode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let instruction = self.instruction.to_string();
        let instruction = format!("INS: {instruction}\n");

        let bytes_pos1 = "POS: |..3 ..................2 ..................1 ..................0|\n";
        let bytes_pos2 = "     |1_0_9_8_7_6_5_4_3_2_1_0_9_8_7_6_5_4_3_2_1_0_9_8_7_6_5_4_3_2_1_0|\n";

        let op_code_format: &str = match &self.instruction {
            ArmModeInstruction::DataProcessing { .. } => {
                "FMT: |_Cond__|0_0|I|_code__|S|__Rn___|__Rd___|_______operand2________|"
            }
            ArmModeInstruction::Multiply => "FMT: |_Cond__|",
            ArmModeInstruction::MultiplyLong => "FMT: |_Cond__|",
            ArmModeInstruction::SingleDataSwap => "FMT: |_Cond__|",
            ArmModeInstruction::BranchAndExchange { .. } => {
                "FMT: |_Cond__|0_0_0_1|0_0_1_0|1_1_1_1|1_1_1_1|1_1_1_1|0_0_0_1|__Rn___|"
            }
            ArmModeInstruction::HalfwordDataTransferRegisterOffset => {
                "FMT: |_Cond__|0_0_0|P|U|0|W|L|__Rn___|__Rd___|0_0_0_0|1|S|H|1|__Rm___|"
            }
            ArmModeInstruction::HalfwordDataTransferImmediateOffset => {
                "FMT: |_Cond__|0_0_0|P|U|1|W|L|__Rn___|__Rd___|_Offset|1|S|H|1|_Offset|"
            }
            ArmModeInstruction::SingleDataTransfer { .. } => {
                "FMT: |_Cond__|0_1|I|P|U|B|W|L|__Rn___|__Rd___|________Offset_________|"
            }
            ArmModeInstruction::Undefined => "FMT: |_Cond__|",
            ArmModeInstruction::BlockDataTransfer { .. } => {
                "FMT: |_Cond__|1_0_0|P|U|S|W|L|__Rn___|_____________Reg_List__________|"
            }
            ArmModeInstruction::Branch { .. } => {
                "FMT: |_Cond__|1_0_1|L|______________________Offset___________________|"
            }
            ArmModeInstruction::CoprocessorDataTransfer { .. } => {
                "FMT: |_Cond__|1_1_0|P|U|N|W|L|__Rn___|__CRd__|__Cp#__|____Offset_____|"
            }
            ArmModeInstruction::CoprocessorDataOperation => "FMT: |_Cond__|",
            ArmModeInstruction::CoprocessorRegisterTransfer => "FMT: |_Cond__|",
            ArmModeInstruction::SoftwareInterrupt => "FMT: |_Cond__|",
        };

        let mut raw_bits = String::new();
        for i in format!("{:#034b}", self.raw).chars().skip(2) {
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
