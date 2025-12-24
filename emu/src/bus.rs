//! Memory bus connecting the CPU to all hardware components.
//!
//! The [`Bus`] is the central hub through which the ARM7TDMI CPU accesses all memory
//! and I/O registers. It implements address decoding to route reads and writes to the
//! appropriate hardware component.
//!
//! # Memory Map Overview
//!
//! See [`gba`](crate::gba) for the complete GBA memory map. The bus routes addresses:
//!
//! | Address Range       | Component                           | Handler               |
//! |---------------------|-------------------------------------|-----------------------|
//! | `0x0000_0000-3FFF`  | BIOS (with read protection)         | [`InternalMemory`]    |
//! | `0x0200_0000-3FFF`  | Work RAM (256KB, mirrored)          | [`InternalMemory`]    |
//! | `0x0300_0000-7FFF`  | Internal RAM (32KB, mirrored)       | [`InternalMemory`]    |
//! | `0x0400_0000-005F`  | LCD I/O registers                   | [`Lcd`]               |
//! | `0x0400_0060-00AF`  | Sound registers                     | [`Sound`]             |
//! | `0x0400_00B0-00FF`  | DMA registers                       | [`Dma`]               |
//! | `0x0400_0100-011F`  | Timer registers                     | [`Timers`]            |
//! | `0x0400_0120-01FF`  | Serial/Keypad registers             | [`Serial`]/[`Keypad`] |
//! | `0x0400_0200-FFFF`  | Interrupt control                   | [`InterruptControl`]  |
//! | `0x0500_0000-03FF`  | Palette RAM (1KB, mirrored)         | [`Lcd`] memory        |
//! | `0x0600_0000-17FFF` | VRAM (96KB, mirrored)               | [`Lcd`] memory        |
//! | `0x0700_0000-03FF`  | OAM (1KB, mirrored)                 | [`Lcd`] memory        |
//! | `0x0800_0000+`      | Game Pak ROM/Flash                  | [`InternalMemory`]    |
//!
//! # Memory Access Sizes
//!
//! The bus supports three access sizes, each with alignment requirements:
//! - **Byte** (8-bit): Any address
//! - **Halfword** (16-bit): Must be 2-byte aligned (address & 1 == 0)
//! - **Word** (32-bit): Must be 4-byte aligned (address & 3 == 0)
//!
//! Unaligned accesses are force-aligned with a warning logged.
//!
//! # Special Behaviors
//!
//! ## BIOS Read Protection
//! The BIOS can only be read when the program counter is within the BIOS region
//! (`0x0000-0x3FFF`). Reads from outside return the last fetched BIOS opcode.
//!
//! ## Video Memory Write Restrictions
//! - **OAM**: Byte writes are ignored (must use halfword/word)
//! - **VRAM**: Byte writes are duplicated to both bytes of a halfword
//! - **Palette RAM**: Byte writes are duplicated to both bytes of a halfword
//!
//! ## Interrupt Acknowledge
//! Writing `1` to a bit in the Interrupt Request Flags register (`0x0400_0202`)
//! clears that interrupt flag (acknowledges it).
//!
//! # Timing
//!
//! The bus tracks cycle counts for memory accesses. Different memory regions have
//! different wait states, though currently a simplified 1-cycle model is used.
//! The [`step`](Bus::step) method advances timers and LCD state each CPU cycle.
//!
//! [`InternalMemory`]: crate::cpu::hardware::internal_memory::InternalMemory
//! [`Lcd`]: crate::cpu::hardware::lcd::Lcd
//! [`Sound`]: crate::cpu::hardware::sound::Sound
//! [`Dma`]: crate::cpu::hardware::dma::Dma
//! [`Timers`]: crate::cpu::hardware::timers::Timers
//! [`Serial`]: crate::cpu::hardware::serial::Serial
//! [`Keypad`]: crate::cpu::hardware::keypad::Keypad
//! [`InterruptControl`]: crate::cpu::hardware::interrupt_control::InterruptControl

use std::collections::HashMap;

use logger::log;
use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;
use crate::cpu::hardware::dma::{Dma, Registers};
use crate::cpu::hardware::get_unmasked_address;
use crate::cpu::hardware::internal_memory::InternalMemory;
use crate::cpu::hardware::interrupt_control::InterruptControl;
use crate::cpu::hardware::keypad::Keypad;
use crate::cpu::hardware::lcd::Lcd;
use crate::cpu::hardware::serial::Serial;
use crate::cpu::hardware::sound::Sound;
use crate::cpu::hardware::timers::Timers;

#[derive(Default, Serialize, Deserialize)]
pub struct Bus {
    pub internal_memory: InternalMemory,
    pub lcd: Lcd,
    sound: Sound,
    dma: Dma,
    timers: Timers,
    serial: Serial,
    keypad: Keypad,
    interrupt_control: InterruptControl,
    cycles_count: u128,
    last_used_address: usize,
    unused_region: HashMap<usize, u8>,
    /// Tracks the last opcode fetched from BIOS for read protection
    last_bios_opcode: u32,
    /// Tracks the current program counter
    current_pc: usize,
}

#[allow(dead_code)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) enum IrqType {
    VBlank,
    HBlank,
    VCount,
    Timer0,
    Timer1,
    Timer2,
    Timer3,
    Serial,
    Dma0,
    Dma1,
    Dma2,
    Dma3,
    Keypad,
    Gamepak,
}

impl IrqType {
    /// Returns the index of the corresponding `IrqType` inside the Interrupt Request Flag register
    const fn get_idx_in_if(self) -> u8 {
        match self {
            Self::VBlank => 0,
            Self::HBlank => 1,
            Self::VCount => 2,
            Self::Timer0 => 3,
            Self::Timer1 => 4,
            Self::Timer2 => 5,
            Self::Timer3 => 6,
            Self::Serial => 7,
            Self::Dma0 => 8,
            Self::Dma1 => 9,
            Self::Dma2 => 10,
            Self::Dma3 => 11,
            Self::Keypad => 12,
            Self::Gamepak => 13,
        }
    }
}
impl Bus {
    fn read_interrupt_control_raw(&self, address: usize) -> u8 {
        match address {
            0x0400_0200 => self.interrupt_control.interrupt_enable.get_byte(0),
            0x0400_0201 => self.interrupt_control.interrupt_enable.get_byte(1),
            0x0400_0202 => self.interrupt_control.interrupt_request.get_byte(0),
            0x0400_0203 => self.interrupt_control.interrupt_request.get_byte(1),
            0x0400_0204 => self.interrupt_control.wait_state_control.get_byte(0),
            0x0400_0205 => self.interrupt_control.wait_state_control.get_byte(1),
            0x0400_0208 => self.interrupt_control.interrupt_master_enable.get_byte(0),
            0x0400_0209 => self.interrupt_control.interrupt_master_enable.get_byte(1),
            0x0400_0300 => self.interrupt_control.post_boot_flag.get_byte(0),
            0x0400_0301 => self.interrupt_control.power_down_control.get_byte(0),
            0x0400_0410 => self.interrupt_control.purpose_unknown.get_byte(0),
            0x0400_0206
            | 0x0400_0207
            | 0x400_020A..=0x400_02FF
            | 0x0400_0302..=0x0400_040F
            | 0x0400_0411 => {
                log(format!("read on unused memory 0x{address:08X}"));
                *self.unused_region.get(&address).unwrap_or(&0)
            }
            _ => match address & 0b111 {
                0x800 => self.interrupt_control.internal_memory_control.get_byte(0),
                0x801 => self.interrupt_control.internal_memory_control.get_byte(1),
                0x802 => self.interrupt_control.internal_memory_control.get_byte(2),
                0x803 => self.interrupt_control.internal_memory_control.get_byte(3),
                _ => {
                    log(format!("read on unused memory 0x{address:08X}"));
                    *self.unused_region.get(&address).unwrap_or(&0)
                }
            },
        }
    }

