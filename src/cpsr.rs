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
        self.0 & 0x8000 != 0
    }

    fn overflow(&self) -> bool {
        self.0 & 0x1000 != 0
    }
}
