use std::collections::HashMap;

use logger::log;

use crate::bitwise::Bits;
use crate::cpu::hardware::dma::{Dma, DmaRegisters};
use crate::cpu::hardware::lcd::Lcd;
use crate::cpu::hardware::sound::Sound;
use crate::cpu::hardware::HardwareComponent;
use crate::memory::{internal_memory::InternalMemory, io_device::IoDevice};

#[derive(Default)]
pub struct Bus {
    pub internal_memory: InternalMemory,
    pub lcd: Lcd,
    sound: Sound,
    dma: Dma,
    cycles_count: u128,
    last_used_address: usize,
    unused_region: HashMap<usize, u8>,
}

impl Bus {
    fn read_dma_raw(&self, address: usize) -> u8 {
        let read_dma_bank = |channel: &DmaRegisters, address: usize| match address {
            0..=9 => panic!("Reading a write-only DMA I/O register"),
            10 => channel.control.get_byte(0),
            11 => channel.control.get_byte(1),
            _ => panic!("DMA channel read address is out of bound"),
        };

        match address {
            0x040000B0..=0x040000BB => read_dma_bank(&self.dma.channels[0], address - 0x040000B0),
            0x040000BC..=0x040000C7 => read_dma_bank(&self.dma.channels[0], address - 0x040000BC),
            0x040000C8..=0x040000D3 => read_dma_bank(&self.dma.channels[0], address - 0x040000C8),
            0x040000D4..=0x040000DF => read_dma_bank(&self.dma.channels[0], address - 0x040000D4),
            0x040000E0..=0x040000FF => {
                log("read on unused memory");
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("DMA read address is out of bound"),
        }
    }

    fn write_dma_raw(&mut self, address: usize, value: u8) {
        let write_dma_bank = |channel: &mut DmaRegisters, address: usize, value: u8| match address {
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
                write_dma_bank(&mut self.dma.channels[0], address - 0x040000B0, value)
            }
            0x040000BC..=0x040000C7 => {
                write_dma_bank(&mut self.dma.channels[1], address - 0x040000BC, value)
            }
            0x040000C8..=0x040000D3 => {
                write_dma_bank(&mut self.dma.channels[2], address - 0x040000C8, value)
            }
            0x040000D4..=0x040000DF => {
                write_dma_bank(&mut self.dma.channels[3], address - 0x040000D4, value)
            }
            0x040000E0..=0x040000FF => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            _ => panic!("Not implemented write memory address: {address:x}"),
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
            0x04000090..=0x0400009F => self.sound.channel3_wave_pattern_ram[address - 0x0400090],
            0x040000A0..=0x040000A7 => panic!("Reading a write-only Sound I/O register"),
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
                self.sound.channel3_wave_pattern_ram[address - 0x04000090] = value
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
            0x04000000 => self.lcd.dispcnt.get_byte(0),
            0x04000001 => self.lcd.dispcnt.get_byte(1),
            0x04000002 => self.lcd.green_swap.get_byte(0),
            0x04000003 => self.lcd.green_swap.get_byte(1),
            0x04000004 => self.lcd.dispstat.get_byte(0),
            0x04000005 => self.lcd.dispstat.get_byte(1),
            0x04000006 => self.lcd.vcount.get_byte(0),
            0x04000007 => self.lcd.vcount.get_byte(1),
            0x04000008 => self.lcd.bg0cnt.get_byte(0),
            0x04000009 => self.lcd.bg0cnt.get_byte(1),
            0x0400000A => self.lcd.bg1cnt.get_byte(0),
            0x0400000B => self.lcd.bg1cnt.get_byte(1),
            0x0400000C => self.lcd.bg2cnt.get_byte(0),
            0x0400000D => self.lcd.bg2cnt.get_byte(1),
            0x0400000E => self.lcd.bg3cnt.get_byte(0),
            0x0400000F => self.lcd.bg3cnt.get_byte(1),
            0x04000010..=0x04000047 => panic!("Reading a write-only LCD I/O register"),
            0x04000048 => self.lcd.winin.get_byte(0),
            0x04000049 => self.lcd.winin.get_byte(1),
            0x0400004A => self.lcd.winout.get_byte(0),
            0x0400004B => self.lcd.winout.get_byte(1),
            0x0400004C => self.lcd.mosaic.get_byte(0),
            0x0400004D => self.lcd.mosaic.get_byte(1),
            0x04000050 => self.lcd.bldcnt.get_byte(0),
            0x04000051 => self.lcd.bldcnt.get_byte(1),
            0x04000052 => self.lcd.bldalpha.get_byte(0),
            0x04000053 => self.lcd.bldalpha.get_byte(1),
            0x04000054..=0x04000055 => panic!("Reading a write-only LCD I/O register"),
            0x0400004E..=0x0400004F | 0x04000056..=0x0400005F => {
                log("read on unused memory");
                self.unused_region.get(&address).map_or(0, |v| *v)
            }
            _ => panic!("LCD read address is out of bound"),
        }
    }