    fn write_interrupt_control_raw(&mut self, address: usize, value: u8) {
        match address {
            0x0400_0200 => self.interrupt_control.interrupt_enable.set_byte(0, value),
            0x0400_0201 => self.interrupt_control.interrupt_enable.set_byte(1, value),
            0x0400_0202 => {
                // Writing 1 to a bit clears it (acknowledges the interrupt)
                self.interrupt_control.interrupt_request &= !(value as u16);
            }
            0x0400_0203 => {
                // Writing 1 to a bit clears it (acknowledges the interrupt)
                self.interrupt_control.interrupt_request &= !((value as u16) << 8);
            }
            0x0400_0204 => self.interrupt_control.wait_state_control.set_byte(0, value),
            0x0400_0205 => self.interrupt_control.wait_state_control.set_byte(1, value),
            0x0400_0208 => {
                self.interrupt_control
                    .interrupt_master_enable
                    .set_byte(0, value);
            }
            0x0400_0209 => {
                self.interrupt_control
                    .interrupt_master_enable
                    .set_byte(1, value);
            }
            0x0400_0300 => self.interrupt_control.post_boot_flag.set_byte(0, value),
            0x0400_0301 => self.interrupt_control.power_down_control.set_byte(0, value),
            0x0400_0410 => self.interrupt_control.purpose_unknown.set_byte(0, value),
            0x0400_0206
            | 0x0400_0207
            | 0x0400_020A..=0x0400_02FF
            | 0x0400_0302..=0x0400_040F
            | 0x0400_0411 => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            _ => match address & 0b111 {
                0x800 => self
                    .interrupt_control
                    .internal_memory_control
                    .set_byte(0, value),
                0x801 => self
                    .interrupt_control
                    .internal_memory_control
                    .set_byte(1, value),
                0x802 => self
                    .interrupt_control
                    .internal_memory_control
                    .set_byte(2, value),
                0x803 => self
                    .interrupt_control
                    .internal_memory_control
                    .set_byte(3, value),
                _ => {
                    log("write on unused memory");
                    self.unused_region.insert(address, value);
                }
            },
        }
    }

    fn read_keypad_raw(&self, address: usize) -> u8 {
        match address {
            0x0400_0130 => self.keypad.key_input.get_byte(0),
            0x0400_0131 => self.keypad.key_input.get_byte(1),
            0x0400_0132 => self.keypad.key_interrupt_control.get_byte(0),
            0x0400_0133 => self.keypad.key_interrupt_control.get_byte(1),
            _ => panic!("Keypad read address is out of bound"),
        }
    }

    fn write_keypad_raw(&mut self, address: usize, value: u8) {
        match address {
            // 0x0400_0130 and 0x0400_0131 Should be read-only but CPU bios writes it.
            0x0400_0130 => self.keypad.key_input.set_byte(0, value),
            0x0400_0131 => self.keypad.key_input.set_byte(1, value),
            0x0400_0132 => self.keypad.key_interrupt_control.set_byte(0, value),
            0x0400_0133 => self.keypad.key_interrupt_control.set_byte(1, value),
            _ => panic!("Keypad write address is out of bound"),
        }
    }

    fn read_serial_raw(&self, address: usize) -> u8 {
        match address {
            0x0400_0120 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(0),
            0x0400_0121 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(1),
            0x0400_0122 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(2),
            0x0400_0123 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(3),
            0x0400_0124 => self.serial.sio_multi_data_2.get_byte(0),
            0x0400_0125 => self.serial.sio_multi_data_2.get_byte(1),
            0x0400_0126 => self.serial.sio_multi_data_3.get_byte(0),
            0x0400_0127 => self.serial.sio_multi_data_3.get_byte(1),
            0x0400_0128 => self.serial.sio_control_register.get_byte(0),
            0x0400_0129 => self.serial.sio_control_register.get_byte(1),
            0x0400_012A => self.serial.sio_multi_data_send_data_8.get_byte(0),
            0x0400_012B => self.serial.sio_multi_data_send_data_8.get_byte(1),
            0x0400_0134 => self.serial.sio_mode_select.get_byte(0),
            0x0400_0135 => self.serial.sio_mode_select.get_byte(1),
            0x0400_0136 => self.serial.infrared_register.get_byte(0),
            0x0400_0137 => self.serial.infrared_register.get_byte(1),
            0x0400_0140 => self.serial.sio_joy_bus_control.get_byte(0),
            0x0400_0141 => self.serial.sio_joy_bus_control.get_byte(1),
            0x0400_0150 => self.serial.sio_joy_bus_receive_data.get_byte(0),
            0x0400_0151 => self.serial.sio_joy_bus_receive_data.get_byte(1),
            0x0400_0152 => self.serial.sio_joy_bus_receive_data.get_byte(2),
            0x0400_0153 => self.serial.sio_joy_bus_receive_data.get_byte(3),
            0x0400_0154 => self.serial.sio_joy_bus_transmit_data.get_byte(0),
            0x0400_0155 => self.serial.sio_joy_bus_transmit_data.get_byte(1),
            0x0400_0156 => self.serial.sio_joy_bus_transmit_data.get_byte(2),
            0x0400_0157 => self.serial.sio_joy_bus_transmit_data.get_byte(3),
            0x0400_0158 => self.serial.sio_joy_bus_receive_status.get_byte(0),
            0x0400_0159 => self.serial.sio_joy_bus_receive_status.get_byte(1),
            0x0400_012C..=0x0400_012F
            | 0x0400_0138..=0x0400_0141
            | 0x0400_0142..=0x0400_014F
            | 0x0400_015A..=0x0400_01FF => {
                log(format!("read on unused memory {address:x}"));
                *self.unused_region.get(&address).unwrap_or(&0)
            }
            _ => panic!("Serial read address is out of bound: {address:#010x}"),
        }
    }

