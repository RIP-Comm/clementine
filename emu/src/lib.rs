//! # Clementine Emulation Core
//!
//! This crate contains all GBA hardware emulation - no UI code.
//!
//! ## Module Overview
//!
//! | Module              | Description                                    |
//! |---------------------|------------------------------------------------|
//! | [`gba`]             | Top-level GBA system (start here)              |
//! | [`cpu`]             | ARM7TDMI processor and instruction sets        |
//! | [`bus`]             | Memory bus connecting CPU to hardware          |
//! | [`cartridge_header`]| ROM header parsing                             |
//! | [`render`]          | Display output abstractions                    |
//!
//! ## Quick Start
//!
//! ```no_run
//! use emu::{gba::Gba, cartridge_header::CartridgeHeader};
//!
//! let rom = std::fs::read("game.gba").unwrap();
//! let bios = std::fs::read("gba_bios.bin").unwrap();
//! let header = CartridgeHeader::new(&rom).unwrap();
//!
//! let mut gba = Gba::new(header, bios.try_into().unwrap(), rom);
//! loop { gba.step(); }
//! ```
//!
//! ## Architecture
//!
//! See [`gba`] for the system diagram and [`cpu`] for processor details.

#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_wrap)]
mod bitwise;

#[allow(clippy::large_stack_frames)]
pub mod bus;

#[allow(clippy::similar_names)]
pub mod cartridge_header;
pub mod cpu;
pub mod gba;
pub mod render;
