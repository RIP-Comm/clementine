/// Contains some helper methods to manipulate bits,
/// the index (`bit_idk`) is supposed to be from lsb to msb (right to left)
pub(crate) trait Bits {
    fn is_bit_on(&self, bit_idx: u8) -> bool;
    fn is_bit_off(&self, bit_idx: u8) -> bool;
    fn set_bit_on(&mut self, bit_idx: u8);
    fn set_bit_off(&mut self, bit_idx: u8);
    fn toggle_bit(&mut self, bit_idx: u8);
    fn set_bit(&mut self, bit_idx: u8, value: bool);
    fn get_bit(self, bit_idx: u8) -> bool;
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
}
