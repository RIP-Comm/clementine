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
            0x04000200 => self.interrupt_control.interrupt_enable.set_byte(0, value),
            0x04000201 => self.interrupt_control.interrupt_enable.set_byte(1, value),
            0x04000202 => {
                // Writing 1 to a bit clears it (acknowledges the interrupt)
                self.interrupt_control.interrupt_request &= !(value as u16);
            }
            0x04000203 => {
                // Writing 1 to a bit clears it (acknowledges the interrupt)
                self.interrupt_control.interrupt_request &= !((value as u16) << 8);
            }
            0x04000204 => self.interrupt_control.wait_state_control.set_byte(0, value),
            0x04000205 => self.interrupt_control.wait_state_control.set_byte(1, value),
            0x04000208 => {
                self.interrupt_control
                    .interrupt_master_enable
                    .set_byte(0, value);
            }
            0x04000209 => {
                self.interrupt_control
                    .interrupt_master_enable
                    .set_byte(1, value);
            }
            0x04000300 => self.interrupt_control.post_boot_flag.set_byte(0, value),
            0x04000301 => self.interrupt_control.power_down_control.set_byte(0, value),
            0x04000410 => self.interrupt_control.purpose_unknown.set_byte(0, value),
            0x04000206
            | 0x04000207
            | 0x400020A..=0x40002FF
            | 0x04000302..=0x0400040F
            | 0x04000411 => {
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
            0x4000130 => self.keypad.key_input.get_byte(0),
            0x4000131 => self.keypad.key_input.get_byte(1),
            0x4000132 => self.keypad.key_interrupt_control.get_byte(0),
            0x4000133 => self.keypad.key_interrupt_control.get_byte(1),
            _ => panic!("Keypad read address is out of bound"),
        }
    }

    fn write_keypad_raw(&mut self, address: usize, value: u8) {
        match address {
            // 0x4000130 and 0x4000131 Should be read-only but CPU bios writes it.
            0x4000130 => self.keypad.key_input.set_byte(0, value),
            0x4000131 => self.keypad.key_input.set_byte(1, value),
            0x4000132 => self.keypad.key_interrupt_control.set_byte(0, value),
            0x4000133 => self.keypad.key_interrupt_control.set_byte(1, value),
            _ => panic!("Keypad write address is out of bound"),
        }
    }

    fn read_serial_raw(&self, address: usize) -> u8 {
        match address {
            0x04000120 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(0),
            0x04000121 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(1),
            0x04000122 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(2),
            0x04000123 => self.serial.sio_data_32_multi_data_0_data_1.get_byte(3),
            0x04000124 => self.serial.sio_multi_data_2.get_byte(0),
            0x04000125 => self.serial.sio_multi_data_2.get_byte(1),
            0x04000126 => self.serial.sio_multi_data_3.get_byte(0),
            0x04000127 => self.serial.sio_multi_data_3.get_byte(1),
            0x04000128 => self.serial.sio_control_register.get_byte(0),
            0x04000129 => self.serial.sio_control_register.get_byte(1),
            0x0400012A => self.serial.sio_multi_data_send_data_8.get_byte(0),
            0x0400012B => self.serial.sio_multi_data_send_data_8.get_byte(1),
            0x04000134 => self.serial.sio_mode_select.get_byte(0),
            0x04000135 => self.serial.sio_mode_select.get_byte(1),
            0x04000136 => self.serial.infrared_register.get_byte(0),
            0x04000137 => self.serial.infrared_register.get_byte(1),
            0x04000140 => self.serial.sio_joy_bus_control.get_byte(0),
            0x04000141 => self.serial.sio_joy_bus_control.get_byte(1),
            0x04000150 => self.serial.sio_joy_bus_receive_data.get_byte(0),
            0x04000151 => self.serial.sio_joy_bus_receive_data.get_byte(1),
            0x04000152 => self.serial.sio_joy_bus_receive_data.get_byte(2),
            0x04000153 => self.serial.sio_joy_bus_receive_data.get_byte(3),
            0x04000154 => self.serial.sio_joy_bus_transmit_data.get_byte(0),
            0x04000155 => self.serial.sio_joy_bus_transmit_data.get_byte(1),
            0x04000156 => self.serial.sio_joy_bus_transmit_data.get_byte(2),
            0x04000157 => self.serial.sio_joy_bus_transmit_data.get_byte(3),
            0x04000158 => self.serial.sio_joy_bus_receive_status.get_byte(0),
            0x04000159 => self.serial.sio_joy_bus_receive_status.get_byte(1),
            0x0400012C..=0x0400012F
            | 0x04000138..=0x04000141
            | 0x04000142..=0x0400014F
            | 0x0400015A..=0x040001FF => {
                log(format!("read on unused memory {address:x}"));
                *self.unused_region.get(&address).unwrap_or(&0)
            }
            _ => panic!("Serial read address is out of bound: {address:#010x}"),
        }
    }

    fn write_serial_raw(&mut self, address: usize, value: u8) {
        match address {
            0x04000120 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(0, value),
            0x04000121 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(1, value),
            0x04000122 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(2, value),
            0x04000123 => self
                .serial
                .sio_data_32_multi_data_0_data_1
                .set_byte(3, value),
            0x04000124 => self.serial.sio_multi_data_2.set_byte(0, value),
            0x04000125 => self.serial.sio_multi_data_2.set_byte(1, value),
            0x04000126 => self.serial.sio_multi_data_3.set_byte(0, value),
            0x04000127 => self.serial.sio_multi_data_3.set_byte(1, value),
            0x04000128 => self.serial.sio_control_register.set_byte(0, value),
            0x04000129 => self.serial.sio_control_register.set_byte(1, value),
            0x0400012A => self.serial.sio_multi_data_send_data_8.set_byte(0, value),
            0x0400012B => self.serial.sio_multi_data_send_data_8.set_byte(1, value),
            0x04000134 => self.serial.sio_mode_select.set_byte(0, value),
            0x04000135 => self.serial.sio_mode_select.set_byte(1, value),
            0x04000136 => self.serial.infrared_register.set_byte(0, value),
            0x04000137 => self.serial.infrared_register.set_byte(1, value),
            0x04000140 => self.serial.sio_joy_bus_control.set_byte(0, value),
            0x04000141 => self.serial.sio_joy_bus_control.set_byte(1, value),
            0x04000150 => self.serial.sio_joy_bus_receive_data.set_byte(0, value),
            0x04000151 => self.serial.sio_joy_bus_receive_data.set_byte(1, value),
            0x04000152 => self.serial.sio_joy_bus_receive_data.set_byte(2, value),
            0x04000153 => self.serial.sio_joy_bus_receive_data.set_byte(3, value),
            0x04000154 => self.serial.sio_joy_bus_transmit_data.set_byte(0, value),
            0x04000155 => self.serial.sio_joy_bus_transmit_data.set_byte(1, value),
            0x04000156 => self.serial.sio_joy_bus_transmit_data.set_byte(2, value),
            0x04000157 => self.serial.sio_joy_bus_transmit_data.set_byte(3, value),
            0x04000158 => self.serial.sio_joy_bus_receive_status.set_byte(0, value),
            0x04000159 => self.serial.sio_joy_bus_receive_status.set_byte(1, value),
            0x0400012C..=0x0400012F
            | 0x04000138..=0x04000139
            | 0x04000142..=0x0400014F
            | 0x0400015A..=0x040001FF => {
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
            0x04000100 => self.timers.tm0cnt_l.get_byte(0),
            0x04000101 => self.timers.tm0cnt_l.get_byte(1),
            0x04000102 => self.timers.tm0cnt_h.get_byte(0),
            0x04000103 => self.timers.tm0cnt_h.get_byte(1),
            0x04000104 => self.timers.tm1cnt_l.get_byte(0),
            0x04000105 => self.timers.tm1cnt_l.get_byte(1),
            0x04000106 => self.timers.tm1cnt_h.get_byte(0),
            0x04000107 => self.timers.tm1cnt_h.get_byte(1),
            0x04000108 => self.timers.tm2cnt_l.get_byte(0),
            0x04000109 => self.timers.tm2cnt_l.get_byte(1),
            0x0400010A => self.timers.tm2cnt_h.get_byte(0),
            0x0400010B => self.timers.tm2cnt_h.get_byte(1),
            0x0400010C => self.timers.tm3cnt_l.get_byte(0),
            0x0400010D => self.timers.tm3cnt_l.get_byte(1),
            0x0400010E => self.timers.tm3cnt_h.get_byte(0),
            0x0400010F => self.timers.tm3cnt_h.get_byte(1),
            0x04000110..=0x0400011F => self.unused_region.get(&address).map_or(0, |v| *v),
            _ => panic!("Timers read address is out of bound"),
        }
    }

    fn write_timers_raw(&mut self, address: usize, value: u8) {
        match address {
            // Timer 0 reload (writing to CNT_L sets reload value, not counter)
            0x04000100 => {
                let mut reload = self.timers.tm0_reload; // Use reload as base for byte write
                reload.set_byte(0, value);
                self.timers.set_reload(0, reload);
            }
            0x04000101 => {
                let mut reload = self.timers.tm0_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(0, reload);
            }
            // Timer 0 control
            0x04000102 => {
                let mut control = self.timers.tm0cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(0, control);
            }
            0x04000103 => {
                let mut control = self.timers.tm0cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(0, control);
            }
            // Timer 1 reload
            0x04000104 => {
                let mut reload = self.timers.tm1_reload;
                reload.set_byte(0, value);
                self.timers.set_reload(1, reload);
            }
            0x04000105 => {
                let mut reload = self.timers.tm1_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(1, reload);
            }
            // Timer 1 control
            0x04000106 => {
                let mut control = self.timers.tm1cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(1, control);
            }
            0x04000107 => {
                let mut control = self.timers.tm1cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(1, control);
            }
            // Timer 2 reload
            0x04000108 => {
                let mut reload = self.timers.tm2_reload;
                reload.set_byte(0, value);
                self.timers.set_reload(2, reload);
            }
            0x04000109 => {
                let mut reload = self.timers.tm2_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(2, reload);
            }
            // Timer 2 control
            0x0400010A => {
                let mut control = self.timers.tm2cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(2, control);
            }
            0x0400010B => {
                let mut control = self.timers.tm2cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(2, control);
            }
            // Timer 3 reload
            0x0400010C => {
                let mut reload = self.timers.tm3_reload;
                reload.set_byte(0, value);
                self.timers.set_reload(3, reload);
            }
            0x0400010D => {
                let mut reload = self.timers.tm3_reload;
                reload.set_byte(1, value);
                self.timers.set_reload(3, reload);
            }
            // Timer 3 control
            0x0400010E => {
                let mut control = self.timers.tm3cnt_h;
                control.set_byte(0, value);
                self.timers.set_control(3, control);
            }
            0x0400010F => {
                let mut control = self.timers.tm3cnt_h;
                control.set_byte(1, value);
                self.timers.set_control(3, control);
            }
            0x04000110..=0x0400011F => {
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
            0x040000B0..=0x040000BB => read_dma_bank(&self.dma.channels[0], address - 0x040000B0),
            0x040000BC..=0x040000C7 => read_dma_bank(&self.dma.channels[1], address - 0x040000BC),
            0x040000C8..=0x040000D3 => read_dma_bank(&self.dma.channels[2], address - 0x040000C8),
            0x040000D4..=0x040000DF => read_dma_bank(&self.dma.channels[3], address - 0x040000D4),
            0x040000E0..=0x040000FF => {
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
            0x040000B0..=0x040000BB => {
                write_dma_bank(&mut self.dma.channels[0], address - 0x040000B0, value);
            }
            0x040000BC..=0x040000C7 => {
                write_dma_bank(&mut self.dma.channels[1], address - 0x040000BC, value);
            }
            0x040000C8..=0x040000D3 => {
                write_dma_bank(&mut self.dma.channels[2], address - 0x040000C8, value);
            }
            0x040000D4..=0x040000DF => {
                write_dma_bank(&mut self.dma.channels[3], address - 0x040000D4, value);
            }
            0x040000E0..=0x040000FF => {
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
            0x04000060 => self.sound.channel1_sweep.get_byte(0),
            0x04000061 => self.sound.channel1_sweep.get_byte(1),
            0x04000062 => self.sound.channel1_duty_length_envelope.get_byte(0),
            0x04000063 => self.sound.channel1_duty_length_envelope.get_byte(1),
            0x04000064 => self.sound.channel1_frequency_control.get_byte(0),
            0x04000065 => self.sound.channel1_frequency_control.get_byte(1),
            0x04000068 => self.sound.channel2_duty_length_envelope.get_byte(0),
            0x04000069 => self.sound.channel2_duty_length_envelope.get_byte(1),
            0x0400006C => self.sound.channel2_frequency_control.get_byte(0),
            0x0400006D => self.sound.channel2_frequency_control.get_byte(1),
            0x04000070 => self.sound.channel3_stop_wave_ram_select.get_byte(0),
            0x04000071 => self.sound.channel3_stop_wave_ram_select.get_byte(1),
            0x04000072 => self.sound.channel3_length_volume.get_byte(0),
            0x04000073 => self.sound.channel3_length_volume.get_byte(1),
            0x04000074 => self.sound.channel3_frequency_control.get_byte(0),
            0x04000075 => self.sound.channel3_frequency_control.get_byte(1),
            0x04000078 => self.sound.channel4_length_envelope.get_byte(0),
            0x04000079 => self.sound.channel4_length_envelope.get_byte(1),
            0x0400007C => self.sound.channel4_frequency_control.get_byte(0),
            0x0400007D => self.sound.channel4_frequency_control.get_byte(1),
            0x04000080 => self.sound.control_stereo_volume_enable.get_byte(0),
            0x04000081 => self.sound.control_stereo_volume_enable.get_byte(1),
            0x04000082 => self.sound.control_mixing_dma_control.get_byte(0),
            0x04000083 => self.sound.control_mixing_dma_control.get_byte(1),
            0x04000084 => self.sound.control_sound_on_off.get_byte(0),
            0x04000085 => self.sound.control_sound_on_off.get_byte(1),
            0x04000088 => self.sound.sound_pwm_control.get_byte(0),
            0x04000089 => self.sound.sound_pwm_control.get_byte(1),
            0x04000090..=0x0400009F => self.sound.channel3_wave_pattern_ram[address - 0x04000090],
            0x040000A0 => self.sound.channel_a_fifo.get_byte(0),
            0x040000A1 => self.sound.channel_a_fifo.get_byte(1),
            0x040000A2 => self.sound.channel_a_fifo.get_byte(2),
            0x040000A3 => self.sound.channel_a_fifo.get_byte(3),
            0x040000A4 => self.sound.channel_b_fifo.get_byte(0),
            0x040000A5 => self.sound.channel_b_fifo.get_byte(1),
            0x040000A6 => self.sound.channel_b_fifo.get_byte(2),
            0x040000A7 => self.sound.channel_b_fifo.get_byte(3),
            0x04000066..=0x04000067
            | 0x0400006A..=0x0400006B
            | 0x0400006E..=0x0400006F
            | 0x04000076..=0x04000077
            | 0x0400007A..=0x0400007B
            | 0x0400007E..=0x0400007F
            | 0x04000086..=0x04000087
            | 0x0400008A..=0x0400008F
            | 0x040000A8..=0x040000AF => {
                log(format!("read on unused memory {address:x}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("Sound read address is out of bound"),
        }
    }

    fn write_sound_raw(&mut self, address: usize, value: u8) {
        match address {
            0x04000060 => self.sound.channel1_sweep.set_byte(0, value),
            0x04000061 => self.sound.channel1_sweep.set_byte(1, value),
            0x04000062 => self.sound.channel1_duty_length_envelope.set_byte(0, value),
            0x04000063 => self.sound.channel1_duty_length_envelope.set_byte(1, value),
            0x04000064 => self.sound.channel1_frequency_control.set_byte(0, value),
            0x04000065 => self.sound.channel1_frequency_control.set_byte(1, value),
            0x04000068 => self.sound.channel2_duty_length_envelope.set_byte(0, value),
            0x04000069 => self.sound.channel2_duty_length_envelope.set_byte(1, value),
            0x0400006C => self.sound.channel2_frequency_control.set_byte(0, value),
            0x0400006D => self.sound.channel2_frequency_control.set_byte(1, value),
            0x04000070 => self.sound.channel3_stop_wave_ram_select.set_byte(0, value),
            0x04000071 => self.sound.channel3_stop_wave_ram_select.set_byte(1, value),
            0x04000072 => self.sound.channel3_length_volume.set_byte(0, value),
            0x04000073 => self.sound.channel3_length_volume.set_byte(1, value),
            0x04000074 => self.sound.channel3_frequency_control.set_byte(0, value),
            0x04000075 => self.sound.channel3_frequency_control.set_byte(1, value),
            0x04000078 => self.sound.channel4_length_envelope.set_byte(0, value),
            0x04000079 => self.sound.channel4_length_envelope.set_byte(1, value),
            0x0400007C => self.sound.channel4_frequency_control.set_byte(0, value),
            0x0400007D => self.sound.channel4_frequency_control.set_byte(1, value),
            0x04000080 => self.sound.control_stereo_volume_enable.set_byte(0, value),
            0x04000081 => self.sound.control_stereo_volume_enable.set_byte(1, value),
            0x04000082 => self.sound.control_mixing_dma_control.set_byte(0, value),
            0x04000083 => self.sound.control_mixing_dma_control.set_byte(1, value),
            0x04000084 => self.sound.control_sound_on_off.set_byte(0, value),
            0x04000085 => self.sound.control_sound_on_off.set_byte(1, value),
            0x04000088 => self.sound.sound_pwm_control.set_byte(0, value),
            0x04000089 => self.sound.sound_pwm_control.set_byte(1, value),
            0x04000090..=0x0400009F => {
                self.sound.channel3_wave_pattern_ram[address - 0x04000090] = value;
            }
            0x040000A0 => self.sound.channel_a_fifo.set_byte(0, value),
            0x040000A1 => self.sound.channel_a_fifo.set_byte(1, value),
            0x040000A2 => self.sound.channel_a_fifo.set_byte(2, value),
            0x040000A3 => self.sound.channel_a_fifo.set_byte(3, value),
            0x040000A4 => self.sound.channel_b_fifo.set_byte(0, value),
            0x040000A5 => self.sound.channel_b_fifo.set_byte(1, value),
            0x040000A6 => self.sound.channel_b_fifo.set_byte(2, value),
            0x040000A7 => self.sound.channel_b_fifo.set_byte(3, value),
            0x04000066..=0x04000067
            | 0x0400006A..=0x0400006B
            | 0x0400006E..=0x0400006F
            | 0x04000076..=0x04000077
            | 0x0400007A..=0x0400007B
            | 0x0400007E..=0x0400007F
            | 0x04000086..=0x04000087
            | 0x0400008A..=0x0400008F
            | 0x040000A8..=0x040000AF => {
                log(format!("write on unused memory, {address:x}"));
                self.unused_region.insert(address, value);
            }
            _ => panic!("Sound write address is out of bound"),
        }
    }

    fn read_lcd_raw(&self, address: usize) -> u8 {
        match address {
            0x04000000 => self.lcd.registers.dispcnt.get_byte(0),
            0x04000001 => self.lcd.registers.dispcnt.get_byte(1),
            0x04000002 => self.lcd.registers.green_swap.get_byte(0),
            0x04000003 => self.lcd.registers.green_swap.get_byte(1),
            0x04000004 => self.lcd.registers.dispstat.get_byte(0),
            0x04000005 => self.lcd.registers.dispstat.get_byte(1),
            0x04000006 => self.lcd.registers.vcount.get_byte(0),
            0x04000007 => self.lcd.registers.vcount.get_byte(1),
            0x04000008 => self.lcd.registers.bg0cnt.get_byte(0),
            0x04000009 => self.lcd.registers.bg0cnt.get_byte(1),
            0x0400000A => self.lcd.registers.bg1cnt.get_byte(0),
            0x0400000B => self.lcd.registers.bg1cnt.get_byte(1),
            0x0400000C => self.lcd.registers.bg2cnt.get_byte(0),
            0x0400000D => self.lcd.registers.bg2cnt.get_byte(1),
            0x0400000E => self.lcd.registers.bg3cnt.get_byte(0),
            0x0400000F => self.lcd.registers.bg3cnt.get_byte(1),
            0x04000010 => self.lcd.registers.bg0hofs.get_byte(0),
            0x04000011 => self.lcd.registers.bg0hofs.get_byte(1),
            0x04000012 => self.lcd.registers.bg0vofs.get_byte(0),
            0x04000013 => self.lcd.registers.bg0vofs.get_byte(1),
            0x04000014 => self.lcd.registers.bg1hofs.get_byte(0),
            0x04000015 => self.lcd.registers.bg1hofs.get_byte(1),
            0x04000016 => self.lcd.registers.bg1vofs.get_byte(0),
            0x04000017 => self.lcd.registers.bg1vofs.get_byte(1),
            0x04000018 => self.lcd.registers.bg2hofs.get_byte(0),
            0x04000019 => self.lcd.registers.bg2hofs.get_byte(1),
            0x0400001A => self.lcd.registers.bg2vofs.get_byte(0),
            0x0400001B => self.lcd.registers.bg2vofs.get_byte(1),
            0x0400001C => self.lcd.registers.bg3hofs.get_byte(0),
            0x0400001D => self.lcd.registers.bg3hofs.get_byte(1),
            0x0400001E => self.lcd.registers.bg3vofs.get_byte(0),
            0x0400001F => self.lcd.registers.bg3vofs.get_byte(1),
            0x04000020 => self.lcd.registers.bg2pa.get_byte(0),
            0x04000021 => self.lcd.registers.bg2pa.get_byte(1),
            0x04000022 => self.lcd.registers.bg2pb.get_byte(0),
            0x04000023 => self.lcd.registers.bg2pb.get_byte(1),
            0x04000024 => self.lcd.registers.bg2pc.get_byte(0),
            0x04000025 => self.lcd.registers.bg2pc.get_byte(1),
            0x04000026 => self.lcd.registers.bg2pd.get_byte(0),
            0x04000027 => self.lcd.registers.bg2pd.get_byte(1),
            0x04000028 => self.lcd.registers.bg2x.get_byte(0),
            0x04000029 => self.lcd.registers.bg2x.get_byte(1),
            0x0400002A => self.lcd.registers.bg2x.get_byte(2),
            0x0400002B => self.lcd.registers.bg2x.get_byte(3),
            0x0400002C => self.lcd.registers.bg2y.get_byte(0),
            0x0400002D => self.lcd.registers.bg2y.get_byte(1),
            0x0400002E => self.lcd.registers.bg2y.get_byte(2),
            0x0400002F => self.lcd.registers.bg2y.get_byte(3),
            0x04000030 => self.lcd.registers.bg3pa.get_byte(0),
            0x04000031 => self.lcd.registers.bg3pa.get_byte(1),
            0x04000032 => self.lcd.registers.bg3pb.get_byte(0),
            0x04000033 => self.lcd.registers.bg3pb.get_byte(1),
            0x04000034 => self.lcd.registers.bg3pc.get_byte(0),
            0x04000035 => self.lcd.registers.bg3pc.get_byte(1),
            0x04000036 => self.lcd.registers.bg3pd.get_byte(0),
            0x04000037 => self.lcd.registers.bg3pd.get_byte(1),
            0x04000038 => self.lcd.registers.bg3x.get_byte(0),
            0x04000039 => self.lcd.registers.bg3x.get_byte(1),
            0x0400003A => self.lcd.registers.bg3x.get_byte(2),
            0x0400003B => self.lcd.registers.bg3x.get_byte(3),
            0x0400003C => self.lcd.registers.bg3y.get_byte(0),
            0x0400003D => self.lcd.registers.bg3y.get_byte(1),
            0x0400003E => self.lcd.registers.bg3y.get_byte(2),
            0x0400003F => self.lcd.registers.bg3y.get_byte(3),
            0x04000040 => self.lcd.registers.win0h.get_byte(0),
            0x04000041 => self.lcd.registers.win0h.get_byte(1),
            0x04000042 => self.lcd.registers.win1h.get_byte(0),
            0x04000043 => self.lcd.registers.win1h.get_byte(1),
            0x04000044 => self.lcd.registers.win0v.get_byte(0),
            0x04000045 => self.lcd.registers.win0v.get_byte(1),
            0x04000046 => self.lcd.registers.win1v.get_byte(0),
            0x04000047 => self.lcd.registers.win1v.get_byte(1),
            0x04000048 => self.lcd.registers.winin.get_byte(0),
            0x04000049 => self.lcd.registers.winin.get_byte(1),
            0x0400004A => self.lcd.registers.winout.get_byte(0),
            0x0400004B => self.lcd.registers.winout.get_byte(1),
            0x0400004C => self.lcd.registers.mosaic.get_byte(0),
            0x0400004D => self.lcd.registers.mosaic.get_byte(1),
            0x04000050 => self.lcd.registers.bldcnt.get_byte(0),
            0x04000051 => self.lcd.registers.bldcnt.get_byte(1),
            0x04000052 => self.lcd.registers.bldalpha.get_byte(0),
            0x04000053 => self.lcd.registers.bldalpha.get_byte(1),
            0x04000054 => self.lcd.registers.bldy.get_byte(0),
            0x04000055 => self.lcd.registers.bldy.get_byte(1),
            0x0400004E..=0x0400004F | 0x04000056..=0x0400005F => {
                log(format!("read on unused memory 0x{address:08X}"));
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("LCD read address is out of bound"),
        }
    }

    fn write_lcd_raw(&mut self, address: usize, value: u8) {
        match address {
            0x04000000 => {
                self.lcd.registers.dispcnt.set_byte(0, value);
            }
            0x04000001 => {
                self.lcd.registers.dispcnt.set_byte(1, value);
            }
            0x04000002 => self.lcd.registers.green_swap.set_byte(0, value),
            0x04000003 => self.lcd.registers.green_swap.set_byte(1, value),
            0x04000004 => self.lcd.registers.dispstat.set_byte(0, value),
            0x04000005 => self.lcd.registers.dispstat.set_byte(1, value),
            0x04000008 => self.lcd.registers.bg0cnt.set_byte(0, value),
            0x04000006 => self.lcd.registers.vcount.set_byte(0, value),
            0x04000007 => self.lcd.registers.vcount.set_byte(1, value),
            0x04000009 => self.lcd.registers.bg0cnt.set_byte(1, value),
            0x0400000A => self.lcd.registers.bg1cnt.set_byte(0, value),
            0x0400000B => self.lcd.registers.bg1cnt.set_byte(1, value),
            0x0400000C => self.lcd.registers.bg2cnt.set_byte(0, value),
            0x0400000D => self.lcd.registers.bg2cnt.set_byte(1, value),
            0x0400000E => self.lcd.registers.bg3cnt.set_byte(0, value),
            0x0400000F => self.lcd.registers.bg3cnt.set_byte(1, value),
            0x04000010 => self.lcd.registers.bg0hofs.set_byte(0, value),
            0x04000011 => self.lcd.registers.bg0hofs.set_byte(1, value),
            0x04000012 => self.lcd.registers.bg0vofs.set_byte(0, value),
            0x04000013 => self.lcd.registers.bg0vofs.set_byte(1, value),
            0x04000014 => self.lcd.registers.bg1hofs.set_byte(0, value),
            0x04000015 => self.lcd.registers.bg1hofs.set_byte(1, value),
            0x04000016 => self.lcd.registers.bg1vofs.set_byte(0, value),
            0x04000017 => self.lcd.registers.bg1vofs.set_byte(1, value),
            0x04000018 => self.lcd.registers.bg2hofs.set_byte(0, value),
            0x04000019 => self.lcd.registers.bg2hofs.set_byte(1, value),
            0x0400001A => self.lcd.registers.bg2vofs.set_byte(0, value),
            0x0400001B => self.lcd.registers.bg2vofs.set_byte(1, value),
            0x0400001C => self.lcd.registers.bg3hofs.set_byte(0, value),
            0x0400001D => self.lcd.registers.bg3hofs.set_byte(1, value),
            0x0400001E => self.lcd.registers.bg3vofs.set_byte(0, value),
            0x0400001F => self.lcd.registers.bg3vofs.set_byte(1, value),
            0x04000020 => self.lcd.registers.bg2pa.set_byte(0, value),
            0x04000021 => self.lcd.registers.bg2pa.set_byte(1, value),
            0x04000022 => self.lcd.registers.bg2pb.set_byte(0, value),
            0x04000023 => self.lcd.registers.bg2pb.set_byte(1, value),
            0x04000024 => self.lcd.registers.bg2pc.set_byte(0, value),
            0x04000025 => self.lcd.registers.bg2pc.set_byte(1, value),
            0x04000026 => self.lcd.registers.bg2pd.set_byte(0, value),
            0x04000027 => self.lcd.registers.bg2pd.set_byte(1, value),
            0x04000028 => self.lcd.registers.bg2x.set_byte(0, value),
            0x04000029 => self.lcd.registers.bg2x.set_byte(1, value),
            0x0400002A => self.lcd.registers.bg2x.set_byte(2, value),
            0x0400002B => self.lcd.registers.bg2x.set_byte(3, value),
            0x0400002C => self.lcd.registers.bg2y.set_byte(0, value),
            0x0400002D => self.lcd.registers.bg2y.set_byte(1, value),
            0x0400002E => self.lcd.registers.bg2y.set_byte(2, value),
            0x0400002F => self.lcd.registers.bg2y.set_byte(3, value),
            0x04000030 => self.lcd.registers.bg3pa.set_byte(0, value),
            0x04000031 => self.lcd.registers.bg3pa.set_byte(1, value),
            0x04000032 => self.lcd.registers.bg3pb.set_byte(0, value),
            0x04000033 => self.lcd.registers.bg3pb.set_byte(1, value),
            0x04000034 => self.lcd.registers.bg3pc.set_byte(0, value),
            0x04000035 => self.lcd.registers.bg3pc.set_byte(1, value),
            0x04000036 => self.lcd.registers.bg3pd.set_byte(0, value),
            0x04000037 => self.lcd.registers.bg3pd.set_byte(1, value),
            0x04000038 => self.lcd.registers.bg3x.set_byte(0, value),
            0x04000039 => self.lcd.registers.bg3x.set_byte(1, value),
            0x0400003A => self.lcd.registers.bg3x.set_byte(2, value),
            0x0400003B => self.lcd.registers.bg3x.set_byte(3, value),
            0x0400003C => self.lcd.registers.bg3y.set_byte(0, value),
            0x0400003D => self.lcd.registers.bg3y.set_byte(1, value),
            0x0400003E => self.lcd.registers.bg3y.set_byte(2, value),
            0x0400003F => self.lcd.registers.bg3y.set_byte(3, value),
            0x04000040 => self.lcd.registers.win0h.set_byte(0, value),
            0x04000041 => self.lcd.registers.win0h.set_byte(1, value),
            0x04000042 => self.lcd.registers.win1h.set_byte(0, value),
            0x04000043 => self.lcd.registers.win1h.set_byte(1, value),
            0x04000044 => self.lcd.registers.win0v.set_byte(0, value),
            0x04000045 => self.lcd.registers.win0v.set_byte(1, value),
            0x04000046 => self.lcd.registers.win1v.set_byte(0, value),
            0x04000047 => self.lcd.registers.win1v.set_byte(1, value),
            0x04000048 => self.lcd.registers.winin.set_byte(0, value),
            0x04000049 => self.lcd.registers.winin.set_byte(1, value),
            0x0400004A => self.lcd.registers.winout.set_byte(0, value),
            0x0400004B => self.lcd.registers.winout.set_byte(1, value),
            0x0400004C => self.lcd.registers.mosaic.set_byte(0, value),
            0x0400004D => self.lcd.registers.mosaic.set_byte(1, value),
            // 0x0400004E, 0x0400004F are not used
            0x04000050 => self.lcd.registers.bldcnt.set_byte(0, value),
            0x04000051 => self.lcd.registers.bldcnt.set_byte(1, value),
            0x04000052 => self.lcd.registers.bldalpha.set_byte(0, value),
            0x04000053 => self.lcd.registers.bldalpha.set_byte(1, value),
            0x04000054 => self.lcd.registers.bldy.set_byte(0, value),
            0x04000055 => self.lcd.registers.bldy.set_byte(1, value),
            0x0400004E..=0x0400004F | 0x04000056..=0x0400005F => {
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
            0x0000000..=0x0003FFF => {
                // BIOS read protection: if PC is outside BIOS, return last BIOS opcode
                if self.current_pc >= 0x4000 {
                    // Return the appropriate byte from last_bios_opcode
                    self.last_bios_opcode
                        .get_byte(u8::try_from(address & 0b11).unwrap())
                } else {
                    self.internal_memory.read_at(address)
                }
            }
            (0x2000000..=0x03FFFFFF) | (0x08000000..=0x0E00FFFF) => {
                self.internal_memory.read_at(address)
            }
            0x4000000..=0x400005F => self.read_lcd_raw(address),
            0x4000060..=0x40000AF => self.read_sound_raw(address),
            0x40000B0..=0x40000FF => self.read_dma_raw(address),
            0x4000100..=0x400011F => self.read_timers_raw(address),
            0x4000130..=0x4000133 => self.read_keypad_raw(address),
            0x4000120..=0x400012F | 0x4000134..=0x40001FF => self.read_serial_raw(address),
            0x4000200..=0x4FFFFFF => self.read_interrupt_control_raw(address),
            0x5000000..=0x5FFFFFF => {
                let unmasked_address = get_unmasked_address(address, 0x00FFFF00, 0xFF0000FF, 8, 4);

                match unmasked_address {
                    0x05000000..=0x050001FF => {
                        self.lcd.memory.bg_palette_ram[unmasked_address - 0x05000000]
                    }
                    0x05000200..=0x050003FF => {
                        self.lcd.memory.obj_palette_ram[unmasked_address - 0x05000200]
                    }
                    _ => unreachable!(),
                }
            }
            0x6000000..=0x6FFFFFF => {
                let unmasked_address = get_unmasked_address(address, 0x00FF0000, 0xFF00FFFF, 16, 2);

                // VRAM is 64k+32k+32k with the last two 32k being one mirrors of each other
                match unmasked_address {
                    0x06000000..=0x06017FFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x06000000]
                    }
                    0x06018000..=0x0601FFFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x06000000 - 0x8000]
                    }
                    _ => unreachable!(),
                }
            }
            0x7000000..=0x7FFFFFF => {
                let unmasked_address = get_unmasked_address(address, 0x00FFFF00, 0xFF0000FF, 8, 4);

                self.lcd.memory.obj_attributes[unmasked_address - 0x07000000]
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
            0x0000000..=0x0003FFF | 0x2000000..=0x03FFFFFF | 0x08000000..=0x0E00FFFF => {
                self.internal_memory.write_at(address, value);
            }
            0x4000000..=0x400005F => self.write_lcd_raw(address, value),
            0x4000060..=0x40000AF => self.write_sound_raw(address, value),
            0x40000B0..=0x40000FF => self.write_dma_raw(address, value),
            0x4000100..=0x400011F => self.write_timers_raw(address, value),
            0x4000120..=0x400012F | 0x4000134..=0x40001FF => self.write_serial_raw(address, value),
            0x4000130..=0x4000133 => self.write_keypad_raw(address, value),
            0x4000200..=0x4FFFFFF => self.write_interrupt_control_raw(address, value),
            0x5000000..=0x5FFFFFF => {
                let unmasked_address = get_unmasked_address(address, 0x00FFFF00, 0xFF0000FF, 8, 4);

                match unmasked_address {
                    0x05000000..=0x050001FF => {
                        self.lcd.memory.bg_palette_ram[unmasked_address - 0x05000000] = value;
                    }
                    0x05000200..=0x050003FF => {
                        self.lcd.memory.obj_palette_ram[unmasked_address - 0x05000200] = value;
                    }
                    _ => unreachable!(),
                }
            }
            0x6000000..=0x6FFFFFF => {
                let unmasked_address = get_unmasked_address(address, 0x00FF0000, 0xFF00FFFF, 16, 2);

                // VRAM is 64k+32k+32k with the last two 32k being one mirrors of each other
                match unmasked_address {
                    0x06000000..=0x06017FFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x06000000] = value;
                    }
                    0x06018000..=0x0601FFFF => {
                        self.lcd.memory.video_ram[unmasked_address - 0x06000000 - 0x8000] = value;
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
            0x7000000..=0x7FFFFFF => {
                log("OAM byte write ignored");
                return;
            }
            // VRAM byte writes: In bitmap modes, byte writes are duplicated to halfwords
            // and work throughout the framebuffer area. In tile modes, byte writes to
            // OBJ VRAM (0x06010000-0x06017FFF) have special behavior.
            // For now, allow byte writes to all of VRAM (96KB = 0x18000 bytes)
            0x6000000..=0x6FFFFFF => {
                let unmasked_address = get_unmasked_address(address, 0x00FF0000, 0xFF00FFFF, 16, 2);

                // Byte writes work throughout VRAM (duplicated as halfword)
                if unmasked_address < 0x06018000 {
                    // Write as halfword with byte duplicated, aligned to halfword boundary
                    let aligned_address = address & !1;
                    self.write_raw(aligned_address, value);
                    self.write_raw(aligned_address + 1, value);
                } else {
                    log(format!(
                        "VRAM byte write ignored (unmasked address 0x{unmasked_address:08X} >= 0x06018000)"
                    ));
                }
                return;
            }
            // in palette RAM byte writes are duplicated into halfwords
            0x5000000..=0x5FFFFFF => {
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
        let address = 0x04000048; // WININ lower byte

        bus.write_raw(address, 10);

        assert_eq!(bus.lcd.registers.winin, 10);

        let address = 0x04000049; // WININ higher byte

        bus.write_raw(address, 5);
        assert_eq!(bus.lcd.registers.winin, (5 << 8) | 10);
    }

    #[test]
    fn test_read_lcd_reg() {
        let mut bus = Bus::default();
        let address = 0x04000048; // WININ lower byte

        bus.lcd.registers.winin = (5 << 8) | 10;

        assert_eq!(bus.read_raw(address), 10);

        let address = 0x04000049; // WININ higher byte

        assert_eq!(bus.read_raw(address), 5);
    }

    #[test]
    fn test_write_timer_register() {
        let mut bus = Bus::default();
        let address = 0x04000100;

        // Writing to TM0CNT_L sets the reload value, not the counter directly
        bus.write_raw(address, 10);
        assert_eq!(bus.timers.tm0_reload, 10);
    }

    #[test]
    fn test_read_timer_register() {
        let mut bus = Bus::default();
        let address = 0x04000100;

        bus.timers.tm0cnt_l = (5 << 8) | 10;

        assert_eq!(bus.read_raw(address), 10);
    }

    #[test]
    fn write_bg_palette_ram() {
        let mut bus = Bus::default();
        let address = 0x05000008;

        bus.write_raw(address, 10);
        assert_eq!(bus.lcd.memory.bg_palette_ram[8], 10);
    }

    #[test]
    fn read_bg_palette_ram() {
        let mut bus = Bus::default();
        bus.lcd.memory.bg_palette_ram[8] = 15;

        let address = 0x05000008;
        let value = bus.read_raw(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_bg_palette_ram() {
        let mut bus = Bus::default();

        let address = 0x050001FF;
        bus.write_raw(address, 5);

        assert_eq!(bus.lcd.memory.bg_palette_ram[0x1FF], 5);
    }

    #[test]
    fn write_obj_palette_ram() {
        let mut bus = Bus::default();
        let address = 0x05000208;

        bus.write_raw(address, 10);
        assert_eq!(bus.lcd.memory.obj_palette_ram[8], 10);
    }

    #[test]
    fn read_obj_palette_ram() {
        let mut bus = Bus::default();
        bus.lcd.memory.obj_palette_ram[8] = 15;

        let address = 0x05000208;

        let value = bus.read_raw(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_obj_palette_ram() {
        let mut bus = Bus::default();

        let address = 0x050003FF;
        bus.write_raw(address, 5);

        assert_eq!(bus.lcd.memory.obj_palette_ram[0x1FF], 5);
    }

    #[test]
    fn write_vram() {
        let mut bus = Bus::default();
        let address = 0x06000004;

        bus.write_raw(address, 23);
        assert_eq!(bus.lcd.memory.video_ram[4], 23);
    }

    #[test]
    fn read_vram() {
        let mut bus = Bus::default();
        bus.lcd.memory.video_ram[4] = 15;

        let address = 0x06000004;
        let value = bus.read_raw(address);

        assert_eq!(value, 15);
    }

    #[test]
    fn test_last_byte_vram() {
        let mut bus = Bus::default();

        let address = 0x06017FFF;
        bus.write_raw(address, 5);

        assert_eq!(bus.lcd.memory.video_ram[0x17FFF], 5);
    }

    #[test]
    fn test_mirror_bg_palette() {
        let mut bus = Bus::default();
        bus.lcd.memory.bg_palette_ram[0x134] = 5;

        assert_eq!(bus.read_raw(0x05000134), 5);
        assert_eq!(bus.read_raw(0x05000534), 5);
        assert_eq!(bus.read_raw(0x05012534), 5);
        assert_eq!(bus.read_raw(0x05FFFD34), 5);

        bus.write_raw(0x05000134, 10);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 10);

        bus.write_raw(0x05000534, 11);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 11);

        bus.write_raw(0x05012534, 12);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 12);

        bus.write_raw(0x05FFFD34, 13);
        assert_eq!(bus.lcd.memory.bg_palette_ram[0x134], 13);
    }

    #[test]
    fn test_mirror_obj_palette() {
        let mut bus = Bus::default();
        bus.lcd.memory.obj_palette_ram[0x134] = 5;

        assert_eq!(bus.read_raw(0x05000334), 5);
        assert_eq!(bus.read_raw(0x05000734), 5);
        assert_eq!(bus.read_raw(0x05012734), 5);
        assert_eq!(bus.read_raw(0x05FFFF34), 5);

        bus.write_raw(0x05000334, 10);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 10);

        bus.write_raw(0x05000734, 11);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 11);

        bus.write_raw(0x05012734, 12);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 12);

        bus.write_raw(0x05FFFF34, 13);
        assert_eq!(bus.lcd.memory.obj_palette_ram[0x134], 13);
    }

    #[test]
    fn test_mirror_vram() {
        let mut bus = Bus::default();
        bus.lcd.memory.video_ram[0x09345] = 5;

        assert_eq!(bus.read_raw(0x06009345), 5);
        assert_eq!(bus.read_raw(0x06029345), 5);
        assert_eq!(bus.read_raw(0x06129345), 5);
        assert_eq!(bus.read_raw(0x06FE9345), 5);

        bus.write_raw(0x06009345, 1);
        assert_eq!(bus.lcd.memory.video_ram[0x09345], 1);

        bus.write_raw(0x06029345, 2);
        assert_eq!(bus.lcd.memory.video_ram[0x09345], 2);

        bus.write_raw(0x06129345, 3);
        assert_eq!(bus.lcd.memory.video_ram[0x09345], 3);

        bus.write_raw(0x06FE9345, 4);
        assert_eq!(bus.lcd.memory.video_ram[0x09345], 4);

        bus.lcd.memory.video_ram[0x11345] = 10;
        assert_eq!(bus.read_raw(0x06019345), 10);
        assert_eq!(bus.read_raw(0x06131345), 10);
    }

    #[test]
    fn test_mirror_oam() {
        let mut bus = Bus::default();
        bus.lcd.memory.obj_attributes[0x134] = 5;

        assert_eq!(bus.read_raw(0x07000134), 5);
        assert_eq!(bus.read_raw(0x07000534), 5);
        assert_eq!(bus.read_raw(0x0700F534), 5);
        assert_eq!(bus.read_raw(0x07FFFD34), 5);

        bus.write_raw(0x07000134, 10);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 10);

        bus.write_raw(0x07000534, 11);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 11);

        bus.write_raw(0x0700F534, 12);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 12);

        bus.write_raw(0x07FFFD34, 13);
        assert_eq!(bus.lcd.memory.obj_attributes[0x134], 13);
    }

    #[test]
    fn test_timer_reload_vs_counter() {
        let mut bus = Bus::default();

        // Set reload value via write to TM0CNT_L
        bus.write_raw(0x04000100, 0x34); // low byte
        bus.write_raw(0x04000101, 0x12); // high byte

        // Reload value should be set
        assert_eq!(bus.timers.tm0_reload, 0x1234);

        // Counter should still be 0 (reload only takes effect when timer starts)
        assert_eq!(bus.timers.tm0cnt_l, 0);

        // Reading TM0CNT_L returns counter value, not reload
        assert_eq!(bus.read_raw(0x04000100), 0);
        assert_eq!(bus.read_raw(0x04000101), 0);
    }

    #[test]
    fn test_timer_control_write() {
        let mut bus = Bus::default();

        // Write control register TM0CNT_H
        bus.write_raw(0x04000102, 0x80); // Enable timer (bit 7)
        assert!(bus.timers.tm0cnt_h & 0x80 != 0);

        // Write prescaler value
        bus.write_raw(0x04000102, 0x01); // Prescaler F/64
        assert_eq!(bus.timers.tm0cnt_h & 0x03, 0x01);
    }

    #[test]
    fn test_interrupt_request_acknowledge() {
        let mut bus = Bus::default();

        // Set some interrupt request flags directly
        bus.interrupt_control.interrupt_request = 0b0000_0000_0000_0111; // VBlank, HBlank, VCount

        // Verify flags are set
        assert_eq!(bus.read_raw(0x04000202), 0x07);

        // Acknowledge VBlank by writing 1 to bit 0
        bus.write_raw(0x04000202, 0x01);

        // VBlank flag should be cleared, others remain
        assert_eq!(
            bus.interrupt_control.interrupt_request,
            0b0000_0000_0000_0110
        );

        // Acknowledge remaining flags
        bus.write_raw(0x04000202, 0x06);
        assert_eq!(bus.interrupt_control.interrupt_request, 0);
    }

    #[test]
    fn test_interrupt_enable_read_write() {
        let mut bus = Bus::default();

        // Write to interrupt enable register
        bus.write_raw(0x04000200, 0xFF);
        bus.write_raw(0x04000201, 0x3F);

        assert_eq!(bus.interrupt_control.interrupt_enable, 0x3FFF);

        // Read it back
        assert_eq!(bus.read_raw(0x04000200), 0xFF);
        assert_eq!(bus.read_raw(0x04000201), 0x3F);
    }

    #[test]
    fn test_interrupt_master_enable() {
        let mut bus = Bus::default();

        // IME is disabled by default
        assert_eq!(bus.interrupt_control.interrupt_master_enable, 0);

        // Enable IME
        bus.write_raw(0x04000208, 0x01);
        assert_eq!(bus.interrupt_control.interrupt_master_enable, 1);

        // Read it back
        assert_eq!(bus.read_raw(0x04000208), 0x01);

        // Disable IME
        bus.write_raw(0x04000208, 0x00);
        assert_eq!(bus.interrupt_control.interrupt_master_enable, 0);
    }
}
