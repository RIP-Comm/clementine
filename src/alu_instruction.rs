pub(crate) enum ArmModeAluInstruction {
    AND = 0x0,
    EOR = 0x1,
    SUB = 0x2,
    RSB = 0x3,
    ADD = 0x4,
    ADC = 0x5,
    SBC = 0x6,
    RSC = 0x7,
    TST = 0x8,
    TEQ = 0x9,
    CMP = 0xA,
    CMN = 0xB,
    ORR = 0xC,
    MOV = 0xD,
    BIC = 0xE,
    MVN = 0xF,
}

impl From<u8> for ArmModeAluInstruction {
    fn from(alu_opcode: u8) -> Self {
        use ArmModeAluInstruction::*;
        match alu_opcode {
            0x0 => AND,
            0x1 => EOR,
            0x2 => SUB,
            0x3 => RSB,
            0x4 => ADD,
            0x5 => ADC,
            0x6 => SBC,
            0x7 => RSC,
            0x8 => TST,
            0x9 => TEQ,
            0xA => CMP,
            0xB => CMN,
            0xC => ORR,
            0xD => MOV,
            0xE => BIC,
            0xF => MVN,
            _ => unreachable!(),
        }
    }
}
