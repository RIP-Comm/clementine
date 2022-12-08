use crate::cpu::psr::Psr;

#[derive(Default)]
pub struct RegisterBank {
    pub r8_fiq: u32,
    pub r9_fiq: u32,
    pub r10_fiq: u32,
    pub r11_fiq: u32,
    pub r12_fiq: u32,
    pub r13_fiq: u32,
    pub r14_fiq: u32,
    pub r13_svc: u32,
    pub r14_svc: u32,
    pub r13_abt: u32,
    pub r14_abt: u32,
    pub r13_irq: u32,
    pub r14_irq: u32,
    pub r13_und: u32,
    pub r14_und: u32,
    pub spsr_fiq: Psr,
    pub spsr_svc: Psr,
    pub spsr_abt: Psr,
    pub spsr_irq: Psr,
    pub spsr_und: Psr,
}
