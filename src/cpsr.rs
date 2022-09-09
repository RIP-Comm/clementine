use crate::condition::Condition;

/// Current Program Status Register.
#[derive(Default)]
pub(crate) struct Cpsr(u32);

impl Cpsr {
    pub(crate) fn can_execute(&self, cond: Condition) -> bool {
        use Condition::*;
        match cond {
            GE => self.signed() == self.overflow(),
            AL => true,
            _ => todo!(),
        }
    }

    fn signed(&self) -> bool {
        self.0 & 0x80000000 != 0
    }

    pub(crate) fn set_signed(&mut self) {
        self.0 |= 0b1000_0000_0000_0000_0000_0000_0000_0000;
    }

    pub(crate) fn set_not_signed(&mut self) {
        self.0 &= 0b0111_1111_1111_1111_1111_1111_1111_1111;
    }

    fn overflow(&self) -> bool {
        self.0 & 0x10000000 != 0
    }
}

#[cfg(test)]
mod tests {
    use crate::cpsr::Cpsr;

    #[test]
    fn check_sign_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_signed();
        assert!(cpsr.signed());
    }

    #[test]
    fn check_overflow_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.0 = 0b0001_0000_0000_0000_0000_0000_0000_0000;
        assert!(cpsr.overflow());
    }
}
