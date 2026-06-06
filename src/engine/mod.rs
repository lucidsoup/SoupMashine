//! The audio engine: a polyphonic sampler driven live by the pads and in time
//! by the step sequencer.

pub mod sample;
pub mod sampler;
pub mod sequencer;
pub mod synth;

pub use sample::Sample;
pub use sampler::Sampler;
pub use sequencer::{Pattern, Sequencer};

/// Default project sample rate.
pub const SAMPLE_RATE: u32 = 44_100;

pub struct Engine {
    pub sampler: Sampler,
    pub sequencer: Sequencer,
    pub master_gain: f32,
    sample_rate: u32,
    scratch: Vec<f32>,
}

impl Engine {
    pub fn new(sample_rate: u32) -> Self {
        Engine {
            sampler: Sampler::new(Vec::new()),
            sequencer: Sequencer::new(sample_rate, 16),
            master_gain: 0.8,
            sample_rate,
            scratch: Vec::new(),
        }
    }

    /// Load the built-in synthesized drum kit.
    pub fn load_default_kit(&mut self) {
        let kit = synth::default_kit(self.sample_rate);
        self.sampler = Sampler::new(kit);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Trigger a pad immediately (live play), velocity in [0.0, 1.0].
    pub fn trigger_pad(&mut self, pad: usize, velocity: f32) {
        self.sampler.trigger(pad, velocity, 1.0);
    }

    /// Start playback from the top, triggering the first step.
    pub fn play(&mut self) {
        self.sequencer.start();
        self.trigger_step(0);
    }

    pub fn stop(&mut self) {
        self.sequencer.stop();
    }

    pub fn toggle_play(&mut self) {
        if self.sequencer.playing {
            self.stop();
        } else {
            self.play();
        }
    }

    fn trigger_step(&mut self, step: usize) {
        for pad in 0..self.sampler.samples.len() {
            let vel = self.sequencer.pattern.velocity(pad, step);
            if vel > 0 {
                self.sampler.trigger(pad, vel as f32 / 127.0, 1.0);
            }
        }
    }

    /// Render interleaved audio into `out`. Sequenced steps are triggered
    /// sample-accurately at their boundaries.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        for s in out.iter_mut() {
            *s = 0.0;
        }
        if channels == 0 {
            return;
        }
        let frames = out.len() / channels;
        let mut done = 0;
        while done < frames {
            let remaining = frames - done;
            let block = if self.sequencer.playing {
                self.sequencer.frames_until_boundary().min(remaining)
            } else {
                remaining
            }
            .max(1);

            self.scratch.clear();
            self.scratch.resize(block, 0.0);
            self.sampler.render_add(&mut self.scratch);

            for i in 0..block {
                let v = self.scratch[i] * self.master_gain;
                let base = (done + i) * channels;
                for c in 0..channels {
                    out[base + c] += v;
                }
            }

            if let Some(step) = self.sequencer.advance(block) {
                self.trigger_step(step);
            }
            done += block;
        }
    }
}