    fn write_serial_raw(&mut self, address: usize, value: u8) {
        match address {
            0x0400_0120 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(0, value),
            0x0400_0121 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(1, value),
            0x0400_0122 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(2, value),
            0x0400_0123 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(3, value),
            0x0400_0124 => self.serial.sio_multi_data_2.set_byte(0, value),
            0x0400_0125 => self.serial.sio_multi_data_2.set_byte(1, value),
            0x0400_0126 => self.serial.sio_multi_data_3.set_byte(0, value),
            0x0400_0127 => self.serial.sio_multi_data_3.set_byte(1, value),
            0x0400_0128 => self.serial.sio_control_register.set_byte(0, value),
            0x0400_0129 => self.serial.sio_control_register.set_byte(1, value),
            0x0400_012A => self.serial.sio_multi_data_send_data_8.set_byte(0, value),
            0x0400_012B => self.serial.sio_multi_data_send_data_8.set_byte(1, value),
            0x0400_0134 => self.serial.sio_mode_select.set_byte(0, value),
            0x0400_0135 => self.serial.sio_mode_select.set_byte(1, value),
            0x0400_0136 => self.serial.infrared_register.set_byte(0, value),
            0x0400_0137 => self.serial.infrared_register.set_byte(1, value),
            0x0400_0140 => self.serial.sio_joy_bus_control.set_byte(0, value),
            0x0400_0141 => self.serial.sio_joy_bus_control.set_byte(1, value),
            0x0400_0150 => self.serial.sio_joy_bus_receive_data.set_byte(0, value),
            0x0400_0151 => self.serial.sio_joy_bus_receive_data.set_byte(1, value),
            0x0400_0152 => self.serial.sio_joy_bus_receive_data.set_byte(2, value),
            0x0400_0153 => self.serial.sio_joy_bus_receive_data.set_byte(3, value),
            0x0400_0154 => self.serial.sio_joy_bus_transmit_data.set_byte(0, value),
            0x0400_0155 => self.serial.sio_joy_bus_transmit_data.set_byte(1, value),
            0x0400_0156 => self.serial.sio_joy_bus_transmit_data.set_byte(2, value),
            0x0400_0157 => self.serial.sio_joy_bus_transmit_data.set_byte(3, value),
            0x0400_0158 => self.serial.sio_joy_bus_receive_status.set_byte(0, value),
            0x0400_0159 => self.serial.sio_joy_bus_receive_status.set_byte(1, value),
            0x0400_012C..=0x0400_012F
            | 0x0400_0138..=0x0400_0139
            | 0x0400_0142..=0x0400_014F
            | 0x0400_015A..=0x0400_01FF => {
                log(format!("write on unused memory {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => {
                log(format!(
                    "Serial write to unhandled address: 0x{address:08X}"
                ));
                self.unused_region.insert(address, value);
            }
        }
    }

    fn read_timers_raw(&self, address: usize) -> u8 {
        match address {
            0x0400_0100 => self.timers.tm0cnt_l.get_byte(0),
            0x0400_0101 => self.timers.tm0cnt_l.get_byte(1),
            0x0400_0102 => self.timers.tm0cnt_h.get_byte(0),
            0x0400_0103 => self.timers.tm0cnt_h.get_byte(1),
            0x0400_0104 => self.timers.tm1cnt_l.get_byte(0),
            0x0400_0105 => self.timers.tm1cnt_l.get_byte(1),
            0x0400_0106 => self.timers.tm1cnt_h.get_byte(0),
            0x0400_0107 => self.timers.tm1cnt_h.get_byte(1),
            0x0400_0108 => self.timers.tm2cnt_l.get_byte(0),
            0x0400_0109 => self.timers.tm2cnt_l.get_byte(1),
            0x0400_010A => self.timers.tm2cnt_h.get_byte(0),
            0x0400_010B => self.timers.tm2cnt_h.get_byte(1),
            0x0400_010C => self.timers.tm3cnt_l.get_byte(0),
            0x0400_010D => self.timers.tm3cnt_l.get_byte(1),
            0x0400_010E => self.timers.tm3cnt_h.get_byte(0),
            0x0400_010F => self.timers.tm3cnt_h.get_byte(1),
            0x0400_0110..=0x0400_011F => self.unused_region.get(&address).map_or(0, |v| *v),
            _ => panic!("Timers read address is out of bound"),
        }
    }

    fn write_timers_raw(&mut self, address: usize, value: u8) {
        match address {
            // Timer 0 reload (writing to CNT_L sets reload value, not counter)
            0x0400_0100 => {
                let mut reload = self.timers.tm0_reload; // Use reload as base for byte write
                reload.set_byte(0, value);
                self.timers.set_reload(0, reload);
            }
            0x0400_0101 => {
                let mut reload = self.timers.tm0_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(0, reload);
            }
            // Timer 0 control
            0x0400_0102 => {
                let mut control = self.timers.tm0cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(0, control);
            }
            0x0400_0103 => {
                let mut control = self.timers.tm0cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(0, control);
            }
            // Timer 1 reload
            0x0400_0104 => {
                let mut reload = self.timers.tm1_reload;
                reload.set_byte(0, value);
                self.timers.set_reload(1, reload);
            }
            0x0400_0105 => {
                let mut reload = self.timers.tm1_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(1, reload);
            }
            // Timer 1 control
            0x0400_0106 => {
                let mut control = self.timers.tm1cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(1, control);
            }
            0x0400_0107 => {
                let mut control = self.timers.tm1cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(1, control);
            }
            // Timer 2 reload
            0x0400_0108 => {
                let mut reload = self.timers.tm2_reload;
                reload.set_byte(0, value);
                self.timers.set_reload(2, reload);
            }
            0x0400_0109 => {
                let mut reload = self.timers.tm2_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(2, reload);
            }
            // Timer 2 control
            0x0400_010A => {
                let mut control = self.timers.tm2cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(2, control);
            }
            0x0400_010B => {
                let mut control = self.timers.tm2cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(2, control);
            }
            // Timer 3 reload
            0x0400_010C => {
                let mut reload = self.timers.tm3_reload;
                reload.set_byte(0, value);
                self.timers.set_reload(3, reload);
            }
            0x0400_010D => {
                let mut reload = self.timers.tm3_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(3, reload);
            }
            // Timer 3 control
            0x0400_010E => {
                let mut control = self.timers.tm3cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(3, control);
            }
            0x0400_010F => {
                let mut control = self.timers.tm3cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(3, control);
            }
            0x0400_0110..=0x0400_011F => {
                log(format!("write on unused memory {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => panic!("Timers write address is out of bound"),
        }
    }

    fn read_dma_raw(&self, address: usize) -> u8 {
        let read_dma_bank = |channel: &Registers, address: usize| match address {
            0 => channel.source_address.get_byte(0),
            1 => channel.source_address.get_byte(1),
            2 => channel.source_address.get_byte(2),
            3 => channel.source_address.get_byte(3),
            4 => channel.destination_address.get_byte(0),
            5 => channel.destination_address.get_byte(1),
            6 => channel.destination_address.get_byte(2),
            7 => channel.destination_address.get_byte(3),
            8 => channel.word_count.get_byte(0),
            9 => channel.word_count.get_byte(1),
            10 => channel.control.get_byte(0),
            11 => channel.control.get_byte(1),
            _ => panic!("DMA channel read address is out of bound"),
        };

        match address {
            0x0400_00B0..=0x0400_00BB => {
                read_dma_bank(&self.dma.channels[0], address - 0x0400_00B0)
            }
            0x0400_00BC..=0x0400_00C7 => {
                read_dma_bank(&self.dma.channels[1], address - 0x0400_00BC)
            }
            0x0400_00C8..=0x0400_00D3 => {
                read_dma_bank(&self.dma.channels[2], address - 0x0400_00C8)
            }
            0x0400_00D4..=0x0400_00DF => {
                read_dma_bank(&self.dma.channels[3], address - 0x0400_00D4)
            }
            0x0400_00E0..=0x0400_00FF => {
                log(format!("read on unused memory 0x{address:08X}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("DMA read address is out of bound"),
        }
    }

    fn write_dma_raw(&mut self, address: usize, value: u8) {
        let write_dma_bank = |channel: &mut Registers, address: usize, value: u8| match address {
            0 => channel.source_address.set_byte(0, value),
            1 => channel.source_address.set_byte(1, value),
            2 => channel.source_address.set_byte(2, value),
            3 => channel.source_address.set_byte(3, value),
            4 => channel.destination_address.set_byte(0, value),
            5 => channel.destination_address.set_byte(1, value),
            6 => channel.destination_address.set_byte(2, value),
            7 => channel.destination_address.set_byte(3, value),
            8 => channel.word_count.set_byte(0, value),
            9 => channel.word_count.set_byte(1, value),
            10 => channel.control.set_byte(0, value),
            11 => channel.control.set_byte(1, value),
            _ => panic!("DMA channel write-address is out of bound"),
        };

        match address {
            0x0400_00B0..=0x0400_00BB => {
                write_dma_bank(&mut self.dma.channels[0], address - 0x0400_00B0, value);
            }
            0x0400_00BC..=0x0400_00C7 => {
                write_dma_bank(&mut self.dma.channels[1], address - 0x0400_00BC, value);
            }
            0x0400_00C8..=0x0400_00D3 => {
                write_dma_bank(&mut self.dma.channels[2], address - 0x0400_00C8, value);
            }
            0x0400_00D4..=0x0400_00DF => {
                write_dma_bank(&mut self.dma.channels[3], address - 0x0400_00D4, value);
            }
            0x0400_00E0..=0x0400_00FF => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            _ => panic!("Not implemented write memory address: {address:x}"),
        }

        // After writing DMA registers, check if an immediate transfer was triggered
        self.check_and_execute_dma();
    }

    /// Check if a DMA transfer should start and execute it completely
    fn check_and_execute_dma(&mut self) {
        if let Some(channel_idx) = self.dma.check_immediate_transfer() {
            // Execute all transfers for this DMA channel
            let is_32bit = self.dma.channels[channel_idx].control & (1 << 10) != 0;

            loop {
                let source = self.dma.channels[channel_idx].internal_source as usize;
                let dest = self.dma.channels[channel_idx].internal_dest as usize;

                // Perform the actual memory copy
                if is_32bit {
                    let value = self.read_word(source);
                    self.write_word(dest, value);
                } else {
                    let value = self.read_half_word(source);
                    self.write_half_word(dest, value);
                }

                // Update DMA state (increments internal_source and internal_dest) and check if more transfers remain
                if !self.dma.execute_transfer(channel_idx, |_, _, _| {}) {
                    break;
                }
            }
        }
    }

    fn read_sound_raw(&self, address: usize) -> u8 {
        match address {
            0x0400_0060 => self.sound.channel1_sweep.get_byte(0),
            0x0400_0061 => self.sound.channel1_sweep.get_byte(1),
            0x0400_0062 => self.sound.channel1_duty_length_envelope.get_byte(0),
            0x0400_0063 => self.sound.channel1_duty_length_envelope.get_byte(1),
            0x0400_0064 => self.sound.channel1_frequency_control.get_byte(0),
            0x0400_0065 => self.sound.channel1_frequency_control.get_byte(1),
            0x0400_0068 => self.sound.channel2_duty_length_envelope.get_byte(0),
            0x0400_0069 => self.sound.channel2_duty_length_envelope.get_byte(1),
            0x0400_006C => self.sound.channel2_frequency_control.get_byte(0),
            0x0400_006D => self.sound.channel2_frequency_control.get_byte(1),
            0x0400_0070 => self.sound.channel3_stop_wave_ram_select.get_byte(0),
            0x0400_0071 => self.sound.channel3_stop_wave_ram_select.get_byte(1),
            0x0400_0072 => self.sound.channel3_length_volume.get_byte(0),
            0x0400_0073 => self.sound.channel3_length_volume.get_byte(1),
            0x0400_0074 => self.sound.channel3_frequency_control.get_byte(0),
            0x0400_0075 => self.sound.channel3_frequency_control.get_byte(1),
            0x0400_0078 => self.sound.channel4_length_envelope.get_byte(0),
            0x0400_0079 => self.sound.channel4_length_envelope.get_byte(1),
            0x0400_007C => self.sound.channel4_frequency_control.get_byte(0),
            0x0400_007D => self.sound.channel4_frequency_control.get_byte(1),
            0x0400_0080 => self.sound.control_stereo_volume_enable.get_byte(0),
            0x0400_0081 => self.sound.control_stereo_volume_enable.get_byte(1),
            0x0400_0082 => self.sound.control_mixing_dma_control.get_byte(0),
            0x0400_0083 => self.sound.control_mixing_dma_control.get_byte(1),
            0x0400_0084 => self.sound.control_sound_on_off.get_byte(0),
            0x0400_0085 => self.sound.control_sound_on_off.get_byte(1),
            0x0400_0088 => self.sound.sound_pwm_control.get_byte(0),
            0x0400_0089 => self.sound.sound_pwm_control.get_byte(1),
            0x0400_0090..=0x0400_009F => {
                self.sound.channel3_wave_pattern_ram[address - 0x0400_0090]
            }
            0x0400_00A0 => self.sound.channel_a_fifo.get_byte(0),
            0x0400_00A1 => self.sound.channel_a_fifo.get_byte(1),
            0x0400_00A2 => self.sound.channel_a_fifo.get_byte(2),
            0x0400_00A3 => self.sound.channel_a_fifo.get_byte(3),
            0x0400_00A4 => self.sound.channel_b_fifo.get_byte(0),
            0x0400_00A5 => self.sound.channel_b_fifo.get_byte(1),
            0x0400_00A6 => self.sound.channel_b_fifo.get_byte(2),
            0x0400_00A7 => self.sound.channel_b_fifo.get_byte(3),
            0x0400_0066..=0x0400_0067
            | 0x0400_006A..=0x0400_006B
            | 0x0400_006E..=0x0400_006F
            | 0x0400_0076..=0x0400_0077
            | 0x0400_007A..=0x0400_007B
            | 0x0400_007E..=0x0400_007F
            | 0x0400_0086..=0x0400_0087
            | 0x0400_008A..=0x0400_008F
            | 0x0400_00A8..=0x0400_00AF => {
                log(format!("read on unused memory {address:x}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("Sound read address is out of bound"),
        }
    }

    fn write_sound_raw(&mut self, address: usize, value: u8) {
        match address {
            0x0400_0060 => self.sound.channel1_sweep.set_byte(0, value),
            0x0400_0061 => self.sound.channel1_sweep.set_byte(1, value),
            0x0400_0062 => self.sound.channel1_duty_length_envelope.set_byte(0, value),
            0x0400_0063 => self.sound.channel1_duty_length_envelope.set_byte(1, value),
            0x0400_0064 => self.sound.channel1_frequency_control.set_byte(0, value),
            0x0400_0065 => self.sound.channel1_frequency_control.set_byte(1, value),
            0x0400_0068 => self.sound.channel2_duty_length_envelope.set_byte(0, value),
            0x0400_0069 => self.sound.channel2_duty_length_envelope.set_byte(1, value),
            0x0400_006C => self.sound.channel2_frequency_control.set_byte(0, value),
            0x0400_006D => self.sound.channel2_frequency_control.set_byte(1, value),
            0x0400_0070 => self.sound.channel3_stop_wave_ram_select.set_byte(0, value),
            0x0400_0071 => self.sound.channel3_stop_wave_ram_select.set_byte(1, value),
            0x0400_0072 => self.sound.channel3_length_volume.set_byte(0, value),
            0x0400_0073 => self.sound.channel3_length_volume.set_byte(1, value),
            0x0400_0074 => self.sound.channel3_frequency_control.set_byte(0, value),
            0x0400_0075 => self.sound.channel3_frequency_control.set_byte(1, value),
            0x0400_0078 => self.sound.channel4_length_envelope.set_byte(0, value),
            0x0400_0079 => self.sound.channel4_length_envelope.set_byte(1, value),
            0x0400_007C => self.sound.channel4_frequency_control.set_byte(0, value),
            0x0400_007D => self.sound.channel4_frequency_control.set_byte(1, value),
            0x0400_0080 => self.sound.control_stereo_volume_enable.set_byte(0, value),
            0x0400_0081 => self.sound.control_stereo_volume_enable.set_byte(1, value),
            0x0400_0082 => self.sound.control_mixing_dma_control.set_byte(0, value),
            0x0400_0083 => self.sound.control_mixing_dma_control.set_byte(1, value),
            0x0400_0084 => self.sound.control_sound_on_off.set_byte(0, value),
            0x0400_0085 => self.sound.control_sound_on_off.set_byte(1, value),
            0x0400_0088 => self.sound.sound_pwm_control.set_byte(0, value),
            0x0400_0089 => self.sound.sound_pwm_control.set_byte(1, value),
            0x0400_0090..=0x0400_009F => {
                self.sound.channel3_wave_pattern_ram[address - 0x0400_0090] = value;
            }
            0x0400_00A0 => self.sound.channel_a_fifo.set_byte(0, value),
            0x0400_00A1 => self.sound.channel_a_fifo.set_byte(1, value),
            0x0400_00A2 => self.sound.channel_a_fifo.set_byte(2, value),
            0x0400_00A3 => self.sound.channel_a_fifo.set_byte(3, value),
            0x0400_00A4 => self.sound.channel_b_fifo.set_byte(0, value),
            0x0400_00A5 => self.sound.channel_b_fifo.set_byte(1, value),
            0x0400_00A6 => self.sound.channel_b_fifo.set_byte(2, value),
            0x0400_00A7 => self.sound.channel_b_fifo.set_byte(3, value),
            0x0400_0066..=0x0400_0067
            | 0x0400_006A..=0x0400_006B
            | 0x0400_006E..=0x0400_006F
            | 0x0400_0076..=0x0400_0077
            | 0x0400_007A..=0x0400_007B
            | 0x0400_007E..=0x0400_007F
            | 0x0400_0086..=0x0400_0087
            | 0x0400_008A..=0x0400_008F
            | 0x0400_00A8..=0x0400_00AF => {
                log(format!("write on unused memory, {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => panic!("Sound write address is out of bound"),
        }
    }

    fn read_lcd_raw(&self, address: usize) -> u8 {
        match address {
            0x0400_0000 => self.lcd.registers.dispcnt.get_byte(0),
            0x0400_0001 => self.lcd.registers.dispcnt.get_byte(1),
            0x0400_0002 => self.lcd.registers.green_swap.get_byte(0),
            0x0400_0003 => self.lcd.registers.green_swap.get_byte(1),
            0x0400_0004 => self.lcd.registers.dispstat.get_byte(0),
            0x0400_0005 => self.lcd.registers.dispstat.get_byte(1),
            0x0400_0006 => self.lcd.registers.vcount.get_byte(0),
            0x0400_0007 => self.lcd.registers.vcount.get_byte(1),
            0x0400_0008 => self.lcd.registers.bg0cnt.get_byte(0),
            0x0400_0009 => self.lcd.registers.bg0cnt.get_byte(1),
            0x0400_000A => self.lcd.registers.bg1cnt.get_byte(0),
            0x0400_000B => self.lcd.registers.bg1cnt.get_byte(1),
            0x0400_000C => self.lcd.registers.bg2cnt.get_byte(0),
            0x0400_000D => self.lcd.registers.bg2cnt.get_byte(1),
            0x0400_000E => self.lcd.registers.bg3cnt.get_byte(0),
            0x0400_000F => self.lcd.registers.bg3cnt.get_byte(1),
            0x0400_0010 => self.lcd.registers.bg0hofs.get_byte(0),
            0x0400_0011 => self.lcd.registers.bg0hofs.get_byte(1),
            0x0400_0012 => self.lcd.registers.bg0vofs.get_byte(0),
            0x0400_0013 => self.lcd.registers.bg0vofs.get_byte(1),
            0x0400_0014 => self.lcd.registers.bg1hofs.get_byte(0),
            0x0400_0015 => self.lcd.registers.bg1hofs.get_byte(1),
            0x0400_0016 => self.lcd.registers.bg1vofs.get_byte(0),
            0x0400_0017 => self.lcd.registers.bg1vofs.get_byte(1),
            0x0400_0018 => self.lcd.registers.bg2hofs.get_byte(0),
            0x0400_0019 => self.lcd.registers.bg2hofs.get_byte(1),
            0x0400_001A => self.lcd.registers.bg2vofs.get_byte(0),
            0x0400_001B => self.lcd.registers.bg2vofs.get_byte(1),
            0x0400_001C => self.lcd.registers.bg3hofs.get_byte(0),
            0x0400_001D => self.lcd.registers.bg3hofs.get_byte(1),
            0x0400_001E => self.lcd.registers.bg3vofs.get_byte(0),
            0x0400_001F => self.lcd.registers.bg3vofs.get_byte(1),
            0x0400_0020 => self.lcd.registers.bg2pa.get_byte(0),
            0x0400_0021 => self.lcd.registers.bg2pa.get_byte(1),
            0x0400_0022 => self.lcd.registers.bg2pb.get_byte(0),
            0x0400_0023 => self.lcd.registers.bg2pb.get_byte(1),
            0x0400_0024 => self.lcd.registers.bg2pc.get_byte(0),
            0x0400_0025 => self.lcd.registers.bg2pc.get_byte(1),
            0x0400_0026 => self.lcd.registers.bg2pd.get_byte(0),
            0x0400_0027 => self.lcd.registers.bg2pd.get_byte(1),
            0x0400_0028 => self.lcd.registers.bg2x.get_byte(0),
            0x0400_0029 => self.lcd.registers.bg2x.get_byte(1),
            0x0400_002A => self.lcd.registers.bg2x.get_byte(2),
            0x0400_002B => self.lcd.registers.bg2x.get_byte(3),
            0x0400_002C => self.lcd.registers.bg2y.get_byte(0),
            0x0400_002D => self.lcd.registers.bg2y.get_byte(1),
            0x0400_002E => self.lcd.registers.bg2y.get_byte(2),
            0x0400_002F => self.lcd.registers.bg2y.get_byte(3),
            0x0400_0030 => self.lcd.registers.bg3pa.get_byte(0),
            0x0400_0031 => self.lcd.registers.bg3pa.get_byte(1),
            0x0400_0032 => self.lcd.registers.bg3pb.get_byte(0),
            0x0400_0033 => self.lcd.registers.bg3pb.get_byte(1),
            0x0400_0034 => self.lcd.registers.bg3pc.get_byte(0),
            0x0400_0035 => self.lcd.registers.bg3pc.get_byte(1),
            0x0400_0036 => self.lcd.registers.bg3pd.get_byte(0),
            0x0400_0037 => self.lcd.registers.bg3pd.get_byte(1),
            0x0400_0038 => self.lcd.registers.bg3x.get_byte(0),
            0x0400_0039 => self.lcd.registers.bg3x.get_byte(1),
            0x0400_003A => self.lcd.registers.bg3x.get_byte(2),
            0x0400_003B => self.lcd.registers.bg3x.get_byte(3),
            0x0400_003C => self.lcd.registers.bg3y.get_byte(0),
            0x0400_003D => self.lcd.registers.bg3y.get_byte(1),
            0x0400_003E => self.lcd.registers.bg3y.get_byte(2),
            0x0400_003F => self.lcd.registers.bg3y.get_byte(3),
            0x0400_0040 => self.lcd.registers.win0h.get_byte(0),
            0x0400_0041 => self.lcd.registers.win0h.get_byte(1),
            0x0400_0042 => self.lcd.registers.win1h.get_byte(0),
            0x0400_0043 => self.lcd.registers.win1h.get_byte(1),
            0x0400_0044 => self.lcd.registers.win0v.get_byte(0),
            0x0400_0045 => self.lcd.registers.win0v.get_byte(1),
            0x0400_0046 => self.lcd.registers.win1v.get_byte(0),
            0x0400_0047 => self.lcd.registers.win1v.get_byte(1),
            0x0400_0048 => self.lcd.registers.winin.get_byte(0),
            0x0400_0049 => self.lcd.registers.winin.get_byte(1),
            0x0400_004A => self.lcd.registers.winout.get_byte(0),
            0x0400_004B => self.lcd.registers.winout.get_byte(1),
            0x0400_004C => self.lcd.registers.mosaic.get_byte(0),
            0x0400_004D => self.lcd.registers.mosaic.get_byte(1),
            0x0400_0050 => self.lcd.registers.bldcnt.get_byte(0),
            0x0400_0051 => self.lcd.registers.bldcnt.get_byte(1),
            0x0400_0052 => self.lcd.registers.bldalpha.get_byte(0),
            0x0400_0053 => self.lcd.registers.bldalpha.get_byte(1),
            0x0400_0054 => self.lcd.registers.bldy.get_byte(0),
            0x0400_0055 => self.lcd.registers.bldy.get_byte(1),
            0x0400_004E..=0x0400_004F | 0x0400_0056..=0x0400_005F => {
                log(format!("read on unused memory 0x{address:08X}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("LCD read address is out of bound"),
        }
    }

    fn write_lcd_raw(&mut self, address: usize, value: u8) {
        match address {
            0x0400_0000 => {
                self.lcd.registers.dispcnt.set_byte(0, value);
            }
            0x0400_0001 => {
                self.lcd.registers.dispcnt.set_byte(1, value);
            }
            0x0400_0002 => self.lcd.registers.green_swap.set_byte(0, value),
            0x0400_0003 => self.lcd.registers.green_swap.set_byte(1, value),
            0x0400_0004 => self.lcd.registers.dispstat.set_byte(0, value),
            0x0400_0005 => self.lcd.registers.dispstat.set_byte(1, value),
            0x0400_0008 => self.lcd.registers.bg0cnt.set_byte(0, value),
            0x0400_0006 => self.lcd.registers.vcount.set_byte(0, value),
            0x0400_0007 => self.lcd.registers.vcount.set_byte(1, value),
            0x0400_0009 => self.lcd.registers.bg0cnt.set_byte(1, value),
            0x0400_000A => self.lcd.registers.bg1cnt.set_byte(0, value),
            0x0400_000B => self.lcd.registers.bg1cnt.set_byte(1, value),
            0x0400_000C => self.lcd.registers.bg2cnt.set_byte(0, value),
            0x0400_000D => self.lcd.registers.bg2cnt.set_byte(1, value),
            0x0400_000E => self.lcd.registers.bg3cnt.set_byte(0, value),
            0x0400_000F => self.lcd.registers.bg3cnt.set_byte(1, value),
            0x0400_0010 => self.lcd.registers.bg0hofs.set_byte(0, value),
            0x0400_0011 => self.lcd.registers.bg0hofs.set_byte(1, value),
            0x0400_0012 => self.lcd.registers.bg0vofs.set_byte(0, value),
            0x0400_0013 => self.lcd.registers.bg0vofs.set_byte(1, value),
            0x0400_0014 => self.lcd.registers.bg1hofs.set_byte(0, value),
            0x0400_0015 => self.lcd.registers.bg1hofs.set_byte(1, value),
            0x0400_0016 => self.lcd.registers.bg1vofs.set_byte(0, value),
            0x0400_0017 => self.lcd.registers.bg1vofs.set_byte(1, value),
            0x0400_0018 => self.lcd.registers.bg2hofs.set_byte(0, value),
            0x0400_0019 => self.lcd.registers.bg2hofs.set_byte(1, value),
            0x0400_001A => self.lcd.registers.bg2vofs.set_byte(0, value),
            0x0400_001B => self.lcd.registers.bg2vofs.set_byte(1, value),
            0x0400_001C => self.lcd.registers.bg3hofs.set_byte(0, value),
            0x0400_001D => self.lcd.registers.bg3hofs.set_byte(1, value),
            0x0400_001E => self.lcd.registers.bg3vofs.set_byte(0, value),
            0x0400_001F => self.lcd.registers.bg3vofs.set_byte(1, value),
            0x0400_0020 => self.lcd.registers.bg2pa.set_byte(0, value),
            0x0400_0021 => self.lcd.registers.bg2pa.set_byte(1, value),
            0x0400_0022 => self.lcd.registers.bg2pb.set_byte(0, value),
            0x0400_0023 => self.lcd.registers.bg2pb.set_byte(1, value),
            0x0400_0024 => self.lcd.registers.bg2pc.set_byte(0, value),
            0x0400_0025 => self.lcd.registers.bg2pc.set_byte(1, value),
            0x0400_0026 => self.lcd.registers.bg2pd.set_byte(0, value),
            0x0400_0027 => self.lcd.registers.bg2pd.set_byte(1, value),
            0x0400_0028 => self.lcd.registers.bg2x.set_byte(0, value),
            0x0400_0029 => self.lcd.registers.bg2x.set_byte(1, value),
            0x0400_002A => self.lcd.registers.bg2x.set_byte(2, value),
            0x0400_002B => self.lcd.registers.bg2x.set_byte(3, value),
            0x0400_002C => self.lcd.registers.bg2y.set_byte(0, value),
            0x0400_002D => self.lcd.registers.bg2y.set_byte(1, value),
            0x0400_002E => self.lcd.registers.bg2y.set_byte(2, value),
            0x0400_002F => self.lcd.registers.bg2y.set_byte(3, value),
            0x0400_0030 => self.lcd.registers.bg3pa.set_byte(0, value),
            0x0400_0031 => self.lcd.registers.bg3pa.set_byte(1, value),
            0x0400_0032 => self.lcd.registers.bg3pb.set_byte(0, value),
            0x0400_0033 => self.lcd.registers.bg3pb.set_byte(1, value),
            0x0400_0034 => self.lcd.registers.bg3pc.set_byte(0, value),
            0x0400_0035 => self.lcd.registers.bg3pc.set_byte(1, value),
            0x0400_0036 => self.lcd.registers.bg3pd.set_byte(0, value),
            0x0400_0037 => self.lcd.registers.bg3pd.set_byte(1, value),
            0x0400_0038 => self.lcd.registers.bg3x.set_byte(0, value),
            0x0400_0039 => self.lcd.registers.bg3x.set_byte(1, value),
            0x0400_003A => self.lcd.registers.bg3x.set_byte(2, value),
            0x0400_003B => self.lcd.registers.bg3x.set_byte(3, value),
            0x0400_003C => self.lcd.registers.bg3y.set_byte(0, value),
            0x0400_003D => self.lcd.registers.bg3y.set_byte(1, value),
            0x0400_003E => self.lcd.registers.bg3y.set_byte(2, value),
            0x0400_003F => self.lcd.registers.bg3y.set_byte(3, value),
            0x0400_0040 => self.lcd.registers.win0h.set_byte(0, value),
            0x0400_0041 => self.lcd.registers.win0h.set_byte(1, value),
            0x0400_0042 => self.lcd.registers.win1h.set_byte(0, value),
            0x0400_0043 => self.lcd.registers.win1h.set_byte(1, value),
            0x0400_0044 => self.lcd.registers.win0v.set_byte(0, value),
            0x0400_0045 => self.lcd.registers.win0v.set_byte(1, value),
            0x0400_0046 => self.lcd.registers.win1v.set_byte(0, value),
            0x0400_0047 => self.lcd.registers.win1v.set_byte(1, value),
            0x0400_0048 => self.lcd.registers.winin.set_byte(0, value),
            0x0400_0049 => self.lcd.registers.winin.set_byte(1, value),
            0x0400_004A => self.lcd.registers.winout.set_byte(0, value),
            0x0400_004B => self.lcd.registers.winout.set_byte(1, value),
            0x0400_004C => self.lcd.registers.mosaic.set_byte(0, value),
            0x0400_004D => self.lcd.registers.mosaic.set_byte(1, value),
            // 0x0400_004E, 0x0400_004F are not used
            0x0400_0050 => self.lcd.registers.bldcnt.set_byte(0, value),
            0x0400_0051 => self.lcd.registers.bldcnt.set_byte(1, value),
            0x0400_0052 => self.lcd.registers.bldalpha.set_byte(0, value),
            0x0400_0053 => self.lcd.registers.bldalpha.set_byte(1, value),
            0x0400_0054 => self.lcd.registers.bldy.set_byte(0, value),
            0x0400_0055 => self.lcd.registers.bldy.set_byte(1, value),
            0x0400_004E..=0x0400_004F | 0x0400_0056..=0x0400_005F => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            _ => panic!("LCD write address is out of bound"),
        }
    }

    #[must_use]
    pub fn read_raw(&self, address: usize) -> u8 {
        // Mask address to 32-bit to handle potential overflow issues
        let address = address & 0xFFFF_FFFF;
        match address {
            0x0000_0000..=0x0000_3FFF => {
                // BIOS read protection: if PC is outside BIOS, return last BIOS opcode
                if self.current_pc >= 0x4000 {
                    // Return the appropriate byte from last_bios_opcode
                    self.last_bios_opcode
                        .get_byte(u8::try_from(address & 0b11).unwrap())
                } else {
                    self.internal_memory.read_at(address)
                }
            }
            (0x0200_0000..=0x03FF_FFFF) | (0x0800_0000..=0x0E00_FFFF) => {
                self.internal_memory.read_at(address)
            }
            0x0400_0000..=0x0400_005F => self.read_lcd_raw(address),
            0x0400_0060..=0x0400_00AF => self.read_sound_raw(address),
            0x0400_00B0..=0x0400_00FF => self.read_dma_raw(address),
            0x0400_0100..=0x0400_011F => self.read_timers_raw(address),
            0x0400_0130..=0x0400_0133 => self.read_keypad_raw(address),
            0x0400_0120..=0x0400_012F | 0x0400_0134..=0x0400_01FF => self.read_serial_raw(address),
            0x0400_0200..=0x04FF_FFFF => self.read_interrupt_control_raw(address),
            0x0500_0000..=0x05FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_FF00, 0xFF00_00FF, 8, 4);

                match unmasked_address {
                    0x0500_0000..=0x0500_01FF => {
                        self.lcd.memory.bg_palette_ram[unmasked_address - 0x0500_0000]
                    }
                    0x0500_0200..=0x0500_03FF => {
                        self.lcd.memory.obj_palette_ram[unmasked_address - 0x0500_0200]
                    }
                    _ => unreachable!(),
                }
            }
            0x0600_0000..=0x06FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_0000, 0xFF00_FFFF, 16, 2);

                // VRAM is 64k+32k+32k with the last two 32k being one mirrors of each other
                match unmasked_address {
                    0x0600_0000..=0x0601_7FFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x0600_0000]
                    }
                    0x0601_8000..=0x0601_FFFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x0600_0000 - 0x8000]
                    }
                    _ => unreachable!(),
                }
            }
            0x0700_0000..=0x07FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_FF00, 0xFF00_00FF, 8, 4);

                self.lcd.memory.obj_attributes[unmasked_address - 0x0700_0000]
            }
            0x000_4000..=0x1FF_FFFF | 0xE01_0000..=0xFFF_FFFF | 0x1000_0000..=0xFFFF_FFFF => {
                log(format!("read on unused memory {address:x}"));
                *self.unused_region.get(&address).unwrap_or(&0)
            }
            _ => unimplemented!(),
        }
    }

    pub fn write_raw(&mut self, address: usize, value: u8) {
        // Mask address to 32-bit to handle potential overflow issues
        let address = address & 0xFFFF_FFFF;
        match address {
            0x0000_0000..=0x0000_3FFF | 0x0200_0000..=0x03FF_FFFF | 0x0800_0000..=0x0E00_FFFF => {
                self.internal_memory.write_at(address, value);
            }
            0x0400_0000..=0x0400_005F => self.write_lcd_raw(address, value),
            0x0400_0060..=0x0400_00AF => self.write_sound_raw(address, value),
            0x0400_00B0..=0x0400_00FF => self.write_dma_raw(address, value),
            0x0400_0100..=0x0400_011F => self.write_timers_raw(address, value),
            0x0400_0120..=0x0400_012F | 0x0400_0134..=0x0400_01FF => {
                self.write_serial_raw(address, value);
            }
            0x0400_0130..=0x0400_0133 => self.write_keypad_raw(address, value),
            0x0400_0200..=0x04FF_FFFF => self.write_interrupt_control_raw(address, value),
            0x0500_0000..=0x05FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_FF00, 0xFF00_00FF, 8, 4);

                match unmasked_address {
                    0x0500_0000..=0x0500_01FF => {
                        self.lcd.memory.bg_palette_ram[unmasked_address - 0x0500_0000] = value;
                    }
                    0x0500_0200..=0x0500_03FF => {
                        self.lcd.memory.obj_palette_ram[unmasked_address - 0x0500_0200] = value;
                    }
                    _ => unreachable!(),
                }
            }
            0x0600_0000..=0x06FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_0000, 0xFF00_FFFF, 16, 2);

                // VRAM is 64k+32k+32k with the last two 32k being one mirrors of each other
                match unmasked_address {
                    0x0600_0000..=0x0601_7FFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x0600_0000] = value;
                    }
                    0x0601_8000..=0x0601_FFFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x0600_0000 - 0x8000] = value;
                    }
                    _ => unreachable!(),
                }
            }
            0x700_0000..=0x7FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_FF00, 0xFF00_00FF, 8, 4);

