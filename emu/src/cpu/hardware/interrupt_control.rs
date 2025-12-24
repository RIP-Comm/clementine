//! Interrupt controller registers.
//!
//! The GBA interrupt system allows hardware events to trigger CPU exceptions.
//! Three registers control interrupt behavior:
//!
//! # Interrupt Registers
//!
//! | Register | Address       | Description                                    |
//! |----------|---------------|------------------------------------------------|
//! | IE       | `0x0400_0200` | Interrupt Enable - which IRQs can fire         |
//! | IF       | `0x0400_0202` | Interrupt Request Flags - pending interrupts   |
//! | IME      | `0x0400_0208` | Interrupt Master Enable - global on/off        |
//!
//! # Interrupt Sources
//!
//! Each bit in IE/IF corresponds to an interrupt source:
//!
//! | Bit | Source  | Description                    |
//! |-----|---------|--------------------------------|
//! | 0   | VBlank  | Vertical blank period started  |
//! | 1   | HBlank  | Horizontal blank period        |
//! | 2   | VCount  | Scanline counter match         |
//! | 3-6 | Timer   | Timer 0-3 overflow             |
//! | 7   | Serial  | Serial communication           |
//! | 8-11| DMA     | DMA 0-3 complete               |
//! | 12  | Keypad  | Button combination pressed     |
//! | 13  | GamePak | External cartridge interrupt   |
//!
//! # Interrupt Flow
//!
//! 1. Hardware sets a bit in IF when an event occurs
//! 2. If that bit is also set in IE, and IME is enabled, the CPU takes an IRQ exception
//! 3. The IRQ handler reads IF to determine which interrupt(s) fired
//! 4. Handler writes `1` to IF bits to acknowledge/clear them
//!
//! See [`Bus::is_irq_pending`](crate::bus::Bus::is_irq_pending) for the pending check.

use serde::{Deserialize, Serialize};

/// Interrupt control registers for the GBA.
///
/// These registers are memory-mapped at `0x0400_0200` and accessed through the
/// [`Bus`](crate::bus::Bus).
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
