//! Seiko S3511 real-time clock, as used by Pokemon Fire Red/Leaf Green and
//! Ruby/Sapphire/Emerald. The chip is bit-banged over three GPIO pins:
//!
//! - bit 0: SCK, the serial clock.
//! - bit 1: SIO, the bidirectional serial data line.
//! - bit 2: CS, chip select.
//!
//! The game raises CS, clocks in an 8-bit command MSB first, then either clocks
//! data out (a read) or in (a write), LSB first per byte. Only the date/time and
//! control registers are modeled here. Anything unrecognized is ignored rather
//! than left half-processed, so a game can never stall waiting on the RTC.

// Date components are all small and bounded, so the casts to u8 and the
// timestamp cast cannot lose meaningful data.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const PIN_SCK: u8 = 1 << 0;
const PIN_SIO: u8 = 1 << 1;
const PIN_CS: u8 = 1 << 2;

/// Serial transfer phase.
#[derive(Default, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
enum Phase {
    #[default]
    Idle,
    Command,
    Reading,
    Writing,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Rtc {
    sck: bool,
    cs: bool,
    phase: Phase,
    /// Bits accumulated for the current byte.
    shift: u8,
    bit_count: u8,
    /// Bytes still to send (read) or the count still to receive (write).
    buffer: Vec<u8>,
    byte_index: usize,
    /// Current SIO output level the game reads back.
    sio_out: bool,
    /// Control register. Bit 6 selects 24-hour mode, which games expect.
    control: u8,
}

impl Rtc {
    /// Process a GPIO write. `data` is the 4-bit pin state the game drove and
    /// `direction` marks which pins are outputs from the game (1 = output). The
    /// SIO pin is only sampled from the game while it drives it.
    pub fn write(&mut self, data: u8, direction: u8) {
        let sck = data & PIN_SCK != 0;
        let cs = data & PIN_CS != 0;
        let sio_in = data & PIN_SIO != 0;
        let sio_is_input_to_rtc = direction & PIN_SIO != 0;

        // CS going low ends any transfer.
        if self.cs && !cs {
            self.reset_transfer();
        }
        // CS going high starts a fresh command.
        if !self.cs && cs {
            self.phase = Phase::Command;
            self.shift = 0;
            self.bit_count = 0;
        }
        self.cs = cs;

        // Act on the rising edge of SCK while selected.
        let rising = !self.sck && sck;
        self.sck = sck;
        if !cs || !rising {
            return;
        }

        match self.phase {
            Phase::Idle => {}
            Phase::Command => {
                // Command byte is MSB first.
                self.shift = (self.shift << 1) | u8::from(sio_in);
                self.bit_count += 1;
                if self.bit_count == 8 {
                    self.decode_command(self.shift);
                }
            }
            Phase::Reading => self.clock_out_bit(),
            Phase::Writing => {
                if sio_is_input_to_rtc {
                    // Data bytes are LSB first.
                    self.shift = (self.shift >> 1) | (u8::from(sio_in) << 7);
                    self.bit_count += 1;
                    if self.bit_count == 8 {
                        self.store_written_byte(self.shift);
                    }
                }
            }
        }
    }

    /// SIO level the game reads back on the data pin.
    #[must_use]
    pub const fn sio(&self) -> bool {
        self.sio_out
    }

    fn reset_transfer(&mut self) {
        self.phase = Phase::Idle;
        self.shift = 0;
        self.bit_count = 0;
        self.buffer.clear();
        self.byte_index = 0;
    }

    fn decode_command(&mut self, byte: u8) {
        self.bit_count = 0;
        self.shift = 0;

        // High nibble is the fixed 0110 pattern. Reject anything else.
        if byte >> 4 != 0b0110 {
            self.phase = Phase::Idle;
            return;
        }
        let command = (byte >> 1) & 0b111;
        let is_read = byte & 1 != 0;

        match (command, is_read) {
            // Control/status register read.
            (1, true) => {
                self.buffer = vec![self.control];
                self.begin_reading();
            }
            // Control register write.
            (1, false) => {
                self.buffer = vec![0]; // one byte expected
                self.byte_index = 0;
                self.phase = Phase::Writing;
            }
            // Date and time, 7 BCD bytes.
            (2, true) => {
                self.buffer = datetime_bytes(current_unix_secs());
                self.begin_reading();
            }
            // Time only, 3 BCD bytes.
            (3, true) => {
                self.buffer = datetime_bytes(current_unix_secs())[4..7].to_vec();
                self.begin_reading();
            }
            // Reset.
            (0, _) => {
                self.control = 0;
                self.phase = Phase::Idle;
            }
            // Unhandled writes accept and drop a plausible number of bytes so the
            // game's clocking still completes.
            (_, false) => {
                self.buffer = vec![0];
                self.byte_index = 0;
                self.phase = Phase::Writing;
            }
            // Unhandled reads return zeroes.
            (_, true) => {
                self.buffer = vec![0];
                self.begin_reading();
            }
        }
    }

