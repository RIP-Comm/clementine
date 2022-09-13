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

impl From<u8> for ArmModeAluInstruction {
    fn from(alu_opcode: u8) -> Self {
        use ArmModeAluInstruction::*;
        match alu_opcode {
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
