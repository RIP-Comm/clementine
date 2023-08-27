pub mod dma;
pub mod interrupt_control;
pub mod keypad;
pub mod lcd;
pub mod serial;
pub mod sound;
pub mod timers;

pub trait HardwareComponent {
    fn step(&mut self);

    // not sure if it will be useful, let's see
    fn is_interrupt_pending(&self) -> bool;
}
