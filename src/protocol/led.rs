//! LED state and encoding into the three Maschine MK2 output reports.

use super::color::Color;
use super::device::*;

/// Full LED state of the controller. Build it each frame, then `encode()` it
/// into the report buffers that get written to the output endpoint.
#[derive(Debug, Clone)]
pub struct LedState {
    /// Logical pad colors (index 0 = bottom-left pad).
    pub pads: [Color; NUM_PADS],
    /// Group button colors (A..H).
    pub groups: [Color; NUM_GROUPS],
    /// Monochrome button brightness, indexed by `Button::bit_index()`.
    pub buttons: [u8; NUM_BUTTONS],
}

impl Default for LedState {
    fn default() -> Self {
        LedState {
            pads: [Color::BLACK; NUM_PADS],
            groups: [Color::BLACK; NUM_GROUPS],
            buttons: [0; NUM_BUTTONS],
        }
    }
}

/// The three report buffers (report id prefixed) for one LED update.
pub struct LedReports {
    pub pads: Vec<u8>,
    pub groups: Vec<u8>,
    pub buttons: Vec<u8>,
}

impl LedState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Encode into the three output reports.
    pub fn encode(&self) -> LedReports {
        LedReports {
            pads: self.encode_pads(),
            groups: self.encode_groups(),
            buttons: self.encode_buttons(),
        }
    }

    fn encode_pads(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 1 + PAD_LED_DATA_LEN];
        buf[0] = PAD_LED_REPORT_ID;
        // RGB triplets follow immediately after the report id, in report order.
        for logical in 0..NUM_PADS {
            let pos = logical_to_report_pos(logical);
            let off = 1 + pos * 3;
            let c = self.pads[logical];
            buf[off] = c.r;
            buf[off + 1] = c.g;
            buf[off + 2] = c.b;
        }
        buf
    }

    fn encode_groups(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 1 + GROUP_LED_DATA_LEN];
        buf[0] = GROUP_LED_REPORT_ID;
        // Group RGB LEDs occupy the first 24 data bytes. (VERIFY offsets.)
        for g in 0..NUM_GROUPS {
            let off = 1 + g * 3;
            let c = self.groups[g];
            buf[off] = c.r;
            buf[off + 1] = c.g;
            buf[off + 2] = c.b;
        }
        buf
    }

    fn encode_buttons(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 1 + BUTTON_LED_DATA_LEN];
        buf[0] = BUTTON_LED_REPORT_ID;
        // Only the first BUTTON_LED_DATA_LEN button LEDs are addressable here.
        let n = BUTTON_LED_DATA_LEN.min(NUM_BUTTONS);
        buf[1..1 + n].copy_from_slice(&self.buttons[..n]);
        buf
    }
}
