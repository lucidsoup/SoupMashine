//! The groovebox controller logic: turns hardware [`InputEvent`]s into engine
//! actions and holds the small amount of UI state needed to drive the LEDs and
//! displays.

use crate::engine::Engine;
use crate::protocol::{Button, InputEvent};

/// Operating mode for the 16 pads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadMode {
    /// Pads play their sound live (and record while armed).
    Play,
    /// Pads toggle steps of the active track (classic step sequencer).
    Step,
}

pub struct App {
    pub active_track: usize,
    pub recording: bool,
    pub mode: PadMode,
    pub shift: bool,
    /// Which bank of 16 steps the pads address in Step mode.
    pub page: usize,
    /// Latest pad velocity, for UI feedback.
    pub last_velocity: f32,
}

impl Default for App {
    fn default() -> Self {
        App {
            active_track: 0,
            recording: false,
            mode: PadMode::Play,
            shift: false,
            page: 0,
            last_velocity: 0.0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// Absolute step index for a pad in the current page.
    fn step_for_pad(&self, pad: usize) -> usize {
        self.page * 16 + pad
    }

    pub fn handle_event(&mut self, ev: InputEvent, engine: &mut Engine) {
        match ev {
            InputEvent::PadPressed { pad, velocity } => {
                let pad = pad as usize;
                self.last_velocity = velocity;
                match self.mode {
                    PadMode::Play => {
                        self.active_track = pad;
                        engine.trigger_pad(pad, velocity);
                        if self.recording && engine.sequencer.playing {
                            let step = engine.sequencer.current_step;
                            let v = (velocity * 127.0).max(1.0) as u8;
                            engine.sequencer.pattern.set(pad, step, v);
                        }
                    }
                    PadMode::Step => {
                        if self.shift {
                            // Pick which track to edit.
                            self.active_track = pad;
                            engine.trigger_pad(pad, 0.8);
                        } else {
                            let step = self.step_for_pad(pad);
                            engine
                                .sequencer
                                .pattern
                                .toggle(self.active_track, step, 100);
                        }
                    }
                }
            }
            InputEvent::Button { button, pressed } => self.handle_button(button, pressed, engine),
            InputEvent::Encoder { index, delta } => self.handle_encoder(index, delta, engine),
            InputEvent::PadReleased { .. } | InputEvent::PadAftertouch { .. } => {}
        }
    }

    fn handle_button(&mut self, button: Button, pressed: bool, engine: &mut Engine) {
        match button {
            Button::Shift => self.shift = pressed,
            // The rest act on press only.
            _ if !pressed => {}
            Button::Play => engine.toggle_play(),
            Button::Rec => self.recording = !self.recording,
            Button::Grid | Button::PadMode => {
                self.mode = match self.mode {
                    PadMode::Play => PadMode::Step,
                    PadMode::Step => PadMode::Play,
                };
            }
            Button::Erase | Button::Clear => {
                if self.shift {
                    engine.sequencer.pattern.clear();
                } else {
                    engine.sequencer.pattern.clear_track(self.active_track);
                }
            }
            Button::StepLeft => self.page = self.page.saturating_sub(1),
            Button::StepRight => {
                let max_page = (engine.sequencer.pattern.steps.saturating_sub(1)) / 16;
                self.page = (self.page + 1).min(max_page);
            }
            _ => {}
        }
    }

    fn handle_encoder(&mut self, index: u8, delta: i32, engine: &mut Engine) {
        match index {
            0 => {
                // Main encoder: tempo.
                let bpm = (engine.sequencer.bpm + delta as f32).clamp(20.0, 300.0);
                engine.sequencer.bpm = bpm;
            }
            1 => {
                engine.master_gain = (engine.master_gain + delta as f32 * 0.02).clamp(0.0, 1.5);
            }
            _ => {}
        }
    }
}
