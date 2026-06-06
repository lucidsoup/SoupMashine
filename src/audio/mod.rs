//! Real-time audio output. Only built with `--features audio` (pulls in cpal,
//! which needs a host audio API such as ALSA on Linux).

#[cfg(feature = "audio")]
pub mod cpal_sink;
