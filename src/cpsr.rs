use crate::{bitwise::Bits, condition::Condition};

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

    pub(crate) fn signed(&self) -> bool {
        self.0.get_bit(31)
    }

    pub(crate) fn set_signed(&mut self, value: bool) {
        self.0.set_bit(31, value)
    }

    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub(crate) fn zero_flag(&self) -> bool {
        self.0.get_bit(30)
    }

    pub(crate) fn set_zero_flag(&mut self, value: bool) {
        self.0.set_bit(30, value)
    }

    fn overflow(&self) -> bool {
        self.0 & 0b0001_0000_0000_0000_0000_0000_0000_0000 != 0
    }
}

#[cfg(test)]
mod tests {
    use crate::cpsr::Cpsr;

    #[test]
    fn check_sign_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_signed(true);
        assert!(cpsr.signed());
    }

    #[test]
    fn check_overflow_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.0 = 0b0001_0000_0000_0000_0000_0000_0000_0000;
        assert!(cpsr.overflow());
    }
}
