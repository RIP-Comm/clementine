use std::ops::RangeInclusive;

/// Contains some helper methods to manipulate bits,
/// the index (`bit_idk`) is supposed to be from lsb to msb (right to left)
pub trait Bits {
    fn is_bit_on(&self, bit_idx: u8) -> bool;
    fn is_bit_off(&self, bit_idx: u8) -> bool;
    fn set_bit_on(&mut self, bit_idx: u8);
    fn set_bit_off(&mut self, bit_idx: u8);
    fn toggle_bit(&mut self, bit_idx: u8);
    fn set_bit(&mut self, bit_idx: u8, value: bool);
    fn get_bit(self, bit_idx: u8) -> bool;
    fn get_bits(self, bits_range: RangeInclusive<u8>) -> u32;
    fn are_bits_on(self, bits_range: RangeInclusive<u8>) -> bool;
    fn get_byte(&self, byte_nth: u8) -> u8;
    fn set_byte(&mut self, byte_nth: u8, value: u8);
}

impl Bits for u32 {
    fn is_bit_on(&self, bit_idx: u8) -> bool {
        debug_assert!(bit_idx < 32);

        let mask = 0b1 << bit_idx;
        (self & mask) != 0
    }

    fn is_bit_off(&self, bit_idx: u8) -> bool {
        debug_assert!(bit_idx < 32);

        let mask = 0b1 << bit_idx;
        (self & mask) == 0
    }

    fn set_bit_on(&mut self, bit_idx: u8) {
        debug_assert!(bit_idx < 32);

        let mask = 0b1 << bit_idx;
        *self |= mask;
    }

    fn set_bit_off(&mut self, bit_idx: u8) {
        debug_assert!(bit_idx < 32);

        let mask = !(0b1 << bit_idx);
        *self &= mask;
    }

    /// Switches from 1 to 0, or conversely from 0 to 1
    fn toggle_bit(&mut self, bit_idx: u8) {
        debug_assert!(bit_idx < 32);

        let mask = 0b1 << bit_idx;
        *self ^= mask;
    }

    fn set_bit(&mut self, bit_idx: u8, value: bool) {
        match value {
            false => self.set_bit_off(bit_idx),
            true => self.set_bit_on(bit_idx),
        }
    }

    fn get_bit(self, bit_idx: u8) -> bool {
        self.is_bit_on(bit_idx)
    }

    fn get_bits(self, bits_range: RangeInclusive<u8>) -> u32 {
        let mut bits = 0b0;
        for (shift_value, bit_index) in bits_range.enumerate() {
            let bit_value: Self = self.get_bit(bit_index).into();
            bits |= bit_value << shift_value;
        }
        bits
    }

    /// Checks if a certein sequence of bit is set to 1.
    /// Return false whenever there is a least one bit which is set to 0, true otherwise.
    /// When all bits are set to 1, the if statement fails and true is returned.
    fn are_bits_on(self, bits_range: RangeInclusive<u8>) -> bool {
        for (_, bit_index) in bits_range.enumerate() {
            if self.is_bit_off(bit_index) {
                return false;
            }
        }
        true
    }

    fn get_byte(&self, byte_nth: u8) -> u8 {
        if byte_nth > 3 {
            panic!("Byte number must be a value between 0 and 3.");
}

        // We access the byte_nth octect:
        // from the byte_nth*8 bit to the byte_nth*8+7 bit (inclusive)
        // e.g., 2nd octect is from 16th bit to 23rd bit
        self.get_bits(byte_nth * 8..=byte_nth * 8 + 7)
            .try_into()
            .unwrap()
    }

