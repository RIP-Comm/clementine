use crate::bitwise::Bits;
use crate::condition::Condition;
use crate::instruction::ArmModeInstruction;
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

        let op_code_format: &str = match self.instruction {
            ArmModeInstruction::DataProcessing1 => {
                "FMT: |_Cond__|0_0_0|___Op__|S|__Rn___|__Rd___|__Shift__|Typ|0|__Rm___|\n"
            }
            ArmModeInstruction::DataProcessing2 => {
                "FMT: |_Cond__|0_0_0|___Op__|S|__Rn___|__Rd___|__Rs___|0|Typ|1|__Rm___|\n"
            }
            ArmModeInstruction::DataProcessing3 => {
                "FMT: |_Cond__|0_0_1|___Op__|S|__Rn___|__Rd___|_Shift_|___Immediate___|\n"
            }
            ArmModeInstruction::TransImm9 => {
                "FMT: |_Cond__|0_1_0|P|U|B|W|L|__Rn___|__Rd___|_________Offset________|\n"
            }
            ArmModeInstruction::Branch | ArmModeInstruction::BranchLink => {
                "FMT: |_Cond__|1_0_1|L|___________________Offset______________________|\n"
            }
            ArmModeInstruction::Unknown => "",
        };

        let cond = self.get_bits(28..=31);
        let op_code_value: String = match self.instruction {
            ArmModeInstruction::DataProcessing1 => {
                let op = self.get_bits(21..=24);
                let s = self.get_bit(20) as u8;
                let rn = self.get_bits(16..=19);
                let rd = self.get_bits(12..=15);
                let shift_amount = self.get_bits(7..=11);
                let shift_type = self.get_bits(5..=6);
                let rm = self.get_bits(0..=3);

                format!("HEX: |{cond:07X}|0_0_0|{op:07X}|{s:01X}|{rn:07X}|{rd:07X}|{shift_amount:09X}|{shift_type:03X}|0|{rm:07X}|\n\
                         DEC: |{cond:07}|0_0_0|{op:07}|{s:01}|{rn:07}|{rd:07}|{shift_amount:09}|{shift_type:03}|0|{rm:07}|\n\
                         BIN: |{cond:07b}|0_0_0|{op:07b}|{s:01b}|{rn:07b}|{rd:07b}|{shift_amount:09b}|{shift_type:03b}|0|{rm:07b}|\n")
            }
            ArmModeInstruction::DataProcessing2 => {
                let op = self.get_bits(21..=24);
                let s = self.get_bit(20) as u8;
                let rn = self.get_bits(16..=19);
                let rd = self.get_bits(12..=15);
                let rs = self.get_bits(8..=11);
                let shift_type = self.get_bits(5..=6);
                let rm = self.get_bits(0..=3);

                format!("HEX: |{cond:07X}|0_0_0|{op:07X}|{s:01X}|{rn:07X}|{rd:07X}|{rs:07X}|0|{shift_type:03X}|1|{rm:07X}|\n\
                         DEC: |{cond:07}|0_0_0|{op:07}|{s:01}|{rn:07}|{rd:07}|{rs:07}|0|{shift_type:03}|1|{rm:07}|\n\
                         BIN: |{cond:07b}|0_0_0|{op:07b}|{s:01b}|{rn:07b}|{rd:07b}|{rs:07b}|0|{shift_type:03b}|1|{rm:07b}|\n")
            }
            ArmModeInstruction::DataProcessing3 => {
                let op = self.get_bits(21..=24);
                let s = self.get_bit(20) as u8;
                let rn = self.get_bits(16..=19);
                let rd = self.get_bits(12..=15);
                let shift_amount = self.get_bits(8..=11);
                let immediate = self.get_bits(0..=7);

                format!("HEX: |{cond:07X}|0_0_1|{op:07X}|{s:01X}|{rn:07X}|{rd:07X}|{shift_amount:07X}|{immediate:015X}|\n\
                         DEC: |{cond:07}|0_0_1|{op:07}|{s:01}|{rn:07}|{rd:07}|{shift_amount:07}|{immediate:015}|\n\
                         BIN: |{cond:07b}|0_0_1|{op:07b}|{s:01b}|{rn:07b}|{rd:07b}|{shift_amount:07b}|{immediate:015b}|\n")
            }
            ArmModeInstruction::TransImm9 => {
                let p = self.get_bit(24) as u8;
                let u = self.get_bit(23) as u8;
                let b = self.get_bit(22) as u8;
                let w = self.get_bit(21) as u8;
                let l = self.get_bit(20) as u8;
                let rn = self.get_bits(16..=19);
                let rd = self.get_bits(12..=15);
                let offset = self.get_bits(0..=11);

                format!("HEX: |{cond:07X}|0_1_0|{p:01X}|{u:01X}|{b:01X}|{w:01X}|{l:01X}|{rn:07X}|{rd:07X}|{offset:023X}|\n\
                         DEC: |{cond:07}|0_1_0|{p:01}|{u:01}|{b:01}|{w:01}|{l:01}|{rn:07}|{rd:07}|{offset:023}|\n\
                         BIN: |{cond:07b}|0_1_0|{p:01b}|{u:01b}|{b:01b}|{w:01b}|{l:01b}|{rn:07b}|{rd:07b}|{offset:023b}|\n")
            }
            ArmModeInstruction::Branch | ArmModeInstruction::BranchLink => {
                let l = self.get_bit(24) as u8;
                let offset = self.get_bits(0..=23);
                format!(
                    "HEX: |{cond:07X}|1_0_1|{l:01X}|{offset:047X}|\n\
                     DEC: |{cond:07}|1_0_1|{l:01}|{offset:047}|\n\
                     BIN: |{cond:07b}|1_0_1|{l:01b}|{offset:047b}|\n"
                )
            }
            ArmModeInstruction::Unknown => String::new(),
        };

        let mut raw_bits = String::new();
        for i in format!("{:b}", self.raw).chars() {
            raw_bits.push(i);
            raw_bits.push('_');
        }
        raw_bits.pop();
        let raw_bits = format!("RAW: |{}|\n", raw_bits);

        write!(
            f,
            "{instruction}{bytes_pos1}{bytes_pos2}{raw_bits}{op_code_format}{op_code_value}"
        )
    }
}