                self.lcd.memory.obj_attributes[unmasked_address - 0x0700_0000] = value;
            }
            0x000_4000..=0x1FF_FFFF | 0xE01_0000..=0xFFF_FFFF | 0x1000_0000..=0xFFFF_FFFF => {
                log(format!("write on unused memory {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => {
                panic!("Unimplemented write to address 0x{address:08X} with value 0x{value:02X}");
            }
        }
    }

    pub fn read_byte(&mut self, address: usize) -> u8 {
        // TODO: Implement proper cycle-based timing
        self.cycles_count += self.get_wait_cycles(address);

        self.last_used_address = address;

        self.read_raw(address)
    }

    pub fn write_byte(&mut self, address: usize, value: u8) {
        // TODO: Implement proper cycle-based timing
        self.cycles_count += self.get_wait_cycles(address);

        self.last_used_address = address;

        // Special handling for video memory byte writes
        match address {
            // in OAM (object attributes map) byte writes are ignored
            0x0700_0000..=0x07FF_FFFF => {
                log("OAM byte write ignored");
                return;
            }
            // VRAM byte writes: In bitmap modes, byte writes are duplicated to halfwords
            // and work throughout the framebuffer area. In tile modes, byte writes to
            // OBJ VRAM (0x0601_0000-0x0601_7FFF) have special behavior.
            // For now, allow byte writes to all of VRAM (96KB = 0x0001_8000 bytes)
            0x0600_0000..=0x06FF_FFFF => {
                let unmasked_address =
                    get_unmasked_address(address, 0x00FF_0000, 0xFF00_FFFF, 16, 2);

                // Byte writes work throughout VRAM (duplicated as halfword)
                if unmasked_address < 0x0601_8000 {
                    // Write as halfword with byte duplicated, aligned to halfword boundary
                    let aligned_address = address & !1;
                    self.write_raw(aligned_address, value);
                    self.write_raw(aligned_address + 1, value);
                } else {
                    log(format!(
                        "VRAM byte write ignored (unmasked address 0x{unmasked_address:08X} >= 0x0601_8000)"
                    ));
                }
                return;
            }
            // in palette RAM byte writes are duplicated into halfwords
            0x0500_0000..=0x05FF_FFFF => {
                // Write as halfword with byte duplicated, aligned to halfword boundary
                let aligned_address = address & !1;
                self.write_raw(aligned_address, value);
                self.write_raw(aligned_address + 1, value);
                return;
            }
            _ => {}
        }

        self.write_raw(address, value);
    }

    pub(crate) fn step(&mut self) {
        self.cycles_count += 1;

        // Step timers every CPU cycle
        let timer_result = self.timers.step();
        if timer_result.timer0_overflow {
            self.request_interrupt(IrqType::Timer0);
        }
        if timer_result.timer1_overflow {
            self.request_interrupt(IrqType::Timer1);
        }
        if timer_result.timer2_overflow {
            self.request_interrupt(IrqType::Timer2);
        }
        if timer_result.timer3_overflow {
            self.request_interrupt(IrqType::Timer3);
        }

        // A pixel takes 4 cycles to get drawn
        if self.cycles_count.is_multiple_of(4) {
            let lcd_output = self.lcd.step();

            if lcd_output.request_hblank_irq {
                self.request_interrupt(IrqType::HBlank);
            }

            if lcd_output.request_vblank_irq {
                self.request_interrupt(IrqType::VBlank);
            }

            if lcd_output.request_vcount_irq {
                self.request_interrupt(IrqType::VCount);
            }
        }
    }

    pub(crate) fn request_interrupt(&mut self, irq_type: IrqType) {
        self.interrupt_control
            .interrupt_request
            .set_bit(irq_type.get_idx_in_if(), true);
    }

    #[must_use]
    pub fn with_memory(memory: InternalMemory) -> Self {
        Self {
            internal_memory: memory,
            ..Default::default()
        }
    }

    const fn get_wait_cycles(&self, address: usize) -> u128 {
        let _ = self;
        let _ = address;

        // let _is_sequential =
        // address == self.last_used_address || address + 4 == self.last_used_address;

        // TODO: Restore this when we have a proper memory map
        // match address {
        // Bios
        // 0x0..=0x3FFF => 1,
        // _ => 1,
        // }

        1
    }

    pub fn read_word(&mut self, mut address: usize) -> u32 {
        // TODO: here we have to see how many times to wait for the waitcycles
        // It depends on the bus width of the memory region
        // Right now we're assuming that every region has a bus width of 32 bits
        // So we wait only once to read a word.
        // In reality for example WRAM has a bus width of 16 bits so we would
        // have to repeat this cycle 2 times (to emulate the fact that we will access the memory
        // two times)
        // TODO: Implement proper cycle-based timing
        self.cycles_count += self.get_wait_cycles(address);

        self.last_used_address = address;

        if address & 3 != 0 {
            log("warning, read_word has address not word aligned");
            address &= !3;
        }

        let part_0: u32 = self.read_raw(address).into();
        let part_1: u32 = self.read_raw(address + 1).into();
        let part_2: u32 = self.read_raw(address + 2).into();
        let part_3: u32 = self.read_raw(address + 3).into();

        part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0
    }

    pub fn write_word(&mut self, mut address: usize, value: u32) {
        // TODO: Look at read_word
        // TODO: Implement proper cycle-based timing
        self.cycles_count += self.get_wait_cycles(address);

        self.last_used_address = address;

        if address & 3 != 0 {
            log("warning, write_word has address not word aligned");
            address &= !3;
        }

        let part_0: u8 = value.get_bits(0..=7).try_into().unwrap();
        let part_1: u8 = value.get_bits(8..=15).try_into().unwrap();
        let part_2: u8 = value.get_bits(16..=23).try_into().unwrap();
        let part_3: u8 = value.get_bits(24..=31).try_into().unwrap();

        self.write_raw(address, part_0);
        self.write_raw(address + 1, part_1);
        self.write_raw(address + 2, part_2);
        self.write_raw(address + 3, part_3);
    }

    pub fn read_half_word(&mut self, mut address: usize) -> u16 {
        // TODO: Implement proper cycle-based timing instead of recursive step() calls
        // For now, just track cycles without stepping to avoid recursion bugs
        self.cycles_count += self.get_wait_cycles(address);

        self.last_used_address = address;

        if address & 1 != 0 {
            log("warning, read_half_word has address not half-word aligned");
            address &= !1;
        }

        let part_0: u16 = self.read_raw(address).into();
        let part_1: u16 = self.read_raw(address + 1).into();

        part_1 << 8 | part_0
    }

    pub fn write_half_word(&mut self, mut address: usize, value: u16) {
        // TODO: Look at read_word
        // TODO: Implement proper cycle-based timing
        self.cycles_count += self.get_wait_cycles(address);

        self.last_used_address = address;

        if address & 1 != 0 {
            log("warning, write_half_word has address not half-word aligned");
            address &= !1;
        }

        let part_0: u8 = value.get_bits(0..=7).try_into().unwrap();
        let part_1: u8 = value.get_bits(8..=15).try_into().unwrap();

        self.write_raw(address, part_0);
        self.write_raw(address + 1, part_1);
    }

    /// Returns true if there is an enabled interrupt pending
    #[must_use]
    pub const fn is_irq_pending(&self) -> bool {
        // Interrupt Master Enable has to be 1
        // && there needs to be an interrupt requested which is also enabled in the interrupt enable reg
        (self.interrupt_control.interrupt_master_enable == 1)
            && (self.interrupt_control.interrupt_enable & self.interrupt_control.interrupt_request
                != 0)
    }

    /// Updates the current program counter for BIOS read protection
    pub const fn set_current_pc(&mut self, pc: usize) {
        self.current_pc = pc;
    }

    /// Updates the last BIOS opcode for BIOS read protection
    pub const fn set_last_bios_opcode(&mut self, opcode: u32) {
        self.last_bios_opcode = opcode;
    }
}

