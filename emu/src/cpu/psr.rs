use crate::bitwise::Bits;
use crate::cpu::arm::alu_instruction::ArithmeticOpResult;
use crate::cpu::{condition::Condition, cpu_modes::Mode};

/// Program Status Register.
#[derive(Default, Clone, Copy)]
pub struct Psr(u32);

impl Psr {
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
            LE => self.zero_flag() || (self.sign_flag() != self.overflow_flag()), // Less or equal (Z=1 or N<>V)
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
    pub fn sticky_overflow(&self) -> bool {
        self.0.get_bit(27)
    }

    /// Reserved => Bits 26-8, (For future use) - Do not change manually!
    pub const fn reserved_bits(&self) -> bool {
        // These bits are reserved for possible future implementations.
        // For best forwards compatibility, the user should never change the state of these bits,
        // and should not expect these bits to be set to a specific value.
        true
    }

    /// I => Bit 7, (0=Enable, 1=Disable)
    pub fn irq_disable(&self) -> bool {
        self.0.get_bit(7)
    }

    /// F => Bit 6, (0=Enable, 1=Disable)
    pub fn fiq_disable(&self) -> bool {
        self.0.get_bit(6)
    }

    /// T => Bit 5, (0=ARM, 1=THUMB) - Do not change manually!
    pub fn state_bit(&self) -> bool {
        self.0.get_bit(5)
    }

    /// M4-M0 => Bits 4-0
    pub fn mode(&self) -> Mode {
        Mode::try_from(self.0 & 0b11111).unwrap()
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
    pub fn set_irq_disable(&mut self, value: bool) {
        // TODO: Should we check we are in privileged modes or it occurred an exeption?
        self.0.set_bit(7, value);
    }

    /// The interrupt bit F is used to disable/enable FIQ interrupts respectively (1 means disabled and 0 means enabled).
    pub fn set_fiq_disable(&mut self, value: bool) {
        // TODO: Should we check we are in privileged modes or it occurred an exeption?
        self.0.set_bit(6, value);
    }

    /// The T Bit is used to set the current state of the CPU on ARM/THUMB mode (1 means ARM and 0 means THUMB).
    pub fn set_state_bit(&mut self, value: bool) {
        self.0.set_bit(5, value);
    }

    pub fn set_mode_raw(&mut self, m: u32) {
        self.0 &= 0b1111_1111_1111_1111_1111_1111_1110_0000;

        let mode_raw = m & 0b0001_1111;

        self.0 |= mode_raw;
    }

    /// The Mode Bits M4-M0 contain the current operating mode.
    pub fn set_mode(&mut self, m: Mode) {
        // Setting mode bits to 0
        self.0 &= 0b1111_1111_1111_1111_1111_1111_1110_0000;

        // Setting mode bits according to the chosen mode
        match m {
            Mode::User => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0000,
            Mode::Fiq => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0001,
            Mode::Irq => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0010,
            Mode::Supervisor => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0011,
            Mode::Abort => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_0111,
            Mode::Undefined => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_1011,
            Mode::System => self.0 |= 0b0000_0000_0000_0000_0000_0000_0001_1111,
        }
    }

    pub fn cpu_state(&self) -> CpuState {
        self.state_bit().into()
    }

    pub fn set_cpu_state(&mut self, state: CpuState) {
        self.set_state_bit(state.into());
    }
}

impl From<Mode> for Psr {
    fn from(m: Mode) -> Self {
        let mut s = Self(0);

        s.set_mode(m);

        s
    }
}

impl From<Psr> for u32 {
    fn from(p: Psr) -> Self {
        p.0
    }
}

/// Represents the CPU state (ARM/THUMB).
pub enum CpuState {
    /// Which operates with 16-bit, halfword-aligned THUMB instructions.
    /// In this state, the PC uses bit 1 to select between alternate halfwords.
    Thumb,

    /// Which executes 32-bit, word-aligned ARM instructions.
    Arm,
}

impl From<CpuState> for bool {
    fn from(state: CpuState) -> Self {
        match state {
            CpuState::Arm => false,
            CpuState::Thumb => true,
        }
    }
}

impl From<bool> for CpuState {
    fn from(state: bool) -> Self {
        match state {
            true => Self::Thumb,
            false => Self::Arm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_sign_flag() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_sign_flag(true);
        assert!(cpsr.sign_flag());
    }

    #[test]
    fn check_zero_flag() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_zero_flag(true);
        assert!(cpsr.zero_flag());
    }

    #[test]
    fn check_carry_flag() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_carry_flag(true);
        assert!(cpsr.carry_flag());
    }

    #[test]
    fn check_overflow_flag() {
        let mut cpsr: Psr = Psr(0);
        cpsr.0 = 0b0001_0000_0000_0000_0000_0000_0000_0000;
        assert!(cpsr.overflow_flag());
    }

    #[test]
    fn check_sticky_overflow() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_sticky_overflow(true);
        assert!(cpsr.sticky_overflow());
    }

    #[test]
    fn check_reserved_bits() {
        let cpsr: Psr = Psr(0);
        assert!(cpsr.reserved_bits());
    }

    #[test]
    fn check_irq_disable() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_irq_disable(true);
        assert!(cpsr.irq_disable());
    }

    #[test]
    fn check_fiq_disable() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_fiq_disable(true);
        assert!(cpsr.fiq_disable());
    }

    #[test]
    fn check_state_bit() {
        let mut cpsr: Psr = Psr(0);
        cpsr.set_state_bit(true);
        assert!(cpsr.state_bit());
    }

    #[test]
    fn check_user() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::User;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b10000);

        let cpsr = Psr(0b10000);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::User);
    }

    #[test]
    fn check_fiq() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::Fiq;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b10001);

        let cpsr = Psr(0b10001);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::Fiq);
    }

    #[test]
    fn check_irq() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::Irq;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b10010);

        let cpsr = Psr(0b10010);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::Irq);
    }

    #[test]
    fn check_supervisor() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::Supervisor;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b10011);

        let cpsr = Psr(0b10011);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::Supervisor);
    }

    #[test]
    fn check_abort() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::Abort;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b10111);

        let cpsr = Psr(0b10111);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::Abort);
    }

    #[test]
    fn check_undefined() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::Undefined;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b11011);

        let cpsr = Psr(0b11011);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::Undefined);
    }

    #[test]
    fn check_system() {
        let mut cpsr: Psr = Psr(0);
        let mode = Mode::System;
        cpsr.set_mode(mode);
        assert_eq!(cpsr.0 & 0b11111, 0b11111);

        let cpsr = Psr(0b11111);
        let mode = cpsr.mode();

        assert_eq!(mode, Mode::System);
    }
}
