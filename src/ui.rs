//! Rendering of the app state onto the pad LEDs and the two displays.

use crate::app::{App, PadMode};
use crate::engine::Engine;
use crate::protocol::device::{NUM_DISPLAYS, NUM_PADS};
use crate::protocol::{Button, Color, Framebuffer, LedState};

/// Fixed per-pad color palette.
pub const PAD_PALETTE: [Color; NUM_PADS] = [
    Color::new(255, 40, 40),   // kick
    Color::new(255, 140, 0),   // snare
    Color::new(255, 220, 0),   // closed hat
    Color::new(180, 255, 0),   // open hat
    Color::new(0, 255, 80),    // clap
    Color::new(0, 255, 200),   // rim
    Color::new(0, 200, 255),   // low tom
    Color::new(0, 120, 255),   // mid tom
    Color::new(80, 60, 255),   // cowbell
    Color::new(160, 0, 255),   // tone
    Color::new(220, 0, 220),   // tone
    Color::new(255, 0, 140),   // tone
    Color::new(255, 60, 120),  // tone
    Color::new(120, 255, 120), // tone
    Color::new(120, 200, 255), // tone
    Color::new(220, 220, 220), // tone
];

/// Build the full LED state for the current frame.
pub fn build_leds(app: &App, engine: &Engine) -> LedState {
    let mut leds = LedState::new();
    let seq = &engine.sequencer;

    match app.mode {
        PadMode::Play => {
            for (pad, &color) in PAD_PALETTE.iter().enumerate() {
                let mut c = color.dim(0.18);
                if pad == app.active_track {
                    c = color.dim(0.6);
                }
                // Flash pads that fire on the current step while playing.
                if seq.playing && seq.pattern.is_on(pad, seq.current_step) {
                    c = color;
                }
                leds.pads[pad] = c;
            }
        }
        PadMode::Step => {
            let track = app.active_track;
            for pad in 0..NUM_PADS {
                let step = app.page * 16 + pad;
                let mut c = Color::BLACK;
                if step < seq.pattern.steps {
                    if seq.pattern.is_on(track, step) {
                        c = PAD_PALETTE[track].dim(0.8);
                    } else {
                        c = PAD_PALETTE[track].dim(0.06);
                    }
                    if seq.playing && seq.current_step == step {
                        c = Color::WHITE;
                    }
                }
                leds.pads[pad] = c;
            }
        }
    }

    // Group LEDs: light the active track's group, roughly.
    if app.active_track / 2 < leds.groups.len() {
        leds.groups[app.active_track / 2] = PAD_PALETTE[app.active_track].dim(0.5);
    }

    // Transport / mode button LEDs (offsets are best-effort; see protocol notes).
    let set = |leds: &mut LedState, b: Button, on: bool| {
        let i = b.bit_index();
        if i < leds.buttons.len() {
            leds.buttons[i] = if on { 127 } else { 0 };
        }
    };
    set(&mut leds, Button::Play, seq.playing);
    set(&mut leds, Button::Rec, app.recording);
    set(&mut leds, Button::Grid, app.mode == PadMode::Step);
    set(&mut leds, Button::Shift, app.shift);

    leds
}

/// Build both display framebuffers.
pub fn build_displays(app: &App, engine: &Engine) -> [Framebuffer; NUM_DISPLAYS] {
    [build_left(app, engine), build_right(app, engine)]
}

fn build_left(app: &App, engine: &Engine) -> Framebuffer {
    let mut fb = Framebuffer::new();
    let seq = &engine.sequencer;

    fb.draw_text("SOUPMASHINE", 2, 0, 1);
    fb.hline(0, 9, 256, true);

    // Big BPM readout.
    fb.draw_text("BPM", 2, 14, 1);
    fb.draw_text(&format!("{:>3}", seq.bpm.round() as i32), 2, 24, 3);

    // Transport / mode status.
    let transport = if seq.playing { "PLAY" } else { "STOP" };
    fb.draw_text(transport, 120, 14, 2);
    if app.recording {
        fb.draw_text("REC", 120, 32, 2);
    }

    let mode = match app.mode {
        PadMode::Play => "MODE PLAY",
        PadMode::Step => "MODE STEP",
    };
    fb.draw_text(mode, 2, 50, 1);
    fb.draw_text(&format!("TRK {:>2}", app.active_track + 1), 120, 50, 1);
    fb.draw_text(&format!("STEP {:>2}", seq.current_step + 1), 180, 50, 1);
    fb
}

fn build_right(app: &App, engine: &Engine) -> Framebuffer {
    let mut fb = Framebuffer::new();
    let seq = &engine.sequencer;
    let track = app.active_track;

    fb.draw_text(&format!("TRACK {:>2}", track + 1), 2, 0, 1);
    fb.hline(0, 9, 256, true);

    // Step grid for the active track: 16 cells across, 16 px each.
    let cell = 15;
    let top = 18;
    let height = 28;
    for i in 0..16 {
        let step = app.page * 16 + i;
        let x = (i as i32) * (cell + 1);
        fb.rect(x, top, cell, height, true);
        if step < seq.pattern.steps && seq.pattern.is_on(track, step) {
            fb.fill_rect(x + 2, top + 2, cell - 4, height - 4, true);
        }
        // Playhead marker under the current step.
        if seq.playing && seq.current_step == step {
            fb.fill_rect(x, top + height + 2, cell, 3, true);
        }
    }

    fb.draw_text(&format!("PAGE {}", app.page + 1), 2, 54, 1);
    fb
}