    fn begin_reading(&mut self) {
        self.phase = Phase::Reading;
        self.byte_index = 0;
        self.bit_count = 0;
        self.load_output_bit();
    }

    fn clock_out_bit(&mut self) {
        self.bit_count += 1;
        if self.bit_count == 8 {
            self.bit_count = 0;
            self.byte_index += 1;
            if self.byte_index >= self.buffer.len() {
                self.phase = Phase::Idle;
                return;
            }
        }
        self.load_output_bit();
    }

    fn load_output_bit(&mut self) {
        // Data bytes are sent LSB first.
        let byte = self.buffer.get(self.byte_index).copied().unwrap_or(0);
        self.sio_out = (byte >> self.bit_count) & 1 != 0;
    }

    const fn store_written_byte(&mut self, byte: u8) {
        self.bit_count = 0;
        // Only the control register write is meaningful here.
        self.control = byte;
        self.byte_index += 1;
        if self.byte_index >= self.buffer.len() {
            self.phase = Phase::Idle;
        }
    }
}

/// Current wall-clock time in seconds since the Unix epoch, clamped at 0.
fn current_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The 7 date/time bytes the RTC reports, all BCD: year (2 digit), month, day,
/// day of week (0-6), hour, minute, second.
fn datetime_bytes(unix_secs: i64) -> Vec<u8> {
    let days = unix_secs.div_euclid(86400);
    let secs_of_day = unix_secs.rem_euclid(86400);

    let (year, month, day) = civil_from_days(days);
    let weekday = days.rem_euclid(7); // 1970-01-01 was a Thursday, mapped to 4 below
    let weekday = (weekday + 4).rem_euclid(7);

    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;

    vec![
        bcd((year.rem_euclid(100)) as u8),
        bcd(month as u8),
        bcd(day as u8),
        bcd(weekday as u8),
        bcd(hour as u8),
        bcd(minute as u8),
        bcd(second as u8),
    ]
}

/// Convert days since the Unix epoch to a civil (year, month, day) using Howard
/// Hinnant's algorithm. Month is 1-12 and day is 1-31.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const fn bcd(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

#[cfg(test)]
mod tests {
    use super::{
        PIN_CS, PIN_SCK, PIN_SIO, Rtc, bcd, civil_from_days, current_unix_secs, datetime_bytes,
    };

    #[test]
    fn civil_conversion_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2021-01-01 is 18628 days after the epoch.
        assert_eq!(civil_from_days(18628), (2021, 1, 1));
    }

    #[test]
    fn datetime_bytes_are_bcd() {
        // 2021-02-03 04:05:06 UTC = 1612325106.
        let bytes = datetime_bytes(1_612_325_106);
        assert_eq!(bytes[0], bcd(21)); // year
        assert_eq!(bytes[1], bcd(2)); // month
        assert_eq!(bytes[2], bcd(3)); // day
        assert_eq!(bytes[4], bcd(4)); // hour
        assert_eq!(bytes[5], bcd(5)); // minute
        assert_eq!(bytes[6], bcd(6)); // second
    }

    /// Drive one bit into the RTC on an SCK rising edge with CS held high.
    fn clock_in(rtc: &mut Rtc, bit: bool) {
        let base = PIN_CS | if bit { PIN_SIO } else { 0 };
        rtc.write(base, PIN_SIO); // SCK low, game drives SIO
        rtc.write(base | PIN_SCK, PIN_SIO); // SCK high: rising edge samples
    }

    /// Read one bit out of the RTC on an SCK rising edge, SIO as input to the game.
    fn clock_out(rtc: &mut Rtc) -> bool {
        rtc.write(PIN_CS, 0); // SCK low, SIO floating (RTC drives it)
        let bit = rtc.sio();
        rtc.write(PIN_CS | PIN_SCK, 0); // SCK high advances to the next bit
        bit
    }

    #[test]
    fn datetime_read_streams_the_clock_bytes() {
        let mut rtc = Rtc::default();
        // Start a transfer: CS high.
        rtc.write(PIN_CS, PIN_SIO);

        // Command byte 0110_010_1: fixed 0110, command 2 (datetime), read.
        for bit in [false, true, true, false, false, true, false, true] {
            clock_in(&mut rtc, bit);
        }

        // The first byte read back must be the current year in BCD, matching
        // what datetime_bytes reports for roughly now.
        let mut byte = 0u8;
        for i in 0..8 {
            byte |= u8::from(clock_out(&mut rtc)) << i; // LSB first
        }
        let expected_year = datetime_bytes(current_unix_secs())[0];
        assert_eq!(byte, expected_year);
    }
}
