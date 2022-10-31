#[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
use crate::condition::ModeBits;
use crate::{bitwise::Bits, condition::Condition, data_processing::ArithmeticOpResult};

/// Current Program Status Register.
#[derive(Default)]
pub struct Cpsr(u32);

impl Cpsr {
    pub(crate) fn can_execute(&self, cond: Condition) -> bool {
        use Condition::*;
        match cond {
            EQ => self.zero_flag(),                         // Equal (Z=1)
            NE => !self.zero_flag(),                        // Not equal (Z=0)
            CS => self.carry_flag(),                        // Unsigned higher or same (C=1)
            CC => !self.carry_flag(),                       // Unsigned lower (C=0)
            MI => self.sign_flag(),                         // Negative (N=1)
            PL => !self.sign_flag(),                        // Positive or zero (N=0)
            VS => self.overflow_flag(),                     // Overflow (V=1)
            VC => !self.overflow_flag(),                    // No overflow (V=0)
            HI => self.carry_flag() && !self.zero_flag(),   // Unsigned higher (C=1 and Z=0)
            LS => !self.carry_flag() || self.zero_flag(),   // Unsigned lower or same (C=0 and Z=1)
            GE => self.sign_flag() == self.overflow_flag(), // Greater or equal (N=V)
            LT => self.sign_flag() != self.overflow_flag(), // Less than (N<>V)
            GT => !self.zero_flag() && (self.sign_flag() == self.overflow_flag()), // Greater than (Z=0 and N=V)
            LE => self.zero_flag() && (self.sign_flag() != self.overflow_flag()), // Less or equal (Z=1 or N<>V)
            AL => true,  // Always (the "AL" suffix can be omitted)
            NV => false, // Never (ARMv1, v2 only) (Reserved ARMv3 and up)
        }
    }

    /// N => Bit 31, (0=Not Signed, 1=Signed)
    pub fn sign_flag(&self) -> bool {
        self.0.get_bit(31)
    }

    /// Z => Bit 30, (0=Not Zero, 1=Zero)
    pub fn zero_flag(&self) -> bool {
        self.0.get_bit(30)
    }

    /// C => Bit 29, (0=Borrow/No Carry, 1=Carry/No Borrow)
    pub fn carry_flag(&self) -> bool {
        self.0.get_bit(29)
    }

    /// V => Bit 28, (0=No Overflow, 1=Overflow)
    pub fn overflow_flag(&self) -> bool {
        self.0.get_bit(28)
    }

    /// Q => Bit 27, (1=Sticky Overflow, ARMv5TE and up only)
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn sticky_overflow(&self) -> bool {
        self.0.get_bit(27)
    }

    /// Reserved => Bits 26-8, (For future use) - Do not change manually!
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn reserved_bits(&self) -> bool {
        // These bits are reserved for possible future implementations.
        // For best forwards compatibility, the user should never change the state of these bits,
        // and should not expect these bits to be set to a specific value.
        true
    }

    /// I => Bit 7, (0=Enable, 1=Disable)
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn irq_disable(&self) -> bool {
        self.0.get_bit(7)
    }

    /// F => Bit 6, (0=Enable, 1=Disable)
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn fiq_disable(&self) -> bool {
        self.0.get_bit(6)
    }

    /// T => Bit 5, (0=ARM, 1=THUMB) - Do not change manually!
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn state_bit(&self) -> bool {
        self.0.get_bit(5)
    }

    /// M4-M0 => Bits 4-0
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn mode_bits(&self) -> bool {
        self.0 == 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0000_0001 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0000_0010 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0000_0011 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_0000 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_0001 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_0010 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_0011 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_0111 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_1011 != 0
            || self.0 & 0b0000_0000_0000_0000_0000_0000_0001_1111 != 0
    }

    pub fn set_sign_flag(&mut self, value: bool) {
        self.0.set_bit(31, value);
    }

    pub fn set_zero_flag(&mut self, value: bool) {
        self.0.set_bit(30, value);
    }

    pub fn set_carry_flag(&mut self, value: bool) {
        self.0.set_bit(29, value);
    }

    pub fn set_flags(&mut self, op_result: ArithmeticOpResult) {
        self.set_carry_flag(op_result.carry);
        self.set_zero_flag(op_result.zero);
        self.set_sign_flag(op_result.sign);
        self.set_overflow_flag(op_result.overflow);
    }

    #[allow(dead_code)] // TODO: remove allow when this API will be used at least one in prod code.
    pub fn set_overflow_flag(&mut self, value: bool) {
        self.0.set_bit(28, value);
    }

