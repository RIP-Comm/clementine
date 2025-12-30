//! # Clementine Emulation Core
//!
//! This crate contains all GBA hardware emulation, no UI code.

mod bitwise;
pub mod bus;
pub mod cartridge_header;
pub mod cpu;
pub mod gba;
pub mod render;
pub mod ring_buffer;
