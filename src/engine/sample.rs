//! Mono audio samples plus a small dependency-free WAV reader/writer.

use std::io::{self, Read, Write};
use std::path::Path;

/// A mono audio sample stored as f32 frames in [-1.0, 1.0].
#[derive(Debug, Clone, Default)]
pub struct Sample {
    pub frames: Vec<f32>,
    pub sample_rate: u32,
}

impl Sample {
    pub fn new(frames: Vec<f32>, sample_rate: u32) -> Self {
        Sample {
            frames,
            sample_rate,
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Return a copy resampled (linear interpolation) to `target_rate`.
    pub fn resampled(&self, target_rate: u32) -> Sample {
        if self.sample_rate == target_rate || self.frames.is_empty() {
            return self.clone();
        }
        let ratio = target_rate as f64 / self.sample_rate as f64;
        let out_len = (self.frames.len() as f64 * ratio).round() as usize;
        let mut out = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let src = i as f64 / ratio;
            let i0 = src.floor() as usize;
            let frac = (src - i0 as f64) as f32;
            let a = self.frames.get(i0).copied().unwrap_or(0.0);
            let b = self.frames.get(i0 + 1).copied().unwrap_or(a);
            out.push(a + (b - a) * frac);
        }
        Sample::new(out, target_rate)
    }

    /// Load a PCM WAV file (8/16/24-bit, any channel count) as mono f32.
    pub fn load_wav<P: AsRef<Path>>(path: P) -> io::Result<Sample> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut bytes)?;
        parse_wav(&bytes)
    }

    /// Write this sample as a 16-bit mono WAV.
    pub fn write_wav<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        write_wav16(&mut f, &self.frames, 1, self.sample_rate)
    }
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn parse_wav(b: &[u8]) -> io::Result<Sample> {
    let err = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(err("not a RIFF/WAVE file"));
    }
    let mut pos = 12;
    let mut channels = 1u16;
    let mut sample_rate = 44_100u32;
    let mut bits = 16u16;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = rd_u32(b, pos + 4) as usize;
        let body_start = pos + 8;
        let body_end = (body_start + size).min(b.len());
        if id == b"fmt " && body_end - body_start >= 16 {
            channels = rd_u16(b, body_start + 2).max(1);
            sample_rate = rd_u32(b, body_start + 4);
            bits = rd_u16(b, body_start + 14);
        } else if id == b"data" {
            data = Some(&b[body_start..body_end]);
        }
        // Chunks are word-aligned.
        pos = body_end + (size & 1);
    }

    let data = data.ok_or_else(|| err("no data chunk"))?;
    let frames = decode_pcm(data, channels, bits)?;
    Ok(Sample::new(frames, sample_rate))
}

/// Decode interleaved PCM into mono f32 (averaging channels).
fn decode_pcm(data: &[u8], channels: u16, bits: u16) -> io::Result<Vec<f32>> {
    let ch = channels as usize;
    let mut mono = Vec::new();
    match bits {
        8 => {
            // 8-bit WAV is unsigned.
            for frame in data.chunks_exact(ch) {
                let s: f32 = frame
                    .iter()
                    .map(|&v| (v as f32 - 128.0) / 128.0)
                    .sum::<f32>()
                    / ch as f32;
                mono.push(s);
            }
        }
        16 => {
            for frame in data.chunks_exact(ch * 2) {
                let mut acc = 0.0;
                for c in 0..ch {
                    let v = i16::from_le_bytes([frame[c * 2], frame[c * 2 + 1]]);
                    acc += v as f32 / 32768.0;
                }
                mono.push(acc / ch as f32);
            }
        }
        24 => {
            for frame in data.chunks_exact(ch * 3) {
                let mut acc = 0.0;
                for c in 0..ch {
                    let o = c * 3;
                    let raw = (frame[o] as i32)
                        | ((frame[o + 1] as i32) << 8)
                        | ((frame[o + 2] as i32) << 16);
                    // Sign-extend 24-bit.
                    let raw = (raw << 8) >> 8;
                    acc += raw as f32 / 8_388_608.0;
                }
                mono.push(acc / ch as f32);
            }
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported bit depth: {other}"),
            ));
        }
    }
    Ok(mono)
}

/// Write interleaved f32 frames as a 16-bit PCM WAV.
pub fn write_wav16<W: Write>(
    w: &mut W,
    frames: &[f32],
    channels: u16,
    sample_rate: u32,
) -> io::Result<()> {
    let bits = 16u16;
    let block_align = channels * bits / 8;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = frames.len() as u32 * 2;
    let riff_len = 36 + data_len;

    w.write_all(b"RIFF")?;
    w.write_all(&riff_len.to_le_bytes())?;
    w.write_all(b"WAVE")?;
    w.write_all(b"fmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&channels.to_le_bytes())?;
    w.write_all(&sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&bits.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;
    for &s in frames {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        w.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}
