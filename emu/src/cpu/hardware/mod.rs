//! GBA hardware component implementations.
//!
//! This module contains all the hardware peripherals that connect to the CPU via the
//! [`Bus`](crate::bus::Bus). Each component is memory-mapped to specific address ranges
//! in the GBA's address space.
//!
//! # Hardware Components
//!
//! | Module                | Description                              | I/O Address Range     |
//! |-----------------------|------------------------------------------|-----------------------|
//! | [`internal_memory`]   | BIOS, RAM, ROM, and Flash storage        | Various (see module)  |
//! | [`lcd`]               | LCD controller and PPU                   | `0x0400_0000-005F`    |
//! | [`sound`]             | Sound channels and mixer                 | `0x0400_0060-00AF`    |
//! | [`dma`]               | 4-channel DMA controller                 | `0x0400_00B0-00FF`    |
//! | [`timers`]            | 4 hardware timers                        | `0x0400_0100-011F`    |
//! | [`serial`]            | Serial communication (Link Cable, etc.) | `0x0400_0120-015F`    |
//! | [`keypad`]            | Button input and interrupts              | `0x0400_0130-0133`    |
//! | [`interrupt_control`] | Interrupt enable/request/master enable   | `0x0400_0200-0301`    |
//!
//! # Address Mirroring
//!
//! The [`get_unmasked_address`] function handles address mirroring for memory regions
//! that repeat throughout their address space. This is how the GBA hardware works -
//! accessing `0x0500_0400` is the same as accessing `0x0500_0000` in palette RAM.

pub mod dma;
pub mod internal_memory;
pub mod interrupt_control;
pub mod keypad;

pub mod lcd;
pub mod serial;
pub mod sound;
pub mod timers;

#[must_use]
pub const fn get_unmasked_address(
    address: usize,
    mask_get: usize,
    mask_set: usize,
    mask_shift: usize,
    modulo: usize,
) -> usize {
    // Get the index of the mirror
    let idx = (address & mask_get) >> mask_shift;
    // Remove the mirror index from the address
    let mut address = address & mask_set;
    // Insert the unmasked index in the address
    address |= (idx % modulo) << mask_shift;

    address
}
