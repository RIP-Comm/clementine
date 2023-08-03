pub mod lcd;
pub mod sound;

pub trait HardwareComponent {
    fn step(&mut self);

    // not sure if it will be useful, let's see
    fn is_interrupt_pending(&self) -> bool;
}
