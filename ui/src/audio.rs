//! # Audio Output
//!
//! Bridges the emulator's DMA sound engine to the host audio device with
//! `cpal`. The emulator produces interleaved stereo `f32` frames into a
//! lock-free ring; the audio callback drains that ring at the device rate.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tracing::{info, warn};

/// Keeps the output stream alive. Dropping it stops playback.
pub struct AudioPlayer {
    _stream: cpal::Stream,
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

    let stream = device
        .build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                for frame in data.chunks_mut(channels) {
                    let left = consumer.pop().unwrap_or(0.0);
                    let right = consumer.pop().unwrap_or(0.0);
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

    Some(AudioPlayer { _stream: stream })
}
