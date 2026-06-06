//! A per-pad step sequencer (16 tracks, up to 64 steps), 16th-note grid.

use crate::protocol::device::NUM_PADS;

pub const MAX_STEPS: usize = 64;

/// One pattern: a velocity grid of `NUM_PADS` tracks by `steps` steps.
/// A velocity of 0 means the step is off.
#[derive(Clone)]
pub struct Pattern {
    pub steps: usize,
    grid: [[u8; MAX_STEPS]; NUM_PADS],
}

impl Pattern {
    pub fn new(steps: usize) -> Self {
        Pattern {
            steps: steps.clamp(1, MAX_STEPS),
            grid: [[0; MAX_STEPS]; NUM_PADS],
        }
    }

    pub fn is_on(&self, pad: usize, step: usize) -> bool {
        pad < NUM_PADS && step < self.steps && self.grid[pad][step] > 0
    }

    pub fn velocity(&self, pad: usize, step: usize) -> u8 {
        if pad < NUM_PADS && step < self.steps {
            self.grid[pad][step]
        } else {
            0
        }
    }

    pub fn set(&mut self, pad: usize, step: usize, velocity: u8) {
        if pad < NUM_PADS && step < self.steps {
            self.grid[pad][step] = velocity;
        }
    }

    /// Toggle a step on/off (on uses `velocity`).
    pub fn toggle(&mut self, pad: usize, step: usize, velocity: u8) {
        if self.is_on(pad, step) {
            self.set(pad, step, 0);
        } else {
            self.set(pad, step, velocity.max(1));
        }
    }

    pub fn clear_track(&mut self, pad: usize) {
        if pad < NUM_PADS {
            self.grid[pad] = [0; MAX_STEPS];
        }
    }

    pub fn clear(&mut self) {
        self.grid = [[0; MAX_STEPS]; NUM_PADS];
    }
}

/// Drives a pattern in real time against the audio clock.
pub struct Sequencer {
    pub pattern: Pattern,
    pub bpm: f32,
    pub playing: bool,
    pub current_step: usize,
    sample_rate: u32,
    phase: f64, // samples elapsed within the current step
}

impl Sequencer {
    pub fn new(sample_rate: u32, steps: usize) -> Self {
        Sequencer {
            pattern: Pattern::new(steps),
            bpm: 120.0,
            playing: false,
            current_step: 0,
            sample_rate,
            phase: 0.0,
        }
    }

    /// Samples per 16th-note step at the current tempo.
    pub fn samples_per_step(&self) -> f64 {
        // 16th notes: four steps per beat.
        self.sample_rate as f64 * 60.0 / self.bpm as f64 / 4.0
    }

    pub fn start(&mut self) {
        self.playing = true;
        self.current_step = 0;
        self.phase = 0.0;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.current_step = 0;
        self.phase = 0.0;
    }

    /// Whole samples remaining until the next step boundary (>= 1).
    pub fn frames_until_boundary(&self) -> usize {
        let remaining = self.samples_per_step() - self.phase;
        remaining.ceil().max(1.0) as usize
    }

    /// Advance the clock by `frames`. Returns `Some(step)` if a new step just
    /// began (the caller should trigger that step's row).
    pub fn advance(&mut self, frames: usize) -> Option<usize> {
        if !self.playing {
            return None;
        }
        self.phase += frames as f64;
        let sps = self.samples_per_step();
        if self.phase >= sps {
            self.phase -= sps;
            self.current_step = (self.current_step + 1) % self.pattern.steps;
            Some(self.current_step)
        } else {
            None
        }
    }
}
