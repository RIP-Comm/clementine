use std::fmt::Display;

use crate::bitwise::Bits;
use crate::cpu::flags::ShiftKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ArmModeAluInstruction {
    And = 0x0,
    Eor = 0x1,
    Sub = 0x2,
    Rsb = 0x3,
    Add = 0x4,
    Adc = 0x5,
    Sbc = 0x6,
    Rsc = 0x7,
    Tst = 0x8,
    Teq = 0x9,
    Cmp = 0xA,
    Cmn = 0xB,
    Orr = 0xC,
    Mov = 0xD,
    Bic = 0xE,
    Mvn = 0xF,
}

impl Display for ArmModeAluInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::And => f.write_str("AND"),
            Self::Eor => f.write_str("EOR"),
            Self::Sub => f.write_str("SUB"),
            Self::Rsb => f.write_str("RSB"),
            Self::Add => f.write_str("ADD"),
            Self::Adc => f.write_str("ADC"),
            Self::Sbc => f.write_str("SBC"),
            Self::Rsc => f.write_str("RSC"),
            Self::Tst => f.write_str("TST"),
            Self::Teq => f.write_str("TEQ"),
            Self::Cmp => f.write_str("CMP"),
            Self::Cmn => f.write_str("CMN"),
            Self::Orr => f.write_str("ORR"),
            Self::Mov => f.write_str("MOV"),
            Self::Bic => f.write_str("BIC"),
            Self::Mvn => f.write_str("MVN"),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum AluInstructionKind {
    Logical,
    Arithmetic,
}

pub trait Kind {
    fn kind(&self) -> AluInstructionKind;
}

impl Kind for ArmModeAluInstruction {
    fn kind(&self) -> AluInstructionKind {
        use ArmModeAluInstruction::*;
        match &self {
            And | Eor | Tst | Teq | Orr | Mov | Bic | Mvn => AluInstructionKind::Logical,
            Sub | Rsb | Add | Adc | Sbc | Rsc | Cmp | Cmn => AluInstructionKind::Arithmetic,
        }
    }
}

impl From<u32> for ArmModeAluInstruction {
    fn from(alu_op_code: u32) -> Self {
        use ArmModeAluInstruction::*;
        match alu_op_code {
            0x0 => And,
            0x1 => Eor,
            0x2 => Sub,
            0x3 => Rsb,
            0x4 => Add,
            0x5 => Adc,
            0x6 => Sbc,
            0x7 => Rsc,
            0x8 => Tst,
            0x9 => Teq,
            0xA => Cmp,
            0xB => Cmn,
            0xC => Orr,
            0xD => Mov,
            0xE => Bic,
            0xF => Mvn,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ArithmeticOpResult {
    pub result: u32,
    pub carry: bool,
    pub overflow: bool,
    pub sign: bool,
    pub zero: bool,
}

pub fn shift(kind: ShiftKind, shift_amount: u32, rm: u32, carry: bool) -> ArithmeticOpResult {
    match kind {
        ShiftKind::Lsl => {
            match shift_amount {
                // LSL#0: No shift performed, ie. directly value=Rm, the C flag is NOT affected.
                0 => ArithmeticOpResult {
                    result: rm,
                    carry,
                    ..Default::default()
                },
                // LSL#1..32: Normal left logical shift
                1..=32 => ArithmeticOpResult {
                    result: rm << shift_amount,
                    carry: rm.get_bit((32 - shift_amount).try_into().unwrap()),
                    ..Default::default()
                },
                // LSL#33...: Result is 0 and carry is 0
                _ => ArithmeticOpResult {
                    carry: false,
                    ..Default::default()
                },
            }
        }
        ShiftKind::Lsr => {
            match shift_amount {
                // LSR#0 is used to encode LSR#32, it has 0 result and carry equal to bit 31 of Rm
                0 => ArithmeticOpResult {
                    result: 0,
                    carry: rm.get_bit(31),
                    ..Default::default()
                },
                // LSR#1..32: Normal right logical shift
                1..=32 => ArithmeticOpResult {
                    result: rm >> shift_amount,
                    carry: rm.get_bit((shift_amount - 1).try_into().unwrap()),
                    ..Default::default()
                },
                _ => ArithmeticOpResult {
                    result: 0,
                    carry: false,
                    ..Default::default()
                },
            }
        }
        ShiftKind::Asr => match shift_amount {
            1..=31 => ArithmeticOpResult {
                result: ((rm as i32) >> shift_amount) as u32,
                carry: rm.get_bit((shift_amount - 1).try_into().unwrap()),
                ..Default::default()
            },
            _ => ArithmeticOpResult {
                result: ((rm as i32) >> 31) as u32,
                carry: rm.get_bit(31),
                ..Default::default()
            },
        },
        ShiftKind::Ror => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_logical_instruction() {
        let alu_op_code = 9;
        let instruction_kind = ArmModeAluInstruction::from(alu_op_code).kind();

        assert_eq!(instruction_kind, AluInstructionKind::Logical);
    }

    #[test]
    fn test_arithmetic_instruction() {
        let alu_op_code = 2;
        let instruction_kind = ArmModeAluInstruction::from(alu_op_code).kind();

        assert_eq!(instruction_kind, AluInstructionKind::Arithmetic);
    }
}
