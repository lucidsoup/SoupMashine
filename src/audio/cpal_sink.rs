//! Streams the shared [`Engine`] to the default output device via cpal.

use crate::engine::Engine;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

/// Start a stereo f32 output stream. The returned [`cpal::Stream`] must be kept
/// alive for audio to keep playing.
pub fn start(engine: Arc<Mutex<Engine>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;

    let sample_rate = engine
        .lock()
        .map_err(|_| "engine lock poisoned".to_string())?
        .sample_rate();
    let channels = 2u16;

    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let err_fn = |e| eprintln!("audio stream error: {e}");
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| match engine.lock() {
                Ok(mut eng) => eng.render(data, channels as usize),
                Err(_) => data.iter_mut().for_each(|s| *s = 0.0),
            },
            err_fn,
            None,
        )
        .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}
