//! # GBA System
//!
//! This module contains the [`Gba`] struct which represents the entire
//! Game Boy Advance system and ties together all components.
//!
//! ## System Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                        Game Boy Advance System                              │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │  ┌─────────────────────────────────────────────────────────────────────┐   │
//! │  │                         ARM7TDMI CPU                                 │   │
//! │  │   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐               │   │
//! │  │   │Registers│  │  CPSR   │  │Pipeline │  │  ALU    │               │   │
//! │  │   │ R0-R15  │  │  SPSR   │  │ 3-stage │  │Shifter  │               │   │
//! │  │   └─────────┘  └─────────┘  └─────────┘  └─────────┘               │   │
//! │  └────────────────────────────────┬────────────────────────────────────┘   │
//! │                                   │                                        │
//! │                                   ▼                                        │
//! │  ┌─────────────────────────────────────────────────────────────────────┐   │
//! │  │                           Memory Bus                                │   │
//! │  └───┬─────────┬─────────┬─────────┬─────────┬─────────┬─────────┬────┘   │
//! │      │         │         │         │         │         │         │        │
//! │      ▼         ▼         ▼         ▼         ▼         ▼         ▼        │
//! │  ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐   │
//! │  │ BIOS  │ │ EWRAM │ │ IWRAM │ │  I/O  │ │Palette│ │ VRAM  │ │  ROM  │   │
//! │  │ 16KB  │ │ 256KB │ │ 32KB  │ │  Regs │ │  1KB  │ │ 96KB  │ │ 32MB  │   │
//! │  │0x0000 │ │0x0200 │ │0x0300 │ │0x0400 │ │0x0500 │ │0x0600 │ │0x0800 │   │
//! │  └───────┘ └───────┘ └───────┘ └───────┘ └───────┘ └───────┘ └───────┘   │
//! │                          │                                                │
//! │                          ▼                                                │
//! │  ┌─────────────────────────────────────────────────────────────────────┐   │
//! │  │                      I/O Registers (0x04000000)                     │   │
//! │  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐   │   │
//! │  │  │ LCD  │ │Sound │ │ DMA  │ │Timers│ │Serial│ │Keypad│ │  IRQ │   │   │
//! │  │  └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘ └──────┘   │   │
//! │  └─────────────────────────────────────────────────────────────────────┘   │
//! │                                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Memory Map
//!
//! | Address Range       | Size  | Description                    |
//! |---------------------|-------|--------------------------------|
//! | 0x00000000-0x00003FFF | 16KB  | BIOS (read-only, protected)    |
//! | 0x02000000-0x0203FFFF | 256KB | EWRAM (External Work RAM)      |
//! | 0x03000000-0x03007FFF | 32KB  | IWRAM (Internal Work RAM)      |
//! | 0x04000000-0x040003FF | 1KB   | I/O Registers                  |
//! | 0x05000000-0x050003FF | 1KB   | Palette RAM                    |
//! | 0x06000000-0x06017FFF | 96KB  | VRAM (Video RAM)               |
//! | 0x07000000-0x070003FF | 1KB   | OAM (Object Attribute Memory)  |
//! | 0x08000000-0x09FFFFFF | 32MB  | ROM (Game Pak)                 |

use std::sync::{Arc, Mutex};

use crate::{
    bus::Bus,
    cartridge_header::CartridgeHeader,
    cpu::{
        DISASM_BUFFER_CAPACITY, DisasmEntry, arm7tdmi::Arm7tdmi,
        hardware::internal_memory::InternalMemory,
    },
    render::gba_lcd::GbaLcd,
};

/// The complete Game Boy Advance system.
pub struct Gba {
    pub cpu: Arm7tdmi,

    /// Parsed cartridge header with game metadata.
    pub cartridge_header: CartridgeHeader,

    pub lcd: Arc<Mutex<Box<GbaLcd>>>,

    /// Consumer for the lock-free disassembler channel.
    pub disasm_rx: Option<rtrb::Consumer<DisasmEntry>>,
}

impl Gba {
    /// Create a new GBA system with the given BIOS and cartridge ROM.
    /// After creation, the CPU is ready to execute the BIOS boot sequence.
    #[must_use]
    pub fn new(bios: [u8; 0x0000_4000], cartridge: &[u8]) -> Self {
        let cartridge_header = CartridgeHeader::new(cartridge);

        let lcd = Arc::new(Mutex::new(Box::default()));
        let memory = InternalMemory::new(bios, cartridge);
        let bus = Bus::with_memory(memory);
        let mut arm = Arm7tdmi::new(bus);

        // avoid to block execution for disassembler
        let (tx, rx) = rtrb::RingBuffer::new(DISASM_BUFFER_CAPACITY);
        arm.disasm_tx = Some(tx);

        Self {
            cpu: arm,
            cartridge_header,
            lcd,
            disasm_rx: Some(rx),
        }
    }

    /// Execute one CPU instruction cycle.
    /// Returns `true` if `VBlank` just started (a new frame is ready to display).
    /// Call this in a loop to run the emulator.
    /// For real-time emulation, you'd call this ~16.78 million times per second (GBA clock speed).
    pub fn step(&mut self) -> bool {
        self.cpu.step()
    }
}
