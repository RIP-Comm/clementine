use crate::condition::Condition;

/// Current Program Status Register.
#[derive(Default)]
pub(crate) struct CPSR(u32);

impl CPSR {
    pub(crate) fn can_execute(&self, cond: Condition) -> bool {
        match cond {
            Condition::GE => self.signed() == self.overflow(),
            _ => todo!(),
        }
    }

    fn signed(&self) -> bool {
        self.0 & 0x8000 != 0
    }

    fn overflow(&self) -> bool {
        self.0 & 0x1000 != 0
    }
}
