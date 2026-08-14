use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::bitwise::Bits;

/// GBA system clock in Hz, used to derive how many CPU cycles fall between two
/// output samples.
const CPU_CLOCK: u64 = 16_777_216;

/// Frame sequencer runs at 512 Hz: one tick every `CPU_CLOCK / 512` cycles.
const FRAME_SEQUENCER_PERIOD: u64 = CPU_CLOCK / 512;

/// Each DMA sound FIFO holds up to 32 bytes (eight 32-bit writes).
const FIFO_CAPACITY: usize = 32;

/// A refill DMA is requested once a FIFO drains to half or below.
const FIFO_REFILL_THRESHOLD: usize = 16;

/// Square wave duty patterns (12.5%, 25%, 50%, 75%), one bit per phase step.
const DUTY_PATTERNS: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

/// Noise divisor codes, in GB clock cycles (scaled to GBA cycles at use).
const NOISE_DIVISORS: [u32; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

/// State of a square wave channel (channel 1 and 2). Channel 1 additionally
/// uses the frequency sweep fields.
#[derive(Default, Serialize, Deserialize)]
struct Square {
    /// Down-counter in CPU cycles until the next duty step.
    timer: i32,
    /// Current position in the 8-step duty pattern.
    phase: u8,
    /// Length counter; the channel is silenced when it reaches zero.
    length: u16,
    /// Current envelope volume (0..15).
    env_volume: u8,
    /// Envelope period down-counter.
    env_timer: u8,
    enabled: bool,

    /// Sweep period down-counter (channel 1 only).
    sweep_timer: u8,
    /// Shadow copy of the frequency the sweep operates on.
    sweep_shadow: u16,
    sweep_enabled: bool,
}

/// State of the wave channel (channel 3).
#[derive(Default, Serialize, Deserialize)]
struct Wave {
    timer: i32,
    /// Position in the 32 four-bit samples of wave RAM.
    position: u8,
    length: u16,
    enabled: bool,
}

/// State of the noise channel (channel 4).
#[derive(Default, Serialize, Deserialize)]
struct Noise {
    timer: i32,
    /// Linear feedback shift register producing the pseudo-random output.
    lfsr: u16,
    length: u16,
    env_volume: u8,
    env_timer: u8,
    enabled: bool,
}

/// Sound registers, the DMA sound engine and the four PSG channels.
///
/// The two DMA sound channels (A and B) are fed by byte FIFOs the game refills
/// through DMA, clocked by timer 0 or timer 1. The four PSG channels (two
/// squares, wave and noise) are driven from a 512 Hz frame sequencer that
/// clocks their length, envelope and sweep units. Every channel is resampled to
/// the host output rate with a sample-and-hold and pushed to `audio_out`.
#[derive(Default, Serialize, Deserialize)]
pub struct Sound {
    pub channel1_sweep: u16,
    pub channel1_duty_length_envelope: u16,
    pub channel1_frequency_control: u16,
    pub channel2_duty_length_envelope: u16,
    pub channel2_frequency_control: u16,
    pub channel3_stop_wave_ram_select: u16,
    pub channel3_length_volume: u16,
    pub channel3_frequency_control: u16,
    pub channel4_length_envelope: u16,
    pub channel4_frequency_control: u16,
    /// `SOUNDCNT_L` (`0x0400_0080`): PSG master volume and per-channel panning.
    pub control_stereo_volume_enable: u16,
    /// `SOUNDCNT_H` (`0x0400_0082`): DMA sound mixing and PSG volume ratio.
    pub control_mixing_dma_control: u16,
    /// `SOUNDCNT_X` (`0x0400_0084`): master sound enable.
    pub control_sound_on_off: u16,
    pub sound_pwm_control: u16,
    pub channel3_wave_pattern_ram: [u8; 16],

    // DMA sound state. Serialized (and kept in this exact order) so it matches
    // the established save-state layout.
    fifo_a: VecDeque<i8>,
    fifo_b: VecDeque<i8>,
    /// Last byte popped from each FIFO, held until the next timer overflow.
    sample_a: i8,
    sample_b: i8,
    /// CPU-cycle accumulator for the output resampler, scaled by `output_rate`.
    /// A host sample is emitted each time it reaches `CPU_CLOCK`.
    cycle_accumulator: u64,
    /// Host output sample rate in Hz. Zero until an audio device is attached.
    output_rate: u32,

    // PSG channel state. Not serialized: it rebuilds itself as the game rewrites
    // the sound registers, and skipping it lets the PSG engine evolve without
    // breaking save states.
    #[serde(skip)]
    square1: Square,
    #[serde(skip)]
    square2: Square,
    #[serde(skip)]
    wave: Wave,
    #[serde(skip)]
    noise: Noise,
    /// Cycle accumulator for the 512 Hz frame sequencer.
    #[serde(skip)]
    frame_sequencer_acc: u64,
    /// Current step (0..7) of the frame sequencer.
    #[serde(skip)]
    frame_sequencer_step: u8,

    /// Lock-free sink to the audio thread. Absent until the host wires an
    /// output device.
    #[serde(skip)]
    audio_out: Option<rtrb::Producer<f32>>,

    /// Previous input and output of the stereo DC-blocking filter, indexed by
    /// left and right. Not serialized because it is short-lived audio state.
    #[serde(skip)]
    dc_prev_in: [f32; 2],
    #[serde(skip)]
    dc_prev_out: [f32; 2],
}

// The PSG bit fields extracted here are all a few bits wide, and the cycle
// counts per step are small, so the down-casts to `u8`/`i32` cannot lose data.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
impl Sound {
    /// Attach the host audio sink and set the output sample rate. Called once
    /// when the emulator is created with an audio device available.
    pub fn set_audio_out(&mut self, producer: rtrb::Producer<f32>, output_rate: u32) {
        self.output_rate = output_rate;
        self.audio_out = Some(producer);
    }

    /// Detach the audio sink so it can be reinstalled after a save state load
    /// replaces the whole CPU (the producer is not serialized).
    pub const fn take_audio_out(&mut self) -> (Option<rtrb::Producer<f32>>, u32) {
        (self.audio_out.take(), self.output_rate)
    }

    /// Reinstall a previously detached audio sink.
    // Not const: replacing `audio_out` drops the previous producer.
    #[allow(clippy::missing_const_for_fn)]
    pub fn restore_audio_out(&mut self, producer: Option<rtrb::Producer<f32>>, output_rate: u32) {
        self.audio_out = producer;
        self.output_rate = output_rate;
    }

    // ----- DMA sound (channels A and B) -----------------------------------

    /// Push a byte written to a DMA sound FIFO, dropping it if the FIFO is full.
    /// `channel` is 0 for FIFO A, 1 for FIFO B.
    pub fn push_fifo(&mut self, channel: usize, value: u8) {
        let fifo = if channel == 0 {
            &mut self.fifo_a
        } else {
            &mut self.fifo_b
        };
        if fifo.len() < FIFO_CAPACITY {
            fifo.push_back(value.cast_signed());
        }
    }

    /// Clear FIFO A/B and its held sample. Triggered by the reset bits in
    /// `SOUNDCNT_H`.
    pub fn reset_fifo(&mut self, channel: usize) {
        if channel == 0 {
            self.fifo_a.clear();
            self.sample_a = 0;
        } else {
            self.fifo_b.clear();
            self.sample_b = 0;
        }
    }

    /// Pop the next byte of a DMA sound channel into its held sample. Returns
    /// `true` when the FIFO has drained to half or below and needs a refill.
    fn pop_fifo(&mut self, channel: usize) -> bool {
        let fifo = if channel == 0 {
            &mut self.fifo_a
        } else {
            &mut self.fifo_b
        };
        if let Some(value) = fifo.pop_front() {
            if channel == 0 {
                self.sample_a = value;
            } else {
                self.sample_b = value;
            }
        }
        let fifo = if channel == 0 {
            &self.fifo_a
        } else {
            &self.fifo_b
        };
        fifo.len() <= FIFO_REFILL_THRESHOLD
    }

    /// Called by the bus when timer 0 or timer 1 overflows. Pops the DMA sound
    /// channels driven by that timer and reports which FIFOs now need a refill
    /// DMA (index 0 = FIFO A, 1 = FIFO B).
    pub fn on_timer_overflow(&mut self, timer: u8) -> [bool; 2] {
        let control = self.control_mixing_dma_control;
        let mut refill = [false; 2];

        if u8::from(control.get_bit(10)) == timer {
            refill[0] = self.pop_fifo(0);
        }
        if u8::from(control.get_bit(14)) == timer {
            refill[1] = self.pop_fifo(1);
        }
        refill
    }

    // ----- PSG channels ---------------------------------------------------

    /// Decode a PSG register write, handling channel triggers, length reloads
    /// and DAC-off silencing. The raw register byte has already been stored by
    /// the bus; `value` is that byte.
    pub fn psg_register_written(&mut self, address: usize, value: u8) {
        match address {
            // Length data (NRx1) reloads the length counter.
            0x0400_0062 => self.square1.length = 64 - u16::from(value & 0x3F),
            0x0400_0068 => self.square2.length = 64 - u16::from(value & 0x3F),
            0x0400_0072 => self.wave.length = 256 - u16::from(value),
            0x0400_0078 => self.noise.length = 64 - u16::from(value & 0x3F),

            // Envelope / DAC control (NRx2, NR30): a disabled DAC kills the
            // channel immediately.
            0x0400_0063 if !Self::dac_on(self.channel1_duty_length_envelope) => {
                self.square1.enabled = false;
            }
            0x0400_0069 if !Self::dac_on(self.channel2_duty_length_envelope) => {
                self.square2.enabled = false;
            }
            0x0400_0070 if !self.channel3_stop_wave_ram_select.get_bit(7) => {
                self.wave.enabled = false;
            }
            0x0400_0079 if !Self::dac_on(self.channel4_length_envelope) => {
                self.noise.enabled = false;
            }

            // Frequency control high byte (NRx4): bit 7 is the trigger.
            0x0400_0065 if value.get_bit(7) => self.trigger_square1(),
            0x0400_006D if value.get_bit(7) => self.trigger_square2(),
            0x0400_0075 if value.get_bit(7) => self.trigger_wave(),
            0x0400_007D if value.get_bit(7) => self.trigger_noise(),
            _ => {}
        }
    }

    /// A square/noise DAC is on when the envelope register's volume or direction
    /// bits are non-zero (bits 11..15 of the combined register).
    const fn dac_on(envelope_register: u16) -> bool {
        envelope_register & 0xF800 != 0
    }

    fn trigger_square1(&mut self) {
        self.square1.enabled = Self::dac_on(self.channel1_duty_length_envelope);
        if self.square1.length == 0 {
            self.square1.length = 64;
        }
        self.square1.timer = self.square_period(0);
        self.reload_square_envelope(0);

        // Sweep unit init (channel 1 only).
        let freq = self.channel1_frequency_control.get_bits(0..=10);
        let period = self.channel1_sweep.get_bits(4..=6) as u8;
        let shift = self.channel1_sweep.get_bits(0..=2);
        self.square1.sweep_shadow = freq;
        self.square1.sweep_timer = if period == 0 { 8 } else { period };
        self.square1.sweep_enabled = period != 0 || shift != 0;
        if shift != 0 {
            self.compute_sweep(true);
        }
    }

    fn trigger_square2(&mut self) {
        self.square2.enabled = Self::dac_on(self.channel2_duty_length_envelope);
        if self.square2.length == 0 {
            self.square2.length = 64;
        }
        self.square2.timer = self.square_period(1);
        self.reload_square_envelope(1);
    }

    fn trigger_wave(&mut self) {
        self.wave.enabled = self.channel3_stop_wave_ram_select.get_bit(7);
        if self.wave.length == 0 {
            self.wave.length = 256;
        }
        self.wave.timer = self.wave_period();
        self.wave.position = 0;
    }

    fn trigger_noise(&mut self) {
        self.noise.enabled = Self::dac_on(self.channel4_length_envelope);
        if self.noise.length == 0 {
            self.noise.length = 64;
        }
        self.noise.timer = self.noise_period();
        self.noise.lfsr = 0x7FFF;
        self.noise.env_volume = (self.channel4_length_envelope.get_bits(12..=15)) as u8;
        self.noise.env_timer = (self.channel4_length_envelope.get_bits(8..=10)) as u8;
    }

    fn reload_square_envelope(&mut self, index: usize) {
        let register = if index == 0 {
            self.channel1_duty_length_envelope
        } else {
            self.channel2_duty_length_envelope
        };
        let volume = register.get_bits(12..=15) as u8;
        let period = register.get_bits(8..=10) as u8;
        let square = if index == 0 {
            &mut self.square1
        } else {
            &mut self.square2
        };
        square.env_volume = volume;
        square.env_timer = period;
    }

    /// Duty-step period in CPU cycles for a square channel.
    fn square_period(&self, index: usize) -> i32 {
        let register = if index == 0 {
            self.channel1_frequency_control
        } else {
            self.channel2_frequency_control
        };
        (2048 - i32::from(register.get_bits(0..=10))) * 16
    }

    /// Sample period in CPU cycles for the wave channel.
    fn wave_period(&self) -> i32 {
        (2048 - i32::from(self.channel3_frequency_control.get_bits(0..=10))) * 8
    }

    /// Shift period in CPU cycles for the noise channel.
    fn noise_period(&self) -> i32 {
        let register = self.channel4_frequency_control;
        let code = register.get_bits(0..=2) as usize;
        let shift = register.get_bits(4..=7);
        // The divisors are in GB cycles; multiply by 4 to reach GBA cycles.
        ((NOISE_DIVISORS[code] << shift) * 4) as i32
    }

    /// Advance all PSG timers and the frame sequencer by `cycles` CPU cycles.
    fn step_psg(&mut self, cycles: u64) {
        self.frame_sequencer_acc += cycles;
        while self.frame_sequencer_acc >= FRAME_SEQUENCER_PERIOD {
            self.frame_sequencer_acc -= FRAME_SEQUENCER_PERIOD;
            self.frame_sequencer_tick();
        }

        self.tick_square(0, cycles);
        self.tick_square(1, cycles);
        self.tick_wave(cycles);
        self.tick_noise(cycles);
    }

    fn tick_square(&mut self, index: usize, cycles: u64) {
        let period = self.square_period(index);
        if period <= 0 {
            return;
        }
        let square = if index == 0 {
            &mut self.square1
        } else {
            &mut self.square2
        };
        square.timer -= cycles as i32;
        while square.timer <= 0 {
            square.timer += period;
            square.phase = (square.phase + 1) & 7;
        }
    }

    fn tick_wave(&mut self, cycles: u64) {
        let period = self.wave_period();
        if period <= 0 {
            return;
        }
        self.wave.timer -= cycles as i32;
        while self.wave.timer <= 0 {
            self.wave.timer += period;
            self.wave.position = (self.wave.position + 1) & 31;
        }
    }

    fn tick_noise(&mut self, cycles: u64) {
        let period = self.noise_period();
        if period <= 0 {
            return;
        }
        let width_7bit = self.channel4_frequency_control.get_bit(3);
        self.noise.timer -= cycles as i32;
        while self.noise.timer <= 0 {
            self.noise.timer += period;
            let feedback = (self.noise.lfsr ^ (self.noise.lfsr >> 1)) & 1;
            self.noise.lfsr >>= 1;
            self.noise.lfsr |= feedback << 14;
            if width_7bit {
                self.noise.lfsr = (self.noise.lfsr & !(1 << 6)) | (feedback << 6);
            }
        }
    }

    /// One step of the 512 Hz frame sequencer: length at 256 Hz, sweep at
    /// 128 Hz, envelope at 64 Hz.
    fn frame_sequencer_tick(&mut self) {
        let step = self.frame_sequencer_step;
        if step.is_multiple_of(2) {
            self.clock_length();
        }
        if step == 2 || step == 6 {
            self.clock_sweep();
        }
        if step == 7 {
            self.clock_envelope();
        }
        self.frame_sequencer_step = (step + 1) & 7;
    }

    fn clock_length(&mut self) {
        if self.channel1_frequency_control.get_bit(14) && self.square1.length > 0 {
            self.square1.length -= 1;
            if self.square1.length == 0 {
                self.square1.enabled = false;
            }
        }
        if self.channel2_frequency_control.get_bit(14) && self.square2.length > 0 {
            self.square2.length -= 1;
            if self.square2.length == 0 {
                self.square2.enabled = false;
            }
        }
        if self.channel3_frequency_control.get_bit(14) && self.wave.length > 0 {
            self.wave.length -= 1;
            if self.wave.length == 0 {
                self.wave.enabled = false;
            }
        }
        if self.channel4_frequency_control.get_bit(14) && self.noise.length > 0 {
            self.noise.length -= 1;
            if self.noise.length == 0 {
                self.noise.enabled = false;
            }
        }
    }

    fn clock_sweep(&mut self) {
        if !self.square1.sweep_enabled {
            return;
        }
        let period = self.channel1_sweep.get_bits(4..=6) as u8;
        self.square1.sweep_timer = self.square1.sweep_timer.saturating_sub(1);
        if self.square1.sweep_timer == 0 {
            self.square1.sweep_timer = if period == 0 { 8 } else { period };
            if period != 0 {
                let new_freq = self.compute_sweep(true);
                let shift = self.channel1_sweep.get_bits(0..=2);
                if new_freq <= 2047 && shift != 0 {
                    self.square1.sweep_shadow = new_freq;
                    self.channel1_frequency_control =
                        (self.channel1_frequency_control & !0x07FF) | new_freq;
                    // A second calculation checks for overflow again.
                    self.compute_sweep(true);
                }
            }
        }
    }

    /// Compute the next sweep frequency. When `check_overflow` is set and the
    /// result exceeds the 11-bit range, the channel is disabled.
    fn compute_sweep(&mut self, check_overflow: bool) -> u16 {
        let shift = self.channel1_sweep.get_bits(0..=2);
        let decrease = self.channel1_sweep.get_bit(3);
        let delta = self.square1.sweep_shadow >> shift;
        let new_freq = if decrease {
            self.square1.sweep_shadow.wrapping_sub(delta)
        } else {
            self.square1.sweep_shadow + delta
        };
        if check_overflow && new_freq > 2047 {
            self.square1.enabled = false;
        }
        new_freq
    }

    fn clock_envelope(&mut self) {
        Self::step_square_envelope(&mut self.square1, self.channel1_duty_length_envelope);
        Self::step_square_envelope(&mut self.square2, self.channel2_duty_length_envelope);

        let period = self.channel4_length_envelope.get_bits(8..=10) as u8;
        let increase = self.channel4_length_envelope.get_bit(11);
        if period != 0 {
            self.noise.env_timer = self.noise.env_timer.saturating_sub(1);
            if self.noise.env_timer == 0 {
                self.noise.env_timer = period;
                if increase && self.noise.env_volume < 15 {
                    self.noise.env_volume += 1;
                } else if !increase && self.noise.env_volume > 0 {
                    self.noise.env_volume -= 1;
                }
            }
        }
    }

    fn step_square_envelope(square: &mut Square, register: u16) {
        let period = register.get_bits(8..=10) as u8;
        let increase = register.get_bit(11);
        if period == 0 {
            return;
        }
        square.env_timer = square.env_timer.saturating_sub(1);
        if square.env_timer == 0 {
            square.env_timer = period;
            if increase && square.env_volume < 15 {
                square.env_volume += 1;
            } else if !increase && square.env_volume > 0 {
                square.env_volume -= 1;
            }
        }
    }

    /// Current output of a square channel as a signed level in `[-1.0, 1.0]`,
    /// or `0.0` when the channel is off.
    fn square_output(&self, index: usize) -> f32 {
        let (square, register) = if index == 0 {
            (&self.square1, self.channel1_duty_length_envelope)
        } else {
            (&self.square2, self.channel2_duty_length_envelope)
        };
        if !square.enabled {
            return 0.0;
        }
        let duty = register.get_bits(6..=7) as usize;
        let level = DUTY_PATTERNS[duty][square.phase as usize] * square.env_volume;
        Self::dac(level)
    }

    fn wave_output(&self) -> f32 {
        if !self.wave.enabled {
            return 0.0;
        }
        let byte = self.channel3_wave_pattern_ram[(self.wave.position / 2) as usize];
        let nibble = if self.wave.position & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        };
        // NR32 volume: 0 mute, 1 full, 2 half, 3 quarter.
        let shift = match self.channel3_length_volume.get_bits(13..=14) {
            0 => return Self::dac(0),
            1 => 0,
            2 => 1,
            _ => 2,
        };
        Self::dac(nibble >> shift)
    }

    fn noise_output(&self) -> f32 {
        if !self.noise.enabled {
            return 0.0;
        }
        let level = if self.noise.lfsr & 1 == 0 {
            self.noise.env_volume
        } else {
            0
        };
        Self::dac(level)
    }

    /// Map a 4-bit DAC level (0..15) to a signed sample in `[-1.0, 1.0]`.
    fn dac(level: u8) -> f32 {
        f32::from(level) / 7.5 - 1.0
    }

    // ----- Output ---------------------------------------------------------

    /// Advance the whole sound engine by `cycles` CPU cycles, emitting host
    /// samples as their period elapses.
    pub fn step(&mut self, cycles: u64) {
        if self.audio_out.is_none() || self.output_rate == 0 {
            return;
        }

        self.step_psg(cycles);

        self.cycle_accumulator += cycles * u64::from(self.output_rate);
        while self.cycle_accumulator >= CPU_CLOCK {
            self.cycle_accumulator -= CPU_CLOCK;
            self.emit_sample();
        }
    }

    /// One-pole DC-blocking high-pass filter, applied per stereo channel. It
    /// removes the constant offset the PSG DAC introduces, so an enabled but
    /// idle channel settles to silence instead of a large negative level, and
    /// enabling a channel no longer produces an audible thump.
    fn dc_block(&mut self, channel: usize, input: f32) -> f32 {
        // R near 1 places the cutoff at a few Hz, below the audible range.
        const R: f32 = 0.999;
        let output = R.mul_add(self.dc_prev_out[channel], input - self.dc_prev_in[channel]);
        self.dc_prev_in[channel] = input;
        self.dc_prev_out[channel] = output;
        output
    }

    /// Mix all channels into a stereo frame and push it.
    fn emit_sample(&mut self) {
        let (left, right) = self.mix();
        let left = self.dc_block(0, left).clamp(-1.0, 1.0);
        let right = self.dc_block(1, right).clamp(-1.0, 1.0);
        if let Some(out) = self.audio_out.as_mut() {
            // Push the left and right samples together or not at all. Dropping
            // only one of them on a nearly full ring would shift the L/R parity
            // and swap the channels for the rest of playback.
            if out.slots() >= 2 {
                let _ = out.push(left);
                let _ = out.push(right);
            }
        }
    }

    /// Combine the DMA sound and PSG channels according to `SOUNDCNT_L`/`H`/`X`
    /// into a normalized stereo pair in `[-1.0, 1.0]`.
    fn mix(&self) -> (f32, f32) {
        if !self.control_sound_on_off.get_bit(7) {
            return (0.0, 0.0);
        }

        let (dma_left, dma_right) = self.mix_dma_sound();
        let (psg_left, psg_right) = self.mix_psg();

        (
            (dma_left + psg_left).clamp(-1.0, 1.0),
            (dma_right + psg_right).clamp(-1.0, 1.0),
        )
    }

    fn mix_dma_sound(&self) -> (f32, f32) {
        let control = self.control_mixing_dma_control;
        let vol_a = if control.get_bit(2) { 1.0 } else { 0.5 };
        let vol_b = if control.get_bit(3) { 1.0 } else { 0.5 };

        let a = f32::from(self.sample_a) / 128.0 * vol_a;
        let b = f32::from(self.sample_b) / 128.0 * vol_b;

        let mut left = 0.0;
        let mut right = 0.0;
        if control.get_bit(9) {
            left += a;
        }
        if control.get_bit(8) {
            right += a;
        }
        if control.get_bit(13) {
            left += b;
        }
        if control.get_bit(12) {
            right += b;
        }

        // No pre-attenuation: a single active channel plays at full scale like
        // hardware, and the final mix clamps if two channels sum past 1.0.
        (left, right)
    }

    fn mix_psg(&self) -> (f32, f32) {
        let outputs = [
            self.square_output(0),
            self.square_output(1),
            self.wave_output(),
            self.noise_output(),
        ];

        let panning = self.control_stereo_volume_enable;
        let mut left = 0.0;
        let mut right = 0.0;
        for (i, &out) in outputs.iter().enumerate() {
            let i = i as u8;
            if panning.get_bit(12 + i) {
                left += out;
            }
            if panning.get_bit(8 + i) {
                right += out;
            }
        }

        // Master volume (0..7) and the PSG output ratio from SOUNDCNT_H.
        let left_master = (f32::from(panning.get_bits(4..=6)) + 1.0) / 8.0;
        let right_master = (f32::from(panning.get_bits(0..=2)) + 1.0) / 8.0;
        let ratio = match self.control_mixing_dma_control.get_bits(0..=1) {
            0 => 0.25,
            1 => 0.5,
            _ => 1.0,
        };

        // Four channels sum into each side; normalize before applying gain.
        (
            left / 4.0 * ratio * left_master,
            right / 4.0 * ratio * right_master,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SOUNDCNT_H bits: A right (8), A left (9), A timer (10), B right (12),
    // B left (13), B timer (14).
    const A_LEFT: u16 = 1 << 9;
    const A_RIGHT: u16 = 1 << 8;
    const MASTER_ON: u16 = 1 << 7;

    #[test]
    fn timer_overflow_pops_selected_channel() {
        // control_mixing_dma_control is 0 by default: both channels on timer 0.
        let mut s = Sound::default();
        s.push_fifo(0, 0x40);

        // Timer 1 must not touch a timer-0 channel.
        let refill = s.on_timer_overflow(1);
        assert_eq!(refill, [false, false]);
        assert_eq!(s.sample_a, 0);

        // Timer 0 pops the queued byte into the held sample.
        let refill = s.on_timer_overflow(0);
        assert_eq!(s.sample_a, 0x40);
        assert!(refill[0]); // drained below the refill threshold
    }

    #[test]
    fn refill_requested_only_below_threshold() {
        let mut s = Sound::default();
        for _ in 0..FIFO_CAPACITY {
            s.push_fifo(0, 1);
        }
        // Pops that leave the FIFO above half do not request a refill.
        while s.fifo_a.len() > FIFO_REFILL_THRESHOLD + 1 {
            assert!(!s.on_timer_overflow(0)[0]);
        }
        // The pop that reaches the threshold asks for a refill.
        assert!(s.on_timer_overflow(0)[0]);
    }

    #[test]
    fn fifo_write_drops_when_full() {
        let mut s = Sound::default();
        for _ in 0..(FIFO_CAPACITY + 8) {
            s.push_fifo(0, 1);
        }
        assert_eq!(s.fifo_a.len(), FIFO_CAPACITY);
    }

    #[test]
    fn mix_honors_master_enable_and_panning() {
        let mut s = Sound {
            sample_a: 127,
            control_mixing_dma_control: A_LEFT | A_RIGHT | (1 << 2), // full volume
            ..Default::default()
        };
        // Master off silences everything.
        assert_eq!(s.mix(), (0.0, 0.0));

        s.control_sound_on_off = MASTER_ON;
        let (l, r) = s.mix();
        assert!(l > 0.0 && r > 0.0);
    }

    #[test]
    fn step_emits_stereo_frames_at_output_rate() {
        let (producer, consumer) = rtrb::RingBuffer::new(64);
        let mut s = Sound {
            control_sound_on_off: MASTER_ON,
            ..Default::default()
        };
        s.set_audio_out(producer, 32768); // 512 cycles per sample

        s.step(512);
        assert_eq!(consumer.slots(), 2); // one left + one right
    }

    #[test]
    fn dma_sound_single_channel_plays_at_full_scale() {
        let s = Sound {
            sample_a: 127, // max 8-bit signed sample
            // SOUNDCNT_H: bit 2 = channel A full volume, bits 9 and 8 = A to
            // the left and right outputs.
            control_mixing_dma_control: (1 << 2) | (1 << 9) | (1 << 8),
            ..Default::default()
        };
        let (left, right) = s.mix_dma_sound();
        assert!(left > 0.9, "single channel must be near full scale: {left}");
        assert!((left - right).abs() < 1e-6);
    }

    #[test]
    fn dc_block_removes_constant_offset() {
        let mut s = Sound::default();
        // An idle enabled channel feeds a constant level like dac(0) = -1.0.
        let mut out = 0.0;
        for _ in 0..10000 {
            out = s.dc_block(0, -1.0);
        }
        assert!(
            out.abs() < 0.01,
            "constant DC must decay toward zero: {out}"
        );
    }

    #[test]
    fn dc_block_passes_the_initial_step() {
        let mut s = Sound::default();
        // The first sample of a step passes through before the offset decays.
        assert!((s.dc_block(0, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn nearly_full_ring_keeps_stereo_parity() {
        // With only an odd number of free slots, a naive push would emit the
        // left sample and drop the right, swapping the channels forever. The
        // frame must be emitted atomically, so the pushed count stays even.
        let (producer, consumer) = rtrb::RingBuffer::new(3);
        let mut s = Sound {
            control_sound_on_off: MASTER_ON,
            ..Default::default()
        };
        s.set_audio_out(producer, 32768);

        // First frame uses two of the three slots, leaving one free.
        s.emit_sample();
        // Second frame cannot fit a full stereo pair, so it emits nothing.
        s.emit_sample();

        assert_eq!(
            consumer.slots() % 2,
            0,
            "pushed sample count must stay even"
        );
        assert_eq!(consumer.slots(), 2);
    }

    #[test]
    fn triggering_square_enables_and_reloads_length() {
        // Non-zero envelope volume so the DAC is on.
        let mut s = Sound {
            channel1_duty_length_envelope: 0xF000,
            channel1_frequency_control: 0x0400, // freq bits set, mid range
            ..Default::default()
        };
        s.psg_register_written(0x0400_0065, 0x80); // trigger

        assert!(s.square1.enabled);
        assert_eq!(s.square1.length, 64);
        assert_eq!(s.square1.env_volume, 15);
    }

    #[test]
    fn length_counter_disables_square_when_it_expires() {
        let mut s = Sound {
            channel1_duty_length_envelope: 0xF000,
            channel1_frequency_control: 1 << 14, // length enable
            ..Default::default()
        };
        s.square1.enabled = true;
        s.square1.length = 1;

        // Two 256 Hz ticks land on frame-sequencer steps 0 and 2.
        s.frame_sequencer_tick();
        assert!(!s.square1.enabled);
    }

    #[test]
    fn dac_off_silences_square() {
        let mut s = Sound::default();
        s.square1.enabled = true;
        // Envelope register with zero volume and zero direction: DAC off.
        s.channel1_duty_length_envelope = 0;
        s.psg_register_written(0x0400_0063, 0);
        assert!(!s.square1.enabled);
    }
}
