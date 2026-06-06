//! Decoding of Maschine MK2 input reports into high-level events.

use super::button::Button;
use super::device::*;
use core::convert::TryFrom;

/// A decoded hardware event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    /// A pad crossed the press threshold. `velocity` is in [0.0, 1.0].
    PadPressed { pad: u8, velocity: f32 },
    /// A pad fell back below the press threshold.
    PadReleased { pad: u8 },
    /// Continuous pressure update while a pad is held (aftertouch).
    PadAftertouch { pad: u8, pressure: f32 },
    /// A button edge. `pressed` is true on press, false on release.
    Button { button: Button, pressed: bool },
    /// Relative movement of an encoder. Index 0 is the main (master) encoder,
    /// 1..=8 are the eight display encoders. `delta` is signed detents.
    Encoder { index: u8, delta: i32 },
}

/// Stateful decoder. It remembers the previous report so it can emit edges and
/// relative encoder deltas.
#[derive(Debug)]
pub struct InputParser {
    buttons: u64,                // bitset of the 48 buttons
    main_encoder: u8,            // 0x0..0x0F
    encoders: [u16; 8],          // raw 16-bit display-encoder values
    pad_pressure: [u16; NUM_PADS], // last pressure per logical pad
    pad_down: [bool; NUM_PADS],
    initialized: bool,
}

impl Default for InputParser {
    fn default() -> Self {
        InputParser {
            buttons: 0,
            main_encoder: 0,
            encoders: [0; 8],
            pad_pressure: [0; NUM_PADS],
            pad_down: [false; NUM_PADS],
            initialized: false,
        }
    }
}

impl InputParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode one raw report (including the leading report-id byte).
    pub fn parse(&mut self, report: &[u8]) -> Vec<InputEvent> {
        match report.first().copied() {
            Some(INPUT_REPORT_BUTTONS) => self.parse_buttons(report),
            Some(INPUT_REPORT_PADS) => self.parse_pads(report),
            _ => Vec::new(),
        }
    }

    fn parse_buttons(&mut self, report: &[u8]) -> Vec<InputEvent> {
        let mut events = Vec::new();
        // 6 bytes of button bits starting at offset 1.
        if report.len() < 1 + 6 {
            return events;
        }
        let mut bits: u64 = 0;
        for byte in 0..6 {
            bits |= (report[1 + byte] as u64) << (byte * 8);
        }
        let changed = bits ^ self.buttons;
        if changed != 0 {
            for idx in 0..NUM_BUTTONS {
                if (changed >> idx) & 1 == 1 {
                    if let Ok(button) = Button::try_from(idx) {
                        events.push(InputEvent::Button {
                            button,
                            pressed: (bits >> idx) & 1 == 1,
                        });
                    }
                }
            }
        }
        self.buttons = bits;

        // Main encoder: single nibble right after the button bytes.
        let main_off = 1 + 6;
        if report.len() > main_off {
            let val = report[main_off] & 0x0F;
            if self.initialized {
                let delta = wrapped_delta_4bit(self.main_encoder, val);
                if delta != 0 {
                    events.push(InputEvent::Encoder { index: 0, delta });
                }
            }
            self.main_encoder = val;
        }

        // Eight display encoders: 2 bytes (little-endian) each.
        let enc_off = main_off + 1;
        for i in 0..8 {
            let lo = enc_off + i * 2;
            let hi = lo + 1;
            if hi >= report.len() {
                break;
            }
            let val = (report[lo] as u16) | ((report[hi] as u16) << 8);
            if self.initialized {
                let delta = wrapped_delta_16bit(self.encoders[i], val);
                if delta != 0 {
                    events.push(InputEvent::Encoder {
                        index: (i + 1) as u8,
                        delta,
                    });
                }
            }
            self.encoders[i] = val;
        }

        self.initialized = true;
        events
    }

    fn parse_pads(&mut self, report: &[u8]) -> Vec<InputEvent> {
        let mut events = Vec::new();
        // Repeating 2-byte entries starting at offset 1:
        //   byte0 = low 8 bits of pressure
        //   byte1 = (pad_index << 4) | high 4 bits of pressure
        let mut i = 1;
        while i + 1 < report.len() {
            let low = report[i] as u16;
            let high = report[i + 1] as u16;
            i += 2;
            let pos = ((high & 0xF0) >> 4) as usize;
            let pressure = ((high & 0x0F) << 8) | low;
            if pos >= NUM_PADS {
                continue;
            }
            let pad = report_pos_to_logical(pos);
            self.pad_pressure[pad] = pressure;

            let now_down = pressure >= PAD_THRESHOLD;
            let was_down = self.pad_down[pad];
            if now_down && !was_down {
                self.pad_down[pad] = true;
                events.push(InputEvent::PadPressed {
                    pad: pad as u8,
                    velocity: (pressure as f32 / PAD_PRESSURE_MAX).clamp(0.0, 1.0),
                });
            } else if !now_down && was_down {
                self.pad_down[pad] = false;
                events.push(InputEvent::PadReleased { pad: pad as u8 });
            } else if now_down {
                events.push(InputEvent::PadAftertouch {
                    pad: pad as u8,
                    pressure: (pressure as f32 / PAD_PRESSURE_MAX).clamp(0.0, 1.0),
                });
            }
        }
        events
    }
}

/// Signed delta for a 4-bit wrapping counter (the main encoder).
fn wrapped_delta_4bit(prev: u8, cur: u8) -> i32 {
    let mut d = cur as i32 - prev as i32;
    if d > 8 {
        d -= 16;
    } else if d < -8 {
        d += 16;
    }
    d
}

/// Signed delta for a 16-bit wrapping counter (display encoders).
fn wrapped_delta_16bit(prev: u16, cur: u16) -> i32 {
    let d = cur.wrapping_sub(prev) as i16;
    d as i32
}