    fn set_byte(&mut self, byte_nth: u8, value: u8) {
        if byte_nth > 3 {
            panic!("Byte number must be a value between 0 and 3.");
        }

        // This mask is used to select the byte_nth octect and set it to 0.
        let mask = !match byte_nth {
            0 => 0xFF,
            1 => 0xFF << 8,
            2 => 0xFF << 16,
            3 => 0xFF << 24,
            _ => unreachable!(),
        };

        // We shift the new octect in place.
        let shifted_value: Self = match byte_nth {
            0 => value as Self,
            1 => (value as Self) << 8,
            2 => (value as Self) << 16,
            3 => (value as Self) << 24,
            _ => unreachable!(),
        };

        // We set the byte_nth octect to 0 using the mask and we
        // do the OR with the new octect.
        *self = (*self & mask) | shifted_value;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_is_on() {
        let b = 0b110011101_u32;
        assert!(b.is_bit_on(0));
        assert!(!b.is_bit_on(1));
        assert!(b.is_bit_on(2));
        assert!(b.is_bit_on(3));
        assert!(b.is_bit_on(8));
        assert!(!b.is_bit_on(31));
    }

    #[test]
    fn test_is_off() {
        let b = 0b110011101_u32;
        assert!(!b.is_bit_off(0));
        assert!(b.is_bit_off(1));
        assert!(!b.is_bit_off(2));
        assert!(!b.is_bit_off(3));
        assert!(!b.is_bit_off(8));
        assert!(b.is_bit_off(31));
    }

    #[test]
    fn test_set_on() {
        let mut b = 0b110011101_u32;
        b.set_bit_on(1);
        b.set_bit_on(0);
        b.set_bit_on(11);
        assert_eq!(b, 0b100110011111);
    }

    #[test]
    fn test_set_off() {
        let mut b = 0b1101001101_u32;
        b.set_bit_off(0);
        b.set_bit_off(4);
        b.set_bit_off(5);
        b.set_bit_off(6);
        b.set_bit_off(20);
        assert_eq!(b, 0b1100001100);
    }

    #[test]
    fn toggle_bit() {
        let original = rand::thread_rng().gen_range(1..=u32::MAX - 1);
        let mut fin = original;
        for i in 0..32 {
            fin.toggle_bit(i)
        }

        assert_eq!(!original, fin);
    }

    #[test]
    fn set_bit() {
        let mut b = 0b1100110_u32;
        b.set_bit(0, true);
        b.set_bit(1, true);
        b.set_bit(2, false);
        b.set_bit(3, false);
        assert_eq!(b, 0b1100011)
    }

    #[test]
    fn get_bit() {
        let b = 0b1011001110_u32;
        assert!(b.get_bit(1));
        assert!(!b.get_bit(0));
        assert!(b.get_bit(2));
        assert!(!b.get_bit(31));
    }

    #[test]
    #[should_panic]
    fn invalid_index() {
        let b = 0u32;
        b.is_bit_on(32);
    }

    #[test]
    fn get_gits() {
        let b = 0b1011001110_u32;
        assert_eq!(b.get_bits(0..=3), 0b1110);
        assert_eq!(b.get_bits(1..=1), 0b1);
        assert_eq!(b.get_bits(4..=7), 0b1100);
        assert_eq!(b.get_bits(8..=9), 0b10);
        assert_eq!(b.get_bits(0..=9), 0b10_1100_1110);
        assert_eq!(b.get_bits(0..=31), 0b10_1100_1110);
        assert_eq!(b.get_bits(28..=31), 0b0);
    }

    #[test]
    fn are_bits_on() {
        let b = 0b1011001110_u32;
        assert!(!b.are_bits_on(0..=3));
        assert!(b.are_bits_on(1..=3));
    }

    #[test]
    fn get_byte() {
        let b = 0b00000001_00100010_00000100_01001000;

        assert_eq!(b.get_byte(0), 0b01001000_u8);
        assert_eq!(b.get_byte(1), 0b00000100_u8);
        assert_eq!(b.get_byte(2), 0b00100010_u8);
        assert_eq!(b.get_byte(3), 0b00000001_u8);
    }

    #[test]
    #[should_panic]
    fn get_byte_panic() {
        let b = 0b00000001_00000010_00000100_00001000;

        b.get_byte(4);
    }

    #[test]
    fn set_byte() {
        let mut b: u32 = 0;

        b.set_byte(0, 0b1010_1010);

        assert_eq!(b, 0b1010_1010);

        b = 0;
        b.set_byte(1, 0b1010_1010);

        assert_eq!(b >> 8, 0b1010_1010);

        b = 0;
        b.set_byte(2, 0b1010_1010);

        assert_eq!(b >> 16, 0b1010_1010);

        b = 0;
        b.set_byte(3, 0b1010_1010);

        assert_eq!(b >> 24, 0b1010_1010);
    }

    #[test]
    #[should_panic]
    fn set_byte_panic() {
        let mut b = 0b00000001_00000010_00000100_00001000;

        b.set_byte(4, 0);
    }
}