#[cfg(test)]
mod tests {
    use crate::bus::Bus;

    #[test]
    fn test_write_lcd_reg() {
        let mut bus = Bus::default();
        let address = 0x0400_0048; // WININ lower byte

        bus.write_raw(address, 10);

        assert_eq!(bus.lcd.registers.winin, 10);

        let address = 0x0400_0049; // WININ higher byte

        bus.write_raw(address, 5);
        assert_eq!(bus.lcd.registers.winin, (5 << 8) | 10);
    }

    #[test]
    fn test_read_lcd_reg() {
        let mut bus = Bus::default();
        let address = 0x0400_0048; // WININ lower byte

        bus.lcd.registers.winin = (5 << 8) | 10;

        assert_eq!(bus.read_raw(address), 10);

        let address = 0x0400_0049; // WININ higher byte

        assert_eq!(bus.read_raw(address), 5);
    }

    #[test]
    fn test_write_timer_register() {
        let mut bus = Bus::default();
        let address = 0x0400_0100;

        // Writing to TM0CNT_L sets the reload value, not the counter directly
        bus.write_raw(address, 10);
        assert_eq!(bus.timers.tm0_reload, 10);
    }

    #[test]
    fn test_read_timer_register() {
        let mut bus = Bus::default();
        let address = 0x0400_0100;

        bus.timers.tm0cnt_l = (5 << 8) | 10;

        assert_eq!(bus.read_raw(address), 10);
    }

