use std::fmt::Debug;
use std::mem::size_of;
use std::ops::RangeInclusive;

/// Contains some helper methods to manipulate bits,
/// the index (`bit_idk`) is supposed to be from lsb to msb (right to left)
pub trait Bits
where
    Self: Clone + Sized + Into<u128> + TryFrom<u128> + From<bool> + TryInto<u8> + From<u8>,
    <Self as TryFrom<u128>>::Error: Debug,
    <Self as TryInto<u8>>::Error: Debug,
{
    fn is_bit_on(&self, bit_idx: u8) -> bool {
        debug_assert!(bit_idx < (size_of::<Self>() * 8) as u8);
        let bitwise: u128 = <Self as Into<u128>>::into(self.clone());
        let mask: u128 = 0b1 << bit_idx;
        (bitwise & mask) != 0
    }

    fn is_bit_off(&self, bit_idx: u8) -> bool {
        debug_assert!(bit_idx < (size_of::<Self>() * 8) as u8);
        let bitwise: u128 = <Self as Into<u128>>::into(self.clone());
        let mask = 0b1 << bit_idx;
        (bitwise & mask) == 0
    }

    fn set_bit_on(&mut self, bit_idx: u8) {
        debug_assert!(bit_idx < (size_of::<Self>() * 8) as u8);
        let mut bitwise: u128 = <Self as Into<u128>>::into(self.clone());
        let mask = 0b1 << bit_idx;
        bitwise |= mask;
        *self = <Self as TryFrom<u128>>::try_from(bitwise).unwrap();
    }

    fn set_bit_off(&mut self, bit_idx: u8) {
        let mut bitwise: u128 = <Self as Into<u128>>::into(self.clone());
        let mask = !(0b1 << bit_idx);
        bitwise &= mask;
        *self = <Self as TryFrom<u128>>::try_from(bitwise).unwrap();
    }

    /// Switches from 1 to 0, or conversely from 0 to 1
    fn toggle_bit(&mut self, bit_idx: u8) {
        debug_assert!(bit_idx < (size_of::<Self>() * 8) as u8);
        let mut bitwise: u128 = <Self as Into<u128>>::into(self.clone());
        let mask = 0b1 << bit_idx;
        bitwise ^= mask;
        *self = <Self as TryFrom<u128>>::try_from(bitwise).unwrap();
    }

    fn set_bit(&mut self, bit_idx: u8, value: bool) {
        match value {
            false => self.set_bit_off(bit_idx),
            true => self.set_bit_on(bit_idx),
        }
    }

    fn get_bit(&self, bit_idx: u8) -> bool {
        self.is_bit_on(bit_idx)
    }

    fn get_bits(&self, bits_range: RangeInclusive<u8>) -> Self {
        let start = bits_range.start();
        let length = bits_range.len() as u32;

        // Gets a value with `length` number of ones.
        // If bits_range is 1..=10 then length is 10 and we want
        // 10 ones.
        let mut mask = (2_u128.pow(length)) - 1;

        // Moves the mask to the correct place.
        // If `bits_range` is 1..=10 then we should move the mask
        // 1 bit to the left in order to get from the first bit on.
        mask <<= start;

        let value: u128 = <Self as Into<u128>>::into(self.clone());

        // We apply the mask and then move the value back to the 0 position.
        <Self as TryFrom<u128>>::try_from((value & mask) >> start).unwrap()
    }

    /// Checks if a certain sequence of bit is set to 1.
    /// Return false whenever there is a least one bit which is set to 0, true otherwise.
    /// When all bits are set to 1, the if statement fails and true is returned.
    fn are_bits_on(&self, bits_range: RangeInclusive<u8>) -> bool {
        for (_, bit_index) in bits_range.enumerate() {
            if self.is_bit_off(bit_index) {
                return false;
            }
        }
        true
    }

    fn get_byte(&self, byte_nth: u8) -> u8 {
        debug_assert!(byte_nth < size_of::<Self>() as u8);

        // We access the byte_nth octet:
        // from the byte_nth*8 bit to the byte_nth*8+7 bit (inclusive)
        // e.g., 2nd octet is from 16th bit to 23rd bit
        self.get_bits(byte_nth * 8..=byte_nth * 8 + 7)
            .try_into()
            .unwrap()
    }

    fn set_byte(&mut self, byte_nth: u8, value: u8) {
        debug_assert!(byte_nth < size_of::<Self>() as u8);

        let mut bitwise: u128 = <Self as Into<u128>>::into(self.clone());
        // This mask is used to select the byte_nth octet and set it to 0.
        let mask: u128 = !(0xFF << (8 * byte_nth));

        // We shift the new octet in place.
        let shifted_value: u128 = (value as u128) << (8 * byte_nth);

        // We set the byte_nth octet to 0 using the mask and we
        // do the OR with the new octet.
        bitwise = (bitwise & mask) | shifted_value;
        *self = <Self as TryFrom<u128>>::try_from(bitwise).unwrap();
    }

    /// Returns a sign-extended copy of the value.
    /// `bits_amount` is the numbers of bits of the value we want
    /// to sign-extend.
    fn sign_extended(&self, number_of_bits: u8) -> Self {
        let value: u128 = <Self as Into<u128>>::into(self.clone());

        // We generate a mask having a 1 in the last most significant bit of the value
        // (if we suppose it to be a two's complement number `bits_amount` long)
        let mask = 1 << (number_of_bits - 1);

        // With `value ^ mask` we get the information about the "sign bit" in the value.
        // If the value is negative the "sign bit" will be set, resulting in a 0 after the XOR operation.
        // Subtracting the mask would then result in a sign-extended value.
        // Example in 4 bits:
        // We want to sign-extend 0b1001 which in `i4` is `-7`.
        // The mask in this case in 0b1000.
        // 0b1001 ^ 0b1000 = 0b0001
        // 0b0001 - 0b1000 = 0b1111_1001 (imagine it sign-extended to an `i8`)
        // The result would then represent `-7` if interpreted as an `i8`.
        // This works because the XOR operation "removed" the "sign-bit", leading to subsequent borrows
        // in the subtraction operation. If the starting value is positive it has the "sign bit" unset:
        // `7 = 0b0111`, `0b0111 ^ 0b1000 = 0b1111`, `0b1111 - 0b1000 = 0b0111`, returning `7` unchanged.
        // This works since the XOR operation "added" a `1` bit in the "sign-bit" position which is later
        // "removed" by the subtraction.
        let value = ((value as i128 ^ mask) - mask) as u128;

        // We need to remove all the excessive leading ones otherwise the `try_from` would fail when
        // `Self` is a type smaller than `128` bits.
        let size_bits = (size_of::<Self>() * 8) as u128;
        let mask = (1 << size_bits) - 1;
        let value = value & mask;

        <Self as TryFrom<u128>>::try_from(value).unwrap()
    }
}

impl Bits for u128 {}
impl Bits for u64 {}
impl Bits for u32 {}
impl Bits for u16 {}
impl Bits for u8 {}

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
        let b: u32 = 0b00000001_00100010_00000100_01001000;

        assert_eq!(b.get_byte(0), 0b01001000_u8);
        assert_eq!(b.get_byte(1), 0b00000100_u8);
        assert_eq!(b.get_byte(2), 0b00100010_u8);
        assert_eq!(b.get_byte(3), 0b00000001_u8);
    }

    #[test]
    #[should_panic]
    fn get_byte_panic() {
        let b: u32 = 0b00000001_00000010_00000100_00001000;

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
        let mut b: u32 = 0b00000001_00000010_00000100_00001000;

        b.set_byte(4, 0);
    }

    #[test]
    fn check_sign_extended() {
        let a: u32 = 0b1001; // -7 in i4

        assert_eq!(a.sign_extended(4) as i32, -7);
    }
}
