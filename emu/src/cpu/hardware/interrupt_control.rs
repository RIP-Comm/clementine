use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct InterruptControl {
    pub interrupt_enable: u16,
    /// Interrupt Request Flags (IF), bits are set when interrupts are requested,
    /// cleared by writing 1 to the corresponding bit
    pub interrupt_request: u16,
    pub wait_state_control: u16,
    pub interrupt_master_enable: u16,
    pub post_boot_flag: u8,
    pub power_down_control: u8,
    pub purpose_unknown: u8,
    pub internal_memory_control: u32,
}
