//! Procedurally generated drum/synth kit, so the groovebox makes sound out of
//! the box with no external sample files. Everything is synthesized at the
//! engine's sample rate using a tiny xorshift noise source.

use super::sample::Sample;
use std::f32::consts::TAU;

/// Deterministic white-noise generator (xorshift32).
struct Noise(u32);
impl Noise {
    fn new(seed: u32) -> Self {
        Noise(seed | 1)
    }
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn env_exp(i: usize, len: usize, decay: f32) -> f32 {
    let t = i as f32 / len as f32;
    (-t * decay).exp()
}

fn kick(sr: u32) -> Sample {
    let len = (sr as f32 * 0.45) as usize;
    let mut f = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        let t = i as f32 / len as f32;
        let freq = 50.0 + 90.0 * (-t * 8.0).exp(); // pitch drop
        phase += TAU * freq / sr as f32;
        let amp = env_exp(i, len, 5.0);
        f.push((phase.sin()) * amp);
    }
    Sample::new(f, sr)
}

fn snare(sr: u32) -> Sample {
    let len = (sr as f32 * 0.25) as usize;
    let mut n = Noise::new(0x1234_5678);
    let mut f = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        phase += TAU * 180.0 / sr as f32;
        let tone = phase.sin() * 0.4;
        let noise = n.next() * 0.8;
        let amp = env_exp(i, len, 9.0);
        f.push((tone + noise) * amp);
    }
    Sample::new(f, sr)
}

fn hat(sr: u32, open: bool) -> Sample {
    let secs = if open { 0.25 } else { 0.05 };
    let len = (sr as f32 * secs) as usize;
    let mut n = Noise::new(if open { 0xC0FF_EE11 } else { 0xBEEF_0042 });
    let mut f = Vec::with_capacity(len);
    let mut hp = 0.0f32; // crude one-pole high-pass state
    for i in 0..len {
        let raw = n.next();
        let filtered = raw - hp;
        hp = raw * 0.15 + hp * 0.85;
        let amp = env_exp(i, len, if open { 6.0 } else { 30.0 });
        f.push(filtered * amp * 0.7);
    }
    Sample::new(f, sr)
}

fn clap(sr: u32) -> Sample {
    let len = (sr as f32 * 0.2) as usize;
    let mut n = Noise::new(0x5151_AAAA);
    let mut f = vec![0.0f32; len];
    let bursts = [0.0, 0.012, 0.024, 0.04];
    for (bi, &b) in bursts.iter().enumerate() {
        let start = (b * sr as f32) as usize;
        let blen = (sr as f32 * 0.05) as usize;
        for i in 0..blen {
            if start + i >= len {
                break;
            }
            let amp = env_exp(i, blen, 18.0) * if bi == bursts.len() - 1 { 1.0 } else { 0.6 };
            f[start + i] += n.next() * amp * 0.7;
        }
    }
    Sample::new(f, sr)
}

fn tom(sr: u32, base: f32) -> Sample {
    let len = (sr as f32 * 0.3) as usize;
    let mut f = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        let t = i as f32 / len as f32;
        let freq = base * (1.0 + 0.5 * (-t * 6.0).exp());
        phase += TAU * freq / sr as f32;
        f.push(phase.sin() * env_exp(i, len, 6.0));
    }
    Sample::new(f, sr)
}

fn rim(sr: u32) -> Sample {
    let len = (sr as f32 * 0.06) as usize;
    let mut f = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        phase += TAU * 1700.0 / sr as f32;
        f.push(phase.sin() * env_exp(i, len, 40.0));
    }
    Sample::new(f, sr)
}

fn cowbell(sr: u32) -> Sample {
    let len = (sr as f32 * 0.25) as usize;
    let mut f = Vec::with_capacity(len);
    let (mut p1, mut p2) = (0.0f32, 0.0f32);
    for i in 0..len {
        p1 += TAU * 540.0 / sr as f32;
        p2 += TAU * 800.0 / sr as f32;
        let sq = |p: f32| if p.sin() >= 0.0 { 1.0 } else { -1.0 };
        f.push((sq(p1) + sq(p2)) * 0.25 * env_exp(i, len, 7.0));
    }
    Sample::new(f, sr)
}

/// A simple pitched tone (saw-ish) for melodic pads. `note` is a MIDI note.
fn tone(sr: u32, note: u8) -> Sample {
    let freq = 440.0 * 2f32.powf((note as f32 - 69.0) / 12.0);
    let len = (sr as f32 * 0.4) as usize;
    let mut f = Vec::with_capacity(len);
    let mut phase = 0.0f32;
    for i in 0..len {
        phase += freq / sr as f32;
        if phase >= 1.0 {
            phase -= 1.0;
        }
        let saw = phase * 2.0 - 1.0;
        f.push(saw * 0.5 * env_exp(i, len, 4.0));
    }
    Sample::new(f, sr)
}

/// Build the default 16-pad kit (logical pad order, index 0 = bottom-left).
pub fn default_kit(sr: u32) -> Vec<Sample> {
    vec![
        kick(sr),       // 0
        snare(sr),      // 1
        hat(sr, false), // 2 closed hat
        hat(sr, true),  // 3 open hat
        clap(sr),       // 4
        rim(sr),        // 5
        tom(sr, 100.0), // 6 low tom
        tom(sr, 160.0), // 7 mid tom
        cowbell(sr),    // 8
        tone(sr, 48),   // 9  C2
        tone(sr, 50),   // 10 D2
        tone(sr, 52),   // 11 E2
        tone(sr, 55),   // 12 G2
        tone(sr, 57),   // 13 A2
        tone(sr, 60),   // 14 C3
        tone(sr, 64),   // 15 E3
    ]
}
