//! Polyphonic one-shot sampler: 16 pad slots, a fixed pool of voices.

use super::sample::Sample;
use crate::protocol::device::NUM_PADS;

const MAX_VOICES: usize = 32;

#[derive(Clone, Copy)]
struct Voice {
    pad: usize,
    pos: f64,
    rate: f64, // playback speed (1.0 = original pitch)
    gain: f32,
    age: u64,
    active: bool,
}

impl Voice {
    const fn silent() -> Self {
        Voice {
            pad: 0,
            pos: 0.0,
            rate: 1.0,
            gain: 0.0,
            age: 0,
            active: false,
        }
    }
}

pub struct Sampler {
    /// One sample per pad (some may be empty).
    pub samples: Vec<Sample>,
    voices: [Voice; MAX_VOICES],
    counter: u64,
}

impl Sampler {
    pub fn new(samples: Vec<Sample>) -> Self {
        let mut samples = samples;
        samples.resize_with(NUM_PADS, Sample::default);
        Sampler {
            samples,
            voices: [Voice::silent(); MAX_VOICES],
            counter: 0,
        }
    }

    pub fn set_sample(&mut self, pad: usize, sample: Sample) {
        if pad < self.samples.len() {
            self.samples[pad] = sample;
        }
    }

    /// Trigger a pad with velocity in [0.0, 1.0] and an optional pitch ratio.
    pub fn trigger(&mut self, pad: usize, velocity: f32, rate: f64) {
        if pad >= self.samples.len() || self.samples[pad].is_empty() {
            return;
        }
        self.counter += 1;
        let slot = self.alloc_voice();
        self.voices[slot] = Voice {
            pad,
            pos: 0.0,
            rate: rate.max(0.01),
            gain: velocity.clamp(0.0, 1.0),
            age: self.counter,
            active: true,
        };
    }

    /// Find a free voice, or steal the oldest active one.
    fn alloc_voice(&mut self) -> usize {
        if let Some(i) = self.voices.iter().position(|v| !v.active) {
            return i;
        }
        let mut oldest = 0;
        let mut oldest_age = u64::MAX;
        for (i, v) in self.voices.iter().enumerate() {
            if v.age < oldest_age {
                oldest_age = v.age;
                oldest = i;
            }
        }
        oldest
    }

    /// Mix all active voices additively into a mono buffer.
    pub fn render_add(&mut self, out: &mut [f32]) {
        for v in self.voices.iter_mut() {
            if !v.active {
                continue;
            }
            let sample = &self.samples[v.pad];
            let frames = &sample.frames;
            for o in out.iter_mut() {
                let i0 = v.pos.floor() as usize;
                if i0 + 1 >= frames.len() {
                    v.active = false;
                    break;
                }
                let frac = (v.pos - i0 as f64) as f32;
                let a = frames[i0];
                let b = frames[i0 + 1];
                *o += (a + (b - a) * frac) * v.gain;
                v.pos += v.rate;
            }
        }
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}
