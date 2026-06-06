//! SoupMashine groovebox — binary entrypoint.
//!
//! Build matrix:
//!   cargo run                          headless: render a demo beat to a WAV
//!   cargo run --features audio         headless: play the demo beat live
//!   cargo run --features full          standalone groovebox on a real MK2
//!   cargo run --features hardware      MK2 LEDs/displays without audio (diag)

use soupmashine::engine::{Engine, Pattern, SAMPLE_RATE};

#[cfg(feature = "hardware")]
use soupmashine::{
    app::App,
    protocol::InputParser,
    transport::{HidTransport, Transport},
    ui,
};

fn banner() {
    println!("==========================================");
    println!("  SoupMashine - Maschine MK2 Groovebox");
    println!("==========================================");
}

/// Program a basic one-bar demo beat into a pattern.
fn demo_pattern(pat: &mut Pattern) {
    // pad 0 = kick, 1 = snare, 2 = closed hat, 4 = clap (see synth::default_kit)
    for &s in &[0, 4, 8, 12] {
        pat.set(0, s, 110); // four-on-the-floor kick
    }
    pat.set(1, 4, 100); // snare on the backbeats
    pat.set(1, 12, 100);
    for s in (0..16).step_by(2) {
        pat.set(2, s, 70); // closed hats on the off-eighths
    }
    pat.set(4, 8, 90); // clap
}

fn main() {
    banner();
    let mut engine = Engine::new(SAMPLE_RATE);
    engine.load_default_kit();
    engine.sequencer.bpm = 120.0;
    demo_pattern(&mut engine.sequencer.pattern);

    #[cfg(feature = "hardware")]
    {
        let args: Vec<String> = std::env::args().collect();

        // `--list` / `--list-usb`: dump USB descriptors and exit.
        if args.iter().any(|a| a == "--list" || a == "--list-usb") {
            match soupmashine::transport::HidTransport::describe() {
                Ok(report) => print!("{report}"),
                Err(e) => eprintln!("USB enumeration failed: {e}"),
            }
            return;
        }

        // `--monitor`: print raw input reports as you press things (for mapping).
        if args.iter().any(|a| a == "--monitor") {
            run_monitor();
            return;
        }

        // `--interface N`: override the auto-detected control interface.
        let forced_iface = args
            .iter()
            .position(|a| a == "--interface")
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<u8>().ok());

        run_with_hardware(engine, forced_iface);
    }

    #[cfg(not(feature = "hardware"))]
    run_headless(engine);
}

// ---------------------------------------------------------------------------
// Headless (no controller attached)
// ---------------------------------------------------------------------------

