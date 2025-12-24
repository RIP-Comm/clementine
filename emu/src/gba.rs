//! # GBA System - Top-Level Emulator
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
//!
//! ## Initialization
//!
//! When [`Gba::new`] is called:
//! 1. Creates internal memory with BIOS and cartridge ROM
//! 2. Creates the memory bus connecting CPU to all hardware
//! 3. Creates the ARM7TDMI CPU (starts in Supervisor mode at 0x00000000)
//! 4. CPU executes BIOS, which eventually jumps to ROM at 0x08000000

use std::sync::{Arc, Mutex};

use crate::{
    bus::Bus,
    cartridge_header::CartridgeHeader,
    cpu::{arm7tdmi::Arm7tdmi, hardware::internal_memory::InternalMemory},
    render::gba_lcd::GbaLcd,
};

/// The complete Game Boy Advance system.
///
/// This struct represents the entire GBA hardware:
/// - ARM7TDMI CPU (with bus access to all memory/hardware)
/// - Cartridge header (parsed ROM metadata)
/// - LCD display output
///
/// ## Usage
///
/// ```ignore
/// // Create a new GBA with BIOS and ROM
/// let gba = Gba::new(cartridge_header, bios_data, rom_data);
///
/// // Run the emulator
/// loop {
///     gba.step();  // Execute one CPU instruction
/// }
/// ```
///
/// ## Threading
///
/// The GBA is typically wrapped in `Arc<Mutex<Gba>>` to allow the UI
/// thread and emulation to safely share access.
pub struct Gba {
    pub cpu: Arm7tdmi,

    pub cartridge_header: CartridgeHeader,
    pub lcd: Arc<Mutex<Box<GbaLcd>>>,
}

impl Gba {
    /// Create a new GBA system with the given BIOS and cartridge ROM.
    ///
    /// # Arguments
    ///
    /// * `cartridge_header` - Parsed cartridge header with game metadata
    /// * `bios` - The GBA BIOS ROM (exactly 16KB / 0x4000 bytes)
    /// * `cartridge` - The game ROM data (up to 32MB)
    ///
    /// # Initialization
    ///
    /// This sets up the complete GBA system:
    /// 1. **Memory**: BIOS at 0x00000000, ROM at 0x08000000
    /// 2. **Bus**: Connects CPU to memory and all I/O hardware
    /// 3. **CPU**: ARM7TDMI starting in Supervisor mode
    ///
    /// After creation, the CPU is ready to execute the BIOS boot sequence.
    #[must_use]
    pub fn new(
        cartridge_header: CartridgeHeader,
        bios: [u8; 0x0000_4000],
        cartridge: Vec<u8>,
    ) -> Self {
        let lcd = Arc::new(Mutex::new(Box::default()));
        let memory = InternalMemory::new(bios, cartridge);
        let bus = Bus::with_memory(memory);
        let arm = Arm7tdmi::new(bus);

        Self {
            cpu: arm,
            cartridge_header,
            lcd,
        }
    }

    /// Execute one CPU instruction cycle.
    ///
    /// This advances the emulator by one step:
    /// 1. CPU fetches/decodes/executes one instruction
    /// 2. Hardware components (LCD, timers, DMA) are updated
    /// 3. Interrupts are checked and handled if pending
    ///
    /// Call this in a loop to run the emulator. For real-time emulation,
    /// you'd call this ~16.78 million times per second (GBA clock speed).
    pub fn step(&mut self) {
        self.cpu.step();
    }
}
