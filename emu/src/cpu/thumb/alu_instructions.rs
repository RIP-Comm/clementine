#[derive(Debug, Eq, PartialEq, Copy, Clone)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbHighRegisterOperation {
    Add,
    Cmp,
    Mov,
    BxOrBlx,
}

impl std::fmt::Display for ThumbHighRegisterOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mov => f.write_str("MOV"),
            Self::Cmp => f.write_str("CMP"),
            Self::Add => f.write_str("ADD"),
            Self::BxOrBlx => f.write_str("BX"),
        }
    }
}

impl From<u16> for ThumbHighRegisterOperation {
    fn from(op: u16) -> Self {
        match op {
            0 => Self::Add,
            1 => Self::Cmp,
            2 => Self::Mov,
            3 => Self::BxOrBlx,
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
