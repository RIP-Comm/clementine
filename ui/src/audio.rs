//! # Audio Output
//!
//! Bridges the emulator's sound engine to the host audio device with `cpal`.
//! The emulator produces interleaved stereo `f32` frames into a lock-free ring;
//! the audio callback drains that ring at the device rate and applies the user
//! controlled master gain.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn};

/// Master volume and mute, shared between the UI and the audio callback.
pub struct AudioControls {
    /// Volume in `[0.0, 1.0]`, stored as the bit pattern of an `f32`.
    volume_bits: AtomicU32,
    muted: AtomicBool,
}

impl AudioControls {
    const fn new() -> Self {
        Self {
            volume_bits: AtomicU32::new(1.0f32.to_bits()),
            muted: AtomicBool::new(false),
        }
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Effective gain applied to every sample.
    fn gain(&self) -> f32 {
        if self.muted() { 0.0 } else { self.volume() }
    }
}

/// Keeps the output stream alive and exposes its master controls. Dropping it
/// stops playback.
pub struct AudioPlayer {
    _stream: cpal::Stream,
    controls: Arc<AudioControls>,
}

impl AudioPlayer {
    /// Shared handle to the master volume and mute, for the UI to drive.
    pub fn controls(&self) -> Arc<AudioControls> {
        Arc::clone(&self.controls)
    }
}

/// Open the default output device and start playback.
///
/// The device's sample rate is only known here, so the ring buffer is created
/// through `make_consumer`, which receives that rate and returns the consumer
/// end of the emulator's sample ring. Returns `None` when no device is
/// available or the format is unsupported, in which case the emulator simply
/// runs muted.
pub fn start<F>(make_consumer: F) -> Option<AudioPlayer>
where
    F: FnOnce(u32) -> rtrb::Consumer<f32>,
{
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;

    if config.sample_format() != cpal::SampleFormat::F32 {
        warn!(
            "unsupported audio format {:?}, running muted",
            config.sample_format()
        );
        return None;
    }

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let mut consumer = make_consumer(sample_rate);

    info!("audio output: {sample_rate} Hz, {channels} channels");

    let controls = Arc::new(AudioControls::new());
    let callback_controls = Arc::clone(&controls);

    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                let gain = callback_controls.gain();
                for frame in data.chunks_mut(channels) {
                    let left = consumer.pop().unwrap_or(0.0) * gain;
                    let right = consumer.pop().unwrap_or(0.0) * gain;
                    if channels == 1 {
                        frame[0] = (left + right) * 0.5;
                    } else {
                        frame[0] = left;
                        frame[1] = right;
                        for extra in frame.iter_mut().skip(2) {
                            *extra = 0.0;
                        }
                    }
                }
            },
            |err| warn!("audio stream error: {err}"),
            None,
        )
        .ok()?;

    stream.play().ok()?;

    Some(AudioPlayer {
        _stream: stream,
        controls,
    })
}
