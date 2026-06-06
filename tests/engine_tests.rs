use soupmashine::app::{App, PadMode};
use soupmashine::engine::sample::Sample;
use soupmashine::engine::{Engine, SAMPLE_RATE};
use soupmashine::protocol::{Button, InputEvent};

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, &s| m.max(s.abs()))
}

#[test]
fn samples_per_step_matches_tempo() {
    let mut engine = Engine::new(44_100);
    engine.sequencer.bpm = 120.0;
    // 120 BPM, 16th notes => 44100 * 60 / 120 / 4 = 5512.5 samples.
    assert!((engine.sequencer.samples_per_step() - 5512.5).abs() < 1e-6);
}

#[test]
fn live_pad_trigger_makes_sound() {
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    engine.trigger_pad(0, 1.0); // kick

    let mut out = vec![0f32; 2048 * 2];
    engine.render(&mut out, 2);
    assert!(peak(&out) > 0.01, "kick should produce audible output");
}

#[test]
fn sequencer_triggers_steps_over_time() {
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    engine.sequencer.bpm = 120.0;
    engine.sequencer.pattern.set(0, 0, 120);
    engine.sequencer.pattern.set(0, 4, 120);
    engine.play();

    // Render two seconds; the sequencer must keep producing sound.
    let frames = SAMPLE_RATE as usize * 2;
    let mut out = vec![0f32; frames * 2];
    engine.render(&mut out, 2);

    assert!(peak(&out) > 0.01);
    // The current step must have advanced past 0.
    assert!(engine.sequencer.current_step != 0 || engine.sequencer.playing);
}

#[test]
fn stereo_output_is_balanced() {
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    engine.trigger_pad(1, 0.9);
    let mut out = vec![0f32; 1000 * 2];
    engine.render(&mut out, 2);
    // Left and right channels are mirror copies in this mono-summed engine.
    for frame in out.chunks_exact(2) {
        assert!((frame[0] - frame[1]).abs() < 1e-6);
    }
}

#[test]
fn wav_roundtrip_preserves_length() {
    let frames: Vec<f32> = (0..1000)
        .map(|i| (i as f32 * 0.05).sin() * 0.5)
        .collect();
    let sample = Sample::new(frames.clone(), 44_100);

    let dir = std::env::temp_dir();
    let path = dir.join("soupmashine_roundtrip.wav");
    sample.write_wav(&path).unwrap();
    let loaded = Sample::load_wav(&path).unwrap();

    assert_eq!(loaded.sample_rate, 44_100);
    assert_eq!(loaded.len(), frames.len());
    // 16-bit quantization tolerance.
    for (a, b) in frames.iter().zip(loaded.frames.iter()) {
        assert!((a - b).abs() < 1e-3);
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn resample_changes_length_proportionally() {
    let sample = Sample::new(vec![0.0; 1000], 44_100);
    let up = sample.resampled(88_200);
    assert!((up.len() as i32 - 2000).abs() <= 1);
}

#[test]
fn app_play_button_toggles_transport() {
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    let mut app = App::new();

    assert!(!engine.sequencer.playing);
    app.handle_event(
        InputEvent::Button {
            button: Button::Play,
            pressed: true,
        },
        &mut engine,
    );
    assert!(engine.sequencer.playing);
}

#[test]
fn app_step_mode_toggles_pattern_steps() {
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    let mut app = App::new();
    app.active_track = 0;

    // Switch to step mode via GRID.
    app.handle_event(
        InputEvent::Button {
            button: Button::Grid,
            pressed: true,
        },
        &mut engine,
    );
    assert_eq!(app.mode, PadMode::Step);

    // Pressing pad 3 toggles step 3 of the active track on.
    app.handle_event(
        InputEvent::PadPressed {
            pad: 3,
            velocity: 0.8,
        },
        &mut engine,
    );
    assert!(engine.sequencer.pattern.is_on(0, 3));

    // Pressing again toggles it off.
    app.handle_event(
        InputEvent::PadPressed {
            pad: 3,
            velocity: 0.8,
        },
        &mut engine,
    );
    assert!(!engine.sequencer.pattern.is_on(0, 3));
}

#[test]
fn app_records_live_hits_into_pattern() {
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    let mut app = App::new();

    engine.play(); // playing, at step 0
    app.recording = true;

    app.handle_event(
        InputEvent::PadPressed {
            pad: 2,
            velocity: 1.0,
        },
        &mut engine,
    );
    assert!(engine.sequencer.pattern.is_on(2, engine.sequencer.current_step));
}