    fn write_lcd_raw(&mut self, address: usize, value: u8) {
        match address {
            0x04000000 => self.lcd.dispcnt.set_byte(0, value),
            0x04000001 => self.lcd.dispcnt.set_byte(1, value),
            0x04000002 => self.lcd.green_swap.set_byte(0, value),
            0x04000003 => self.lcd.green_swap.set_byte(1, value),
            0x04000004 => self.lcd.dispstat.set_byte(0, value),
            0x04000005 => self.lcd.dispstat.set_byte(1, value),
            0x04000008 => self.lcd.bg0cnt.set_byte(0, value),
            0x04000006 => self.lcd.vcount.set_byte(0, value),
            0x04000007 => self.lcd.vcount.set_byte(1, value),
            0x04000009 => self.lcd.bg0cnt.set_byte(1, value),
            0x0400000A => self.lcd.bg1cnt.set_byte(0, value),
            0x0400000B => self.lcd.bg1cnt.set_byte(1, value),
            0x0400000C => self.lcd.bg2cnt.set_byte(0, value),
            0x0400000D => self.lcd.bg2cnt.set_byte(1, value),
            0x0400000E => self.lcd.bg3cnt.set_byte(0, value),
            0x0400000F => self.lcd.bg3cnt.set_byte(1, value),
            0x04000010 => self.lcd.bg0hofs.set_byte(0, value),
            0x04000011 => self.lcd.bg0hofs.set_byte(1, value),
            0x04000012 => self.lcd.bg0vofs.set_byte(0, value),
            0x04000013 => self.lcd.bg0vofs.set_byte(1, value),
            0x04000014 => self.lcd.bg1hofs.set_byte(0, value),
            0x04000015 => self.lcd.bg1hofs.set_byte(1, value),
            0x04000016 => self.lcd.bg1vofs.set_byte(0, value),
            0x04000017 => self.lcd.bg1vofs.set_byte(1, value),
            0x04000018 => self.lcd.bg2hofs.set_byte(0, value),
            0x04000019 => self.lcd.bg2hofs.set_byte(1, value),
            0x0400001A => self.lcd.bg2vofs.set_byte(0, value),
            0x0400001B => self.lcd.bg2vofs.set_byte(1, value),
            0x0400001C => self.lcd.bg3hofs.set_byte(0, value),
            0x0400001D => self.lcd.bg3hofs.set_byte(1, value),
            0x0400001E => self.lcd.bg3vofs.set_byte(0, value),
            0x0400001F => self.lcd.bg3vofs.set_byte(1, value),
            0x04000020 => self.lcd.bg2pa.set_byte(0, value),
            0x04000021 => self.lcd.bg2pa.set_byte(1, value),
            0x04000022 => self.lcd.bg2pb.set_byte(0, value),
            0x04000023 => self.lcd.bg2pb.set_byte(1, value),
            0x04000024 => self.lcd.bg2pc.set_byte(0, value),
            0x04000025 => self.lcd.bg2pc.set_byte(1, value),
            0x04000026 => self.lcd.bg2pd.set_byte(0, value),
            0x04000027 => self.lcd.bg2pd.set_byte(1, value),
            0x04000028 => self.lcd.bg2x.set_byte(0, value),
            0x04000029 => self.lcd.bg2x.set_byte(1, value),
            0x0400002A => self.lcd.bg2x.set_byte(2, value),
            0x0400002B => self.lcd.bg2x.set_byte(3, value),
            0x0400002C => self.lcd.bg2y.set_byte(0, value),
            0x0400002D => self.lcd.bg2y.set_byte(1, value),
            0x0400002E => self.lcd.bg2y.set_byte(2, value),
            0x0400002F => self.lcd.bg2y.set_byte(3, value),
            0x04000030 => self.lcd.bg3pa.set_byte(0, value),
            0x04000031 => self.lcd.bg3pa.set_byte(1, value),
            0x04000032 => self.lcd.bg3pb.set_byte(0, value),
            0x04000033 => self.lcd.bg3pb.set_byte(1, value),
            0x04000034 => self.lcd.bg3pc.set_byte(0, value),
            0x04000035 => self.lcd.bg3pc.set_byte(1, value),
            0x04000036 => self.lcd.bg3pd.set_byte(0, value),
            0x04000037 => self.lcd.bg3pd.set_byte(1, value),
            0x04000038 => self.lcd.bg3x.set_byte(0, value),
            0x04000039 => self.lcd.bg3x.set_byte(1, value),
            0x0400003A => self.lcd.bg3x.set_byte(2, value),
            0x0400003B => self.lcd.bg3x.set_byte(3, value),
            0x0400003C => self.lcd.bg3y.set_byte(0, value),
            0x0400003D => self.lcd.bg3y.set_byte(1, value),
            0x0400003E => self.lcd.bg3y.set_byte(2, value),
            0x0400003F => self.lcd.bg3y.set_byte(3, value),
            0x04000040 => self.lcd.win0h.set_byte(0, value),
            0x04000041 => self.lcd.win0h.set_byte(1, value),
            0x04000042 => self.lcd.win1h.set_byte(0, value),
            0x04000043 => self.lcd.win1h.set_byte(1, value),
            0x04000044 => self.lcd.win0v.set_byte(0, value),
            0x04000045 => self.lcd.win0v.set_byte(1, value),
            0x04000046 => self.lcd.win1v.set_byte(0, value),
            0x04000047 => self.lcd.win1v.set_byte(1, value),
            0x04000048 => self.lcd.winin.set_byte(0, value),
            0x04000049 => self.lcd.winin.set_byte(1, value),
            0x0400004A => self.lcd.winout.set_byte(0, value),
            0x0400004B => self.lcd.winout.set_byte(1, value),
            0x0400004C => self.lcd.mosaic.set_byte(0, value),
            0x0400004D => self.lcd.mosaic.set_byte(1, value),
            // 0x0400004E, 0x0400004F are not used
            0x04000050 => self.lcd.bldcnt.set_byte(0, value),
            0x04000051 => self.lcd.bldcnt.set_byte(1, value),
            0x04000052 => self.lcd.bldalpha.set_byte(0, value),
            0x04000053 => self.lcd.bldalpha.set_byte(1, value),
            0x04000054 => self.lcd.bldy.set_byte(0, value),
            0x04000055 => self.lcd.bldy.set_byte(1, value),
            0x0400004E..=0x0400004F | 0x04000056..=0x0400005F => {
                log("write on unused memory");
                self.unused_region.insert(address, value);
            }
            _ => panic!("LCD write address is out of bound"),
        }
    }

