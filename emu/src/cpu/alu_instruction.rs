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

#[derive(Debug, Eq, PartialEq)]
pub enum ThumbModeAluInstruction {
    And = 0x0,
    Eor = 0x1,
    Lsl = 0x2,
    Lsr = 0x3,
    Asr = 0x4,
    Adc = 0x5,
    Sbc = 0x6,
    Ror = 0x7,
    Tst = 0x8,
    Neg = 0x9,
    Cmp = 0xA,
    Cmn = 0xB,
    Orr = 0xC,
    Mul = 0xD,
    Bic = 0xE,
    Mvn = 0xF,
}

impl From<u16> for ThumbModeAluInstruction {
    fn from(alu_op_code: u16) -> Self {
        use ThumbModeAluInstruction::*;
        match alu_op_code {
            0x0 => And,
            0x1 => Eor,
            0x2 => Lsl,
            0x3 => Lsr,
            0x4 => Asr,
            0x5 => Adc,
            0x6 => Sbc,
            0x7 => Ror,
            0x8 => Tst,
            0x9 => Neg,
            0xA => Cmp,
            0xB => Cmn,
            0xC => Orr,
            0xD => Mul,
            0xE => Bic,
            0xF => Mvn,
            _ => unreachable!(),
        }
    }
}

pub enum ShiftKind {
    Lsl,
    Lsr,
    Asr,
    Ror,
}

impl From<u16> for ShiftKind {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            3 => Self::Ror,
            _ => unreachable!(),
        }
    }
}

impl From<u32> for ShiftKind {
    fn from(op: u32) -> Self {
        match op {
            0 => Self::Lsl,
            1 => Self::Lsr,
            2 => Self::Asr,
            3 => Self::Ror,
            _ => unreachable!(),
        }
    }
}

#[derive(Default)]
pub struct ArithmeticOpResult {
    pub result: u32,
    pub carry: bool,
    pub overflow: bool,
    pub sign: bool,
    pub zero: bool,
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

    #[test]
    fn test_conversion_thumb_alu_op() {
        let op: ThumbModeAluInstruction = 0b0000.into();
        assert_eq!(op, ThumbModeAluInstruction::And);
        let op: ThumbModeAluInstruction = 0b0001.into();
        assert_eq!(op, ThumbModeAluInstruction::Eor);
        let op: ThumbModeAluInstruction = 0b1110.into();
        assert_eq!(op, ThumbModeAluInstruction::Bic);
        let op: ThumbModeAluInstruction = 0b1111.into();
        assert_eq!(op, ThumbModeAluInstruction::Mvn);
    }
}
