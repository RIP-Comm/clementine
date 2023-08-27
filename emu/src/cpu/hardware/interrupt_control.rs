use vecfixed::VecFixed;

pub struct InterruptControl {
    pub interrupt_enable: u16,
    // It is a ring buffer since when we write to this register, the value will reach the CPU
    // after 4 cycles (source 3.10 Interrupt Latencies in ARM datasheet).
    // When we write a value to this address we write it in the back of the ring buffer.
    // When we read the value from this address we read it from the front of the ring buffer.
    // Every bus step this ring should be "advanced": peeking the back and pushing a copy of it.
    pub interrupt_request: VecFixed<5, u16>,
    pub wait_state_control: u16,
    pub interrupt_master_enable: u16,
    pub post_boot_flag: u8,
    pub power_down_control: u8,
    pub purpose_unknown: u8,
    pub internal_memory_control: u32,
}

impl Default for InterruptControl {
    fn default() -> Self {
        Self {
            interrupt_enable: 0,
            interrupt_request: VecFixed::initialize(0),
            wait_state_control: 0,
            interrupt_master_enable: 0,
            post_boot_flag: 0,
            power_down_control: 0,
            purpose_unknown: 0,
            internal_memory_control: 0,
        }
    }
}