    fn read_raw(&self, address: usize) -> u8 {
        match address {
            0x4000000..=0x400005F => self.read_lcd_raw(address),
            0x4000060..=0x40000AF => self.read_sound_raw(address),
            0x40000B0..=0x40000FF => self.read_dma_raw(address),
            // TODO: change also other devices similar to how LCD is handled
            _ => self.internal_memory.read_at(address),
        }
    }

    fn write_raw(&mut self, address: usize, value: u8) {
        match address {
            0x4000000..=0x400005F => self.write_lcd_raw(address, value),
            0x4000060..=0x40000AF => self.write_sound_raw(address, value),
            0x40000B0..=0x40000FF => self.write_dma_raw(address, value),
            // TODO: read read_raw
            _ => self.internal_memory.write_at(address, value),
        }
    }

    pub fn read_byte(&mut self, address: usize) -> u8 {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        self.read_raw(address)
    }

    pub fn write_byte(&mut self, address: usize, value: u8) {
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        self.write_raw(address, value);
    }

    fn step(&mut self) {
        // Step cycles at beginning or end?
        // It may have an impact when we will introduce timers.
        self.cycles_count += 1;

        // TODO: move this somewhere in the UI
        log(format!("CPU Cycles: {}", self.cycles_count));

        // Step ppu, dma, interrupts, timers, etc...
        let val = *self
            .internal_memory
            .interrupts
            .interrupt_request
            .back()
            .unwrap();
        self.internal_memory.interrupts.interrupt_request.push(val);

        let val = *self
            .internal_memory
            .interrupts
            .interrupt_request
            .back()
            .unwrap();
        self.internal_memory.interrupts.interrupt_request.push(val);

        // A pixel takes 4 cycles to get drawn
        if self.cycles_count % 4 == 0 {
            self.lcd.step();
        }
    }