    /// Used by QADD, QSUB, QDADD, QDSUB, SMLAxy, and SMLAWy only.
    /// The Q-flag can be tested/reset by MSR/MRS opcodes only.
    /// These opcodes set the Q-flag in case of overflows, but leave it unchanged otherwise.
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_sticky_overflow(&mut self, value: bool) {
        // TODO (value is true): Should we check the opcode is one of these (QADD, QSUB, QDADD, QDSUB, SMLAxy, and SMLAWy)?
        // TODO (value is false): Should we check the opcode is one of these (MSR/MRS)?
        self.0.set_bit(27, value);
    }

    /// These bits [7-0] below may change when an exception occurs.
    /// In privileged modes (non-user modes) they may be also changed manually.

    /// The interrupt bit I is used to disable/enable IRQ interrupts respectively (1 means disabled and 0 means enabled).
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_irq_disable(&mut self, value: bool) {
        // TODO: Should we check we are in privileged modes or it occurred an exeption?
        self.0.set_bit(7, value);
    }

    /// The interrupt bit F is used to disable/enable FIQ interrupts respectively (1 means disabled and 0 means enabled).
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_fiq_disable(&mut self, value: bool) {
        // TODO: Should we check we are in privileged modes or it occurred an exeption?
        self.0.set_bit(6, value);
    }

    /// The T Bit is used to set the current state of the CPU on ARM/THUMB mode (1 means ARM and 0 means THUMB).
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_state_bit(&mut self, value: bool) {
        // TODO: Must be changeed by BX instructions
        self.0.set_bit(5, value);
    }

    /// The Mode Bits M4-M0 contain the current operating mode.
    #[cfg(test)] // TODO: remove cfg when this API will be used at least one in prod code.
    pub fn set_mode_bits(&mut self, control_bits: ModeBits) {
        match control_bits {
            ModeBits::OldUser => self.0 |= 0b0000_0000_0000_0000_0000_0000_0000_0000,
            ModeBits::OldFiq => self.0 |= 0b0000_0000_0000_0000_0000_0000_0000_0001,
            ModeBits::OldIrq => self.0 |= 0b0000_0000_0000_0000_0000_0000_0000_0010,
            ModeBits::OldSupervisor => self.0 |= 0b0000_0000_0000_0000_0000_0000_0000_0011,
            ModeBits::User => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0000,
            ModeBits::Fiq => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0001,
            ModeBits::Irq => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0010,
            ModeBits::Supervisor => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0011,
            ModeBits::Abort => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0111,
            ModeBits::Undefined => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_1011,
            ModeBits::System => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_1111,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_sign_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_sign_flag(true);
        assert!(cpsr.sign_flag());
    }

    #[test]
    fn check_zero_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_zero_flag(true);
        assert!(cpsr.zero_flag());
    }

    #[test]
    fn check_carry_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_carry_flag(true);
        assert!(cpsr.carry_flag());
    }

    #[test]
    fn check_overflow_flag() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.0 = 0b0001_0000_0000_0000_0000_0000_0000_0000;
        assert!(cpsr.overflow_flag());
    }

    #[test]
    fn check_sticky_overflow() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_sticky_overflow(true);
        assert!(cpsr.sticky_overflow());
    }

    #[test]
    fn check_reserved_bits() {
        let cpsr: Cpsr = Cpsr(0);
        assert!(cpsr.reserved_bits());
    }

    #[test]
    fn check_irq_disable() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_irq_disable(true);
        assert!(cpsr.irq_disable());
    }

    #[test]
    fn check_fiq_disable() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_fiq_disable(true);
        assert!(cpsr.fiq_disable());
    }

    #[test]
    fn check_state_bit() {
        let mut cpsr: Cpsr = Cpsr(0);
        cpsr.set_state_bit(true);
        assert!(cpsr.state_bit());
    }

    #[test]
    fn check_old_user() {
        let mut cpsr: Cpsr = Cpsr(0);
        let mode_bits = ModeBits::OldUser;
        cpsr.set_mode_bits(mode_bits);
        assert!(cpsr.mode_bits())
    }

    #[test]
    fn check_old_fiq() {
        let mut cpsr: Cpsr = Cpsr(0);
        let mode_bits = ModeBits::OldFiq;
        cpsr.set_mode_bits(mode_bits);
        assert!(cpsr.mode_bits())
    }

    #[test]
    fn check_user() {
        let mut cpsr: Cpsr = Cpsr(0);
        let mode_bits = ModeBits::User;
        cpsr.set_mode_bits(mode_bits);
        assert!(cpsr.mode_bits())
    }

    #[test]
    fn check_supervisor() {
        let mut cpsr: Cpsr = Cpsr(0);
        let mode_bits = ModeBits::Supervisor;
        cpsr.set_mode_bits(mode_bits);
        assert!(cpsr.mode_bits())
    }
}
