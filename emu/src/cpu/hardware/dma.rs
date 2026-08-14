use crate::bitwise::Bits;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Registers {
    pub source_address: u32,
    pub destination_address: u32,
    pub word_count: u16,
    pub control: u16,
    // Internal state for active DMA
    pub(crate) internal_source: u32,
    pub(crate) internal_dest: u32,
    pub(crate) internal_count: u32,
    pub(crate) was_enabled: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Dma {
    pub channels: [Registers; 4],
}

impl Registers {
    /// Start timing mode (bits 12-13): 0 immediate, 1 `VBlank`, 2 `HBlank`, 3 special.
    fn timing(&self) -> u16 {
        self.control.get_bits(12..=13)
    }

    /// Address bit widths per channel: (source, destination). DMA0-2 cannot
    /// target ROM so their destination is 27 bits, and DMA0 also cannot read ROM
    /// so its source is 27 bits. DMA3 reaches ROM with 28-bit addresses.
    const fn address_masks(idx: usize) -> (u32, u32) {
        match idx {
            0 => (0x07FF_FFFF, 0x07FF_FFFF),
            1 | 2 => (0x0FFF_FFFF, 0x07FF_FFFF),
            _ => (0x0FFF_FFFF, 0x0FFF_FFFF),
        }
    }

    /// Word count to load, treating 0 as the maximum for the channel. The count
    /// is 14 bits for DMA0-2 and 16 bits for DMA3.
    const fn reload_count(&self, idx: usize) -> u32 {
        let (mask, max) = match idx {
            0..=2 => (0x3FFF, 0x4000), // DMA0-2: 14-bit count, 16K max
            _ => (0xFFFF, 0x1_0000),   // DMA3: 16-bit count, 64K max
        };
        let count = self.word_count & mask;
        if count == 0 { max } else { count as u32 }
    }

    /// Latch the source, destination and count into the internal registers,
    /// done when the channel is enabled.
    const fn latch(&mut self, idx: usize) {
        let (src_mask, dst_mask) = Self::address_masks(idx);
        self.internal_source = self.source_address & src_mask;
        self.internal_dest = self.destination_address & dst_mask;
        self.internal_count = self.reload_count(idx);
    }
}

impl Dma {
    /// Detect channels that were just enabled and latch their internal
    /// registers, for every timing mode. Returns the index of a channel that
    /// should start immediately (timing mode 0).
    pub fn check_immediate_transfer(&mut self) -> Option<usize> {
        let mut immediate = None;
        for (idx, channel) in self.channels.iter_mut().enumerate() {
            let enable = channel.control.get_bit(15);
            let just_enabled = enable && !channel.was_enabled;
            channel.was_enabled = enable;

            if just_enabled {
                channel.latch(idx);
                if channel.timing() == 0 {
                    immediate = Some(idx);
                }
            }
        }
        immediate
    }

    /// Which enabled channels are triggered by the given timing event
    /// (1 = `VBlank`, 2 = `HBlank`, 3 = special).
    #[must_use]
    pub fn channels_for_timing(&self, timing: u16) -> [bool; 4] {
        let mut out = [false; 4];
        for (i, channel) in self.channels.iter().enumerate() {
            out[i] = channel.control.get_bit(15) && channel.timing() == timing;
        }
        out
    }

    /// Advance one channel's source/destination pointers and count after a
    /// single unit transfer.
    pub fn advance(&mut self, idx: usize, is_32bit: bool) {
        let channel = &mut self.channels[idx];
        let size = if is_32bit { 4 } else { 2 };

        // Source: 0=increment, 1=decrement, 2=fixed, 3=prohibited.
        match channel.control.get_bits(7..=8) {
            0 => channel.internal_source = channel.internal_source.wrapping_add(size),
            1 => channel.internal_source = channel.internal_source.wrapping_sub(size),
            _ => {}
        }

        // Destination: 0=increment, 1=decrement, 2=fixed, 3=increment+reload.
        match channel.control.get_bits(5..=6) {
            0 | 3 => channel.internal_dest = channel.internal_dest.wrapping_add(size),
            1 => channel.internal_dest = channel.internal_dest.wrapping_sub(size),
            _ => {}
        }

        channel.internal_count = channel.internal_count.wrapping_sub(1);
    }

    /// Apply the repeat/disable rules after a channel's block of transfers
    /// completes. Repeating channels stay enabled and reload for the next
    /// trigger; others are disabled.
    pub fn finish_block(&mut self, idx: usize) {
        let channel = &mut self.channels[idx];

        if channel.control.get_bit(9) && channel.timing() != 0 {
            channel.internal_count = channel.reload_count(idx);
            // Destination control 3 reloads the destination each repeat.
            if channel.control.get_bits(5..=6) == 3 {
                let (_, dst_mask) = Registers::address_masks(idx);
                channel.internal_dest = channel.destination_address & dst_mask;
            }
        } else {
            channel.control.set_bit_off(15);
            channel.was_enabled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Control register bits: enable (15), repeat (9), timing (12-13).
    const ENABLE: u16 = 1 << 15;
    const REPEAT: u16 = 1 << 9;
    const TIMING_VBLANK: u16 = 1 << 12;

    fn enabled_channel(timing: u16, count: u16) -> Registers {
        Registers {
            source_address: 0x0200_0000,
            destination_address: 0x0300_0000,
            word_count: count,
            control: ENABLE | timing,
            ..Default::default()
        }
    }

    #[test]
    fn enabling_latches_internal_registers_for_any_timing() {
        let mut dma = Dma::default();
        dma.channels[1] = enabled_channel(TIMING_VBLANK, 4);

        // A VBlank channel does not run immediately, but it is latched.
        assert_eq!(dma.check_immediate_transfer(), None);
        assert_eq!(dma.channels[1].internal_source, 0x0200_0000);
        assert_eq!(dma.channels[1].internal_dest, 0x0300_0000);
        assert_eq!(dma.channels[1].internal_count, 4);
        assert!(dma.channels[1].was_enabled);
    }

    #[test]
    fn immediate_channel_is_returned_to_run_now() {
        let mut dma = Dma::default();
        dma.channels[0] = enabled_channel(0, 2);
        assert_eq!(dma.check_immediate_transfer(), Some(0));
    }

    #[test]
    fn channels_for_timing_matches_enabled_and_timing() {
        let mut dma = Dma::default();
        dma.channels[1] = enabled_channel(TIMING_VBLANK, 1);
        dma.channels[2] = enabled_channel(0, 1); // immediate, not VBlank
        let triggered = dma.channels_for_timing(1);
        assert_eq!(triggered, [false, true, false, false]);
    }

    #[test]
    fn advance_increments_pointers_and_decrements_count() {
        let mut dma = Dma::default();
        dma.channels[3] = enabled_channel(0, 2);
        dma.check_immediate_transfer();

        dma.advance(3, true); // 32-bit
        assert_eq!(dma.channels[3].internal_source, 0x0200_0004);
        assert_eq!(dma.channels[3].internal_dest, 0x0300_0004);
        assert_eq!(dma.channels[3].internal_count, 1);
    }

    #[test]
    fn zero_word_count_latches_channel_maximum() {
        // A word count of 0 means the channel maximum: 0x4000 for DMA0-2 and
        // 0x10000 for DMA3, which does not fit in a u16.
        let mut dma = Dma::default();
        dma.channels[0] = enabled_channel(0, 0);
        dma.channels[3] = enabled_channel(0, 0);
        dma.check_immediate_transfer();

        assert_eq!(dma.channels[0].internal_count, 0x4000);
        assert_eq!(dma.channels[3].internal_count, 0x1_0000);
    }

    #[test]
    fn dma0_masks_addresses_to_internal_regions() {
        // DMA0 cannot touch ROM: a source and destination in the ROM region are
        // masked down to 27 bits on latch.
        let mut dma = Dma::default();
        dma.channels[0] = Registers {
            source_address: 0x0800_0000,
            destination_address: 0x0A00_0004,
            word_count: 4,
            control: ENABLE,
            ..Default::default()
        };
        dma.check_immediate_transfer();

        assert_eq!(dma.channels[0].internal_source, 0x0800_0000 & 0x07FF_FFFF);
        assert_eq!(dma.channels[0].internal_dest, 0x0A00_0004 & 0x07FF_FFFF);
    }

    #[test]
    fn dma3_keeps_full_rom_addresses() {
        // DMA3 reaches ROM, so its addresses survive the 28-bit mask unchanged.
        let mut dma = Dma::default();
        dma.channels[3] = Registers {
            source_address: 0x0800_0000,
            destination_address: 0x0200_0000,
            word_count: 4,
            control: ENABLE,
            ..Default::default()
        };
        dma.check_immediate_transfer();

        assert_eq!(dma.channels[3].internal_source, 0x0800_0000);
        assert_eq!(dma.channels[3].internal_dest, 0x0200_0000);
    }

    #[test]
    fn dma0_2_count_is_14_bits() {
        // A count of 0x4000 has no low 14 bits set, so DMA0-2 read it as 0, which
        // means the channel maximum.
        let mut dma = Dma::default();
        dma.channels[1] = enabled_channel(0, 0x4000);
        dma.check_immediate_transfer();
        assert_eq!(dma.channels[1].internal_count, 0x4000);
    }

    #[test]
    fn finish_block_repeats_or_disables() {
        // Repeating VBlank channel stays enabled and reloads the count.
        let mut dma = Dma::default();
        dma.channels[1] = enabled_channel(TIMING_VBLANK, 4);
        dma.channels[1].control |= REPEAT;
        dma.check_immediate_transfer();
        dma.channels[1].internal_count = 0; // block ran out
        dma.finish_block(1);
        assert!(dma.channels[1].control.get_bit(15));
        assert_eq!(dma.channels[1].internal_count, 4);

        // Non-repeating channel is disabled after its block.
        let mut dma = Dma::default();
        dma.channels[1] = enabled_channel(TIMING_VBLANK, 4);
        dma.check_immediate_transfer();
        dma.finish_block(1);
        assert!(!dma.channels[1].control.get_bit(15));
    }
}