    pub fn with_memory(memory: InternalMemory) -> Self {
        Self {
            internal_memory: memory,
            ..Default::default()
        }
    }

    const fn get_wait_cycles(&self, address: usize) -> u128 {
        let _is_sequential =
            address == self.last_used_address || address + 4 == self.last_used_address;

        match address {
            // Bios
            0x0..=0x3FFF => 1,
            _ => 0,
        }
    }

    pub fn read_word(&mut self, address: usize) -> u32 {
        // TODO: here we have to see how many times to wait for the waitcycles
        // It depends on the bus width of the memory region
        // Right now we're assuming that every region has a bus width of 32 bits
        // So we wait only once to read a word.
        // In reality for example WRAM has a bus width of 16 bits so we would
        // have to repeat this cycle 2 times (to emulate the fact that we will access the memory
        // two times)
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        if address & 3 != 0 {
            log("warning, read_word has address not word aligned");
        }

        let part_0: u32 = self.read_raw(address).try_into().unwrap();
        let part_1: u32 = self.read_raw(address + 1).try_into().unwrap();
        let part_2: u32 = self.read_raw(address + 2).try_into().unwrap();
        let part_3: u32 = self.read_raw(address + 3).try_into().unwrap();

        part_3 << 24_u32 | part_2 << 16_u32 | part_1 << 8_u32 | part_0
    }

    pub fn write_word(&mut self, address: usize, value: u32) {
        // TODO: Look at read_word
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        if address & 3 != 0 {
            log("warning, write_word has address not word aligned");
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

    pub fn read_half_word(&mut self, address: usize) -> u16 {
        // TODO: Look at read_word
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        if address & 1 != 0 {
            log("warning, read_half_word has address not half-word aligned");
        }

        let part_0: u16 = self.read_raw(address).try_into().unwrap();
        let part_1: u16 = self.read_raw(address + 1).try_into().unwrap();

        part_1 << 8 | part_0
    }

    pub fn write_half_word(&mut self, address: usize, value: u16) {
        // TODO: Look at read_word
        for _ in 0..self.get_wait_cycles(address) {
            self.step();
        }

        self.last_used_address = address;

        if address & 1 != 0 {
            log("warning, write_half_word has address not half-word aligned");
        }

        let part_0: u8 = value.get_bits(0..=7).try_into().unwrap();
        let part_1: u8 = value.get_bits(8..=15).try_into().unwrap();

        self.write_raw(address, part_0);
        self.write_raw(address + 1, part_1);
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

        assert_eq!(bus.lcd.winin, 10);

        let address = 0x04000049; // WININ higher byte

        bus.write_raw(address, 5);
        assert_eq!(bus.lcd.winin, (5 << 8) | 10);
    }

    #[test]
    fn test_read_lcd_reg() {
        let mut bus = Bus::default();
        let address = 0x04000048; // WININ lower byte

        bus.lcd.winin = (5 << 8) | 10;

        assert_eq!(bus.read_raw(address), 10);

        let address = 0x04000049; // WININ higher byte

        assert_eq!(bus.read_raw(address), 5);
    }
}