    #[test]
    fn write_bg_palette_ram() {
        let mut bus = Bus::default();
        let address = 0x0500_0008;

        bus.write_raw(address, 10);
        assert_eq!(bus.lcd.memory.bg_palette_ram[8], 10);
    }

    #[test]
    fn read_bg_palette_ram() {
        let mut bus = Bus::default();
        bus.lcd.memory.bg_palette_ram[8] = 15;

        let address = 0x0500_0008;
        let value = bus.read_raw(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_bg_palette_ram() {
        let mut bus = Bus::default();

        let address = 0x0500_01FF;
        bus.write_raw(address, 5);

        assert_eq!(bus.lcd.memory.bg_palette_ram[0x1FF], 5);
    }

    #[test]
    fn write_obj_palette_ram() {
        let mut bus = Bus::default();
        let address = 0x0500_0208;

        bus.write_raw(address, 10);
        assert_eq!(bus.lcd.memory.obj_palette_ram[8], 10);
    }

    #[test]
    fn read_obj_palette_ram() {
        let mut bus = Bus::default();
        bus.lcd.memory.obj_palette_ram[8] = 15;

        let address = 0x0500_0208;

        let value = bus.read_raw(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_obj_palette_ram() {
        let mut bus = Bus::default();

        let address = 0x0500_03FF;
        bus.write_raw(address, 5);

        assert_eq!(bus.lcd.memory.obj_palette_ram[0x1FF], 5);
    }

    #[test]
    fn write_vram() {
        let mut bus = Bus::default();
        let address = 0x0600_0004;

        bus.write_raw(address, 23);
        assert_eq!(bus.lcd.memory.video_ram[4], 23);
    }

    #[test]
    fn read_vram() {
        let mut bus = Bus::default();
        bus.lcd.memory.video_ram[4] = 15;

        let address = 0x0600_0004;
        let value = bus.read_raw(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_vram() {
        let mut bus = Bus::default();

        let address = 0x0601_7FFF;
        bus.write_raw(address, 5);

        assert_eq!(bus.lcd.memory.video_ram[0x0001_7FFF], 5);
    }

    #[test]
    fn test_mirror_bg_palette() {
        let mut bus = Bus::default();
        bus.lcd.memory.bg_palette_ram[0x134] = 5;

        assert_eq!(bus.read_raw(0x0500_0134), 5);
        assert_eq!(bus.read_raw(0x0500_0534), 5);
        assert_eq!(bus.read_raw(0x0501_2534), 5);
        assert_eq!(bus.read_raw(0x05FF_FD34), 5);

        bus.write_raw(0x0500_0134, 10);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 10);

        bus.write_raw(0x0500_0534, 11);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 11);

        bus.write_raw(0x0501_2534, 12);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 12);

        bus.write_raw(0x05FF_FD34, 13);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 13);
    }

    #[test]
    fn test_mirror_obj_palette() {
        let mut bus = Bus::default();
        bus.lcd.memory.obj_palette_ram[0x134] = 5;

        assert_eq!(bus.read_raw(0x0500_0334), 5);
        assert_eq!(bus.read_raw(0x0500_0734), 5);
        assert_eq!(bus.read_raw(0x0501_2734), 5);
        assert_eq!(bus.read_raw(0x05FF_FF34), 5);

        bus.write_raw(0x0500_0334, 10);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 10);

        bus.write_raw(0x0500_0734, 11);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 11);

        bus.write_raw(0x0501_2734, 12);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 12);

