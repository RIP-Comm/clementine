use std::collections::HashMap;

use logger::log;

use crate::bitwise::Bits;

use super::{io_device::IoDevice, io_registers::IORegister};

pub struct Sound {
    channel1_sweep: IORegister,
    channel1_duty_length_envelope: IORegister,
    channel1_frequency_control: IORegister,
    channel2_duty_length_envelope: IORegister,
    channel2_frequency_control: IORegister,
    channel3_stop_wave_ram_select: IORegister,
    channel3_length_volume: IORegister,
    channel3_frequency_control: IORegister,
    channel4_length_envelope: IORegister,
    channel4_frequency_control: IORegister,
    control_stereo_volume_enable: IORegister,
    control_mixing_dma_control: IORegister,
    control_sound_on_off: IORegister,
    sound_pwm_control: IORegister,
    channel3_wave_pattern_ram: Vec<u8>,
    channel_a_fifo: IORegister,
    channel_b_fifo: IORegister,
    unused_region: HashMap<usize, u8>,
}

impl Default for Sound {
    fn default() -> Self {
        Self::new()
    }
}

impl Sound {
    pub fn new() -> Self {
        Self {
            channel1_sweep: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel1_duty_length_envelope: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel1_frequency_control: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel2_duty_length_envelope: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel2_frequency_control: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel3_stop_wave_ram_select: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel3_length_volume: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel3_frequency_control: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel4_length_envelope: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel4_frequency_control: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            control_stereo_volume_enable: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            control_mixing_dma_control: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            control_sound_on_off: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            sound_pwm_control: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel3_wave_pattern_ram: vec![0; 16],
            channel_a_fifo: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            channel_b_fifo: IORegister::with_access_control(
                super::io_registers::IORegisterAccessControl::ReadWrite,
            ),
            unused_region: HashMap::default(),
        }
    }
}

impl IoDevice for Sound {
    type Address = usize;
    type Value = u8;

    fn read_at(&self, address: usize) -> u8 {
        match address {
            0x04000060 => self.channel1_sweep.read().get_byte(0),
            0x04000061 => self.channel1_sweep.read().get_byte(1),
            0x04000062 => self.channel1_duty_length_envelope.read().get_byte(0),
            0x04000063 => self.channel1_duty_length_envelope.read().get_byte(1),
            0x04000064 => self.channel1_frequency_control.read().get_byte(0),
            0x04000065 => self.channel1_frequency_control.read().get_byte(1),
            0x04000068 => self.channel2_duty_length_envelope.read().get_byte(0),
            0x04000069 => self.channel2_duty_length_envelope.read().get_byte(1),
            0x0400006C => self.channel2_frequency_control.read().get_byte(0),
            0x0400006D => self.channel2_frequency_control.read().get_byte(1),
            0x04000070 => self.channel3_stop_wave_ram_select.read().get_byte(0),
            0x04000071 => self.channel3_stop_wave_ram_select.read().get_byte(1),
            0x04000072 => self.channel3_length_volume.read().get_byte(0),
            0x04000073 => self.channel3_length_volume.read().get_byte(1),
            0x04000074 => self.channel3_frequency_control.read().get_byte(0),
            0x04000075 => self.channel3_frequency_control.read().get_byte(1),
            0x04000078 => self.channel4_length_envelope.read().get_byte(0),
            0x04000079 => self.channel4_length_envelope.read().get_byte(1),
            0x0400007C => self.channel4_frequency_control.read().get_byte(0),
            0x0400007D => self.channel4_frequency_control.read().get_byte(1),
            0x04000080 => self.control_stereo_volume_enable.read().get_byte(0),
            0x04000081 => self.control_stereo_volume_enable.read().get_byte(1),
            0x04000082 => self.control_mixing_dma_control.read().get_byte(0),
            0x04000083 => self.control_mixing_dma_control.read().get_byte(1),
            0x04000084 => self.control_sound_on_off.read().get_byte(0),
            0x04000085 => self.control_sound_on_off.read().get_byte(1),
            0x04000088 => self.sound_pwm_control.read().get_byte(0),
            0x04000089 => self.sound_pwm_control.read().get_byte(1),
            0x04000090..=0x0400009F => self.channel3_wave_pattern_ram[address - 0x0400090],
            0x040000A0 => self.channel_a_fifo.read().get_byte(0),
            0x040000A1 => self.channel_a_fifo.read().get_byte(1),
            0x040000A2 => self.channel_a_fifo.read().get_byte(2),
            0x040000A3 => self.channel_a_fifo.read().get_byte(3),
            0x040000A4 => self.channel_b_fifo.read().get_byte(0),
            0x040000A5 => self.channel_b_fifo.read().get_byte(1),
            0x040000A6 => self.channel_b_fifo.read().get_byte(2),
            0x040000A7 => self.channel_b_fifo.read().get_byte(3),
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
            _ => panic!("Not implemented write memory address: {address:x}"),
        }
    }