#[cfg(all(not(feature = "hardware"), feature = "audio"))]
fn run_headless(engine: Engine) {
    use std::sync::{Arc, Mutex};
    let engine = Arc::new(Mutex::new(engine));
    engine.lock().unwrap().play();
    match soupmashine::audio::cpal_sink::start(engine.clone()) {
        Ok(_stream) => {
            println!("No controller attached. Playing demo beat live.");
            println!("Press Ctrl-C to stop.");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Err(e) => eprintln!("could not start audio: {e}"),
    }
}

#[cfg(all(not(feature = "hardware"), not(feature = "audio")))]
fn run_headless(mut engine: Engine) {
    use soupmashine::engine::sample::write_wav16;

    println!("No controller and no audio backend (default build).");
    println!("Rendering the demo beat to a WAV file instead.");

    engine.play();
    let sr = engine.sample_rate();
    let channels = 2usize;
    let seconds = 8.0f32;
    let total_frames = (sr as f32 * seconds) as usize;
    let mut out = vec![0f32; total_frames * channels];

    let block = 1024;
    let mut off = 0;
    while off < total_frames {
        let n = block.min(total_frames - off);
        engine.render(&mut out[off * channels..(off + n) * channels], channels);
        off += n;
    }

    let path = "soupmashine-demo.wav";
    match std::fs::File::create(path)
        .and_then(|mut f| write_wav16(&mut f, &out, channels as u16, sr))
    {
        Ok(()) => println!("Wrote {seconds:.0}s demo ({}) to {path}", fmt_bpm(&engine)),
        Err(e) => eprintln!("failed to write {path}: {e}"),
    }
}

#[cfg(all(not(feature = "hardware"), not(feature = "audio")))]
fn fmt_bpm(engine: &Engine) -> String {
    format!("{} BPM", engine.sequencer.bpm.round() as i32)
}

// ---------------------------------------------------------------------------
// Hardware (real Maschine MK2)
// ---------------------------------------------------------------------------

/// Print raw input reports (and the events we currently decode) as the user
/// presses pads/buttons/encoders. Consecutive identical reports are collapsed so
/// held pads don't flood the output. Used to map buttons against real hardware.
#[cfg(feature = "hardware")]
fn run_monitor() {
    if let Err(e) = HidTransport::monitor_raw() {
        eprintln!("Could not monitor Maschine MK2: {e}");
    }
}

#[cfg(feature = "hardware")]
fn run_with_hardware(engine: Engine, forced_iface: Option<u8>) {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    let opened = match forced_iface {
        Some(n) => {
            println!("Opening Maschine MK2 (forced interface {n})...");
            HidTransport::open_interface(n)
        }
        None => HidTransport::open(),
    };

    let mut transport = match opened {
        Ok(t) => {
            println!("Connected to Maschine MK2. Clearing splash screen...");
            t
        }
        Err(e) => {
            eprintln!("Could not open Maschine MK2: {e}");
            // Show what is actually on the bus to help diagnose.
            if let Ok(report) = HidTransport::describe() {
                eprintln!("\nUSB devices seen:\n{report}");
            }
            eprintln!("Tip: run `soupmashine --list` to inspect, or try `--interface N`.");
            return;
        }
    };

    let engine = Arc::new(Mutex::new(engine));

    #[cfg(feature = "audio")]
    let _stream = match soupmashine::audio::cpal_sink::start(engine.clone()) {
        Ok(s) => {
            println!("Audio output started.");
            Some(s)
        }
        Err(e) => {
            eprintln!("Audio unavailable ({e}); running without sound.");
            None
        }
    };

    let mut app = App::new();
    let mut parser = InputParser::new();
    let mut buf = [0u8; 64];
    let mut last_ui = Instant::now();
    #[cfg(not(feature = "audio"))]
    let mut last_tick = Instant::now();

    println!("Running. PLAY = transport, REC = record, GRID = step mode.");

    loop {
        // 1. Read and dispatch hardware input.
        let n = transport.read_input(&mut buf, 4).unwrap_or(0);
        if n > 0 {
            let events = parser.parse(&buf[..n]);
            if !events.is_empty() {
                let mut eng = engine.lock().unwrap();
                for ev in events {
                    app.handle_event(ev, &mut eng);
                }
            }
        }

        // 2. Without an audio thread, advance the sequencer here so the
        //    playhead/LEDs still move (silently).
        #[cfg(not(feature = "audio"))]
        {
            let elapsed = last_tick.elapsed();
            last_tick = Instant::now();
            let frames = (elapsed.as_secs_f64() * SAMPLE_RATE as f64) as usize;
            if frames > 0 {
                let mut sink = vec![0f32; frames * 2];
                engine.lock().unwrap().render(&mut sink, 2);
            }
        }

        // 3. Refresh LEDs and displays at ~60 Hz.
        if last_ui.elapsed() >= Duration::from_millis(16) {
            last_ui = Instant::now();
            let (leds, displays) = {
                let eng = engine.lock().unwrap();
                (ui::build_leds(&app, &eng), ui::build_displays(&app, &eng))
            };
            let reports = leds.encode();
            let _ = transport.write_output(&reports.pads);
            let _ = transport.write_output(&reports.groups);
            let _ = transport.write_output(&reports.buttons);
            for (i, fb) in displays.iter().enumerate() {
                for chunk in fb.encode_chunks(i as u8) {
                    let _ = transport.write_display(&chunk);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}
