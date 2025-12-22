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
    pub(crate) internal_count: u16,
    pub(crate) was_enabled: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Dma {
    pub channels: [Registers; 4],
}

impl Dma {
    /// Check if any DMA channel needs to run immediately (timing mode = 0)
    /// Returns `channel_index` if a transfer should start
    pub fn check_immediate_transfer(&mut self) -> Option<usize> {
        for (idx, channel) in self.channels.iter_mut().enumerate() {
            let enable = channel.control.get_bit(15);
            let timing = channel.control.get_bits(12..=13);

            // Check if DMA just got enabled (0 -> 1 transition)
            let just_enabled = enable && !channel.was_enabled;
            channel.was_enabled = enable;

            // Timing mode 0 = Immediate (start right away)
            if just_enabled && timing == 0 {
                // Initialize internal pointers
                channel.internal_source = channel.source_address;
                channel.internal_dest = channel.destination_address;

                // If count is 0, use default values
                channel.internal_count = if channel.word_count == 0 {
                    match idx {
                        0..=2 => 0x4000_u16, // DMA0-2: 16KB
                        3 => 0,              // DMA3: 64KB (0x10000 doesn't fit in u16, wraps to 0)
                        _ => unreachable!(),
                    }
                } else {
                    channel.word_count
                };

                return Some(idx);
            }
        }
        None
    }

    /// Execute a single DMA transfer for the given channel
    /// Returns true if more transfers remain
    pub fn execute_transfer<F>(&mut self, channel_idx: usize, mut read_write: F) -> bool
    where
        F: FnMut(u32, u32, bool), // (source_addr, dest_addr, is_32bit)
    {
        let channel = &mut self.channels[channel_idx];

        if channel.internal_count == 0 {
            return false;
        }

        let is_32bit = channel.control.get_bit(10);
        let dest_control = channel.control.get_bits(5..=6);
        let source_control = channel.control.get_bits(7..=8);

        // Perform the transfer
        read_write(channel.internal_source, channel.internal_dest, is_32bit);

        // Update addresses based on control bits
        let transfer_size = if is_32bit { 4 } else { 2 };

        // Update source address: 0=increment, 1=decrement, 2=fixed, 3=prohibited
        match source_control {
            0 => channel.internal_source = channel.internal_source.wrapping_add(transfer_size),
            1 => channel.internal_source = channel.internal_source.wrapping_sub(transfer_size),
            _ => {} // Prohibited, treat as fixed
        }

        // Update dest address: 0=increment, 1=decrement, 2=fixed, 3=reload
        match dest_control {
            0 => channel.internal_dest = channel.internal_dest.wrapping_add(transfer_size),
            1 => channel.internal_dest = channel.internal_dest.wrapping_sub(transfer_size),
            2 => {}                                                   // Fixed
            3 => channel.internal_dest = channel.destination_address, // Reload
            _ => unreachable!(),
        }

        channel.internal_count -= 1;

        if channel.internal_count == 0 {
            let repeat = channel.control.get_bit(9);
            if repeat {
                // Reload count for repeat mode
                channel.internal_count = channel.word_count;
                // Reload destination if in reload mode
                if dest_control == 3 {
                    channel.internal_dest = channel.destination_address;
                }
                true
            } else {
                // DMA complete, disable it
                channel.control.set_bit_off(15);
                channel.was_enabled = false;
                false
            }
        } else {
            true
        }
    }
}