    fn write_at(&mut self, address: usize, value: u8) {
        match address {
            0x04000060 => self.channel1_sweep.set_byte(0, value),
            0x04000061 => self.channel1_sweep.set_byte(1, value),
            0x04000062 => self.channel1_duty_length_envelope.set_byte(0, value),
            0x04000063 => self.channel1_duty_length_envelope.set_byte(1, value),
            0x04000064 => self.channel1_frequency_control.set_byte(0, value),
            0x04000065 => self.channel1_frequency_control.set_byte(1, value),
            0x04000068 => self.channel2_duty_length_envelope.set_byte(0, value),
            0x04000069 => self.channel2_duty_length_envelope.set_byte(1, value),
            0x0400006C => self.channel2_frequency_control.set_byte(0, value),
            0x0400006D => self.channel2_frequency_control.set_byte(1, value),
            0x04000070 => self.channel3_stop_wave_ram_select.set_byte(0, value),
            0x04000071 => self.channel3_stop_wave_ram_select.set_byte(1, value),
            0x04000072 => self.channel3_length_volume.set_byte(0, value),
            0x04000073 => self.channel3_length_volume.set_byte(1, value),
            0x04000074 => self.channel3_frequency_control.set_byte(0, value),
            0x04000075 => self.channel3_frequency_control.set_byte(1, value),
            0x04000078 => self.channel4_length_envelope.set_byte(0, value),
            0x04000079 => self.channel4_length_envelope.set_byte(1, value),
            0x0400007C => self.channel4_frequency_control.set_byte(0, value),
            0x0400007D => self.channel4_frequency_control.set_byte(1, value),
            0x04000080 => self.control_stereo_volume_enable.set_byte(0, value),
            0x04000081 => self.control_stereo_volume_enable.set_byte(1, value),
            0x04000082 => self.control_mixing_dma_control.set_byte(0, value),
            0x04000083 => self.control_mixing_dma_control.set_byte(1, value),
            0x04000084 => self.control_sound_on_off.set_byte(0, value),
            0x04000085 => self.control_sound_on_off.set_byte(1, value),
            0x04000088 => self.sound_pwm_control.set_byte(0, value),
            0x04000089 => self.sound_pwm_control.set_byte(1, value),
            0x04000090..=0x0400009F => self.channel3_wave_pattern_ram[address - 0x04000090] = value,
            0x040000A0 => self.channel_a_fifo.set_byte(0, value),
            0x040000A1 => self.channel_a_fifo.set_byte(1, value),
            0x040000A2 => self.channel_a_fifo.set_byte(2, value),
            0x040000A3 => self.channel_a_fifo.set_byte(3, value),
            0x040000A4 => self.channel_b_fifo.set_byte(0, value),
            0x040000A5 => self.channel_b_fifo.set_byte(1, value),
            0x040000A6 => self.channel_b_fifo.set_byte(2, value),
            0x040000A7 => self.channel_b_fifo.set_byte(3, value),
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
            _ => panic!("Not implemented write memory address: {address:x}"),
        }
    }
}