        bus.write_raw(0x05FF_FF34, 13);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 13);
    }

    #[test]
    fn test_mirror_vram() {
        let mut bus = Bus::default();
        bus.lcd.memory.video_ram[0x0000_9345] = 5;

        assert_eq!(bus.read_raw(0x0600_9345), 5);
        assert_eq!(bus.read_raw(0x0602_9345), 5);
        assert_eq!(bus.read_raw(0x0612_9345), 5);
        assert_eq!(bus.read_raw(0x06FE_9345), 5);

        bus.write_raw(0x0600_9345, 1);
        assert_eq!(bus.lcd.memory.video_ram[0x0000_9345], 1);

        bus.write_raw(0x0602_9345, 2);
        assert_eq!(bus.lcd.memory.video_ram[0x0000_9345], 2);

        bus.write_raw(0x0612_9345, 3);
        assert_eq!(bus.lcd.memory.video_ram[0x0000_9345], 3);

        bus.write_raw(0x06FE_9345, 4);
        assert_eq!(bus.lcd.memory.video_ram[0x0000_9345], 4);

        bus.lcd.memory.video_ram[0x0001_1345] = 10;
        assert_eq!(bus.read_raw(0x0601_9345), 10);
        assert_eq!(bus.read_raw(0x0613_1345), 10);
    }

    #[test]
    fn test_mirror_oam() {
        let mut bus = Bus::default();
        bus.lcd.memory.obj_attributes[0x134] = 5;

        assert_eq!(bus.read_raw(0x0700_0134), 5);
        assert_eq!(bus.read_raw(0x0700_0534), 5);
        assert_eq!(bus.read_raw(0x0700_F534), 5);
        assert_eq!(bus.read_raw(0x07FF_FD34), 5);

        bus.write_raw(0x0700_0134, 10);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 10);

        bus.write_raw(0x0700_0534, 11);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 11);

        bus.write_raw(0x0700_F534, 12);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 12);

        bus.write_raw(0x07FF_FD34, 13);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 13);
    }

    #[test]
    fn test_timer_reload_vs_counter() {
        let mut bus = Bus::default();

        // Set reload value via write to TM0CNT_L
        bus.write_raw(0x0400_0100, 0x34); // low byte
        bus.write_raw(0x0400_0101, 0x12); // high byte

        // Reload value should be set
        assert_eq!(bus.timers.tm0_reload, 0x1234);

        // Counter should still be 0 (reload only takes effect when timer starts)
        assert_eq!(bus.timers.tm0cnt_l, 0);

        // Reading TM0CNT_L returns counter value, not reload
        assert_eq!(bus.read_raw(0x0400_0100), 0);
        assert_eq!(bus.read_raw(0x0400_0101), 0);
    }

    #[test]
    fn test_timer_control_write() {
        let mut bus = Bus::default();

        // Write control register TM0CNT_H
        bus.write_raw(0x0400_0102, 0x80); // Enable timer (bit 7)
        assert!(bus.timers.tm0cnt_h & 0x80 != 0);

        // Write prescaler value
        bus.write_raw(0x0400_0102, 0x01); // Prescaler F/64
        assert_eq!(bus.timers.tm0cnt_h & 0x03, 0x01);
    }

    #[test]
    fn test_interrupt_request_acknowledge() {
        let mut bus = Bus::default();

        // Set some interrupt request flags directly
        bus.interrupt_control.interrupt_request = 0b0000_0000_0000_0111; // VBlank, HBlank, VCount

        // Verify flags are set
        assert_eq!(bus.read_raw(0x0400_0202), 0x07);

        // Acknowledge VBlank by writing 1 to bit 0
        bus.write_raw(0x0400_0202, 0x01);

        // VBlank flag should be cleared, others remain
        assert_eq!(
            bus.interrupt_control.interrupt_request,
            0b0000_0000_0000_0110
        );

        // Acknowledge remaining flags
        bus.write_raw(0x0400_0202, 0x06);
        assert_eq!(bus.interrupt_control.interrupt_request, 0);
    }

    #[test]
    fn test_interrupt_enable_read_write() {
        let mut bus = Bus::default();

        // Write to interrupt enable register
        bus.write_raw(0x0400_0200, 0xFF);
        bus.write_raw(0x0400_0201, 0x3F);

        assert_eq!(bus.interrupt_control.interrupt_enable, 0x3FFF);

        // Read it back
        assert_eq!(bus.read_raw(0x0400_0200), 0xFF);
        assert_eq!(bus.read_raw(0x0400_0201), 0x3F);
    }

    #[test]
    fn test_interrupt_master_enable() {
        let mut bus = Bus::default();

        // IME is disabled by default
        assert_eq!(bus.interrupt_control.interrupt_master_enable, 0);

        // Enable IME
        bus.write_raw(0x0400_0208, 0x01);
        assert_eq!(bus.interrupt_control.interrupt_master_enable, 1);

        // Read it back
        assert_eq!(bus.read_raw(0x0400_0208), 0x01);

        // Disable IME
        bus.write_raw(0x0400_0208, 0x00);
        assert_eq!(bus.interrupt_control.interrupt_master_enable, 0);
    }
}
