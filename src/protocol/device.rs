//! Hardware constants for the Native Instruments Maschine MK2.
//!
//! These values are taken from the open-source `cabl` controller library
//! (shaduzlabs/cabl) and corroborated by community reverse-engineering notes.
//! The ones marked CONFIRMED are well established; the ones marked VERIFY are
//! plausible but should be checked against a physical unit.

/// USB vendor id for Native Instruments. (CONFIRMED)
pub const VENDOR_ID: u16 = 0x17CC;
/// USB product id for the Maschine MK2. (CONFIRMED)
pub const PRODUCT_ID: u16 = 0x1140;

/// Interrupt IN endpoint: button / encoder / pad reports. (CONFIRMED)
pub const EP_INPUT: u8 = 0x84;
/// Interrupt OUT endpoint: LED reports. (CONFIRMED)
pub const EP_OUTPUT: u8 = 0x01;
/// Bulk OUT endpoint: display frame data. (CONFIRMED)
pub const EP_DISPLAY: u8 = 0x08;

/// USB interface number to claim.
pub const USB_INTERFACE: u8 = 0;

pub const NUM_PADS: usize = 16;
pub const NUM_GROUPS: usize = 8;
pub const NUM_BUTTONS: usize = 48;
/// Main encoder (index 0) plus the eight display encoders (1..=8).
pub const NUM_ENCODERS: usize = 9;

/// 12-bit pad pressure threshold above which a pad counts as "pressed". (CONFIRMED)
pub const PAD_THRESHOLD: u16 = 200;
/// Full-scale pad pressure value used to normalize velocity to [0.0, 1.0].
pub const PAD_PRESSURE_MAX: f32 = 1024.0;

// ---- Displays ------------------------------------------------------------

pub const NUM_DISPLAYS: usize = 2;
pub const DISPLAY_WIDTH: usize = 256;
pub const DISPLAY_HEIGHT: usize = 64;
/// 8 vertical pages of 8 pixels each.
pub const DISPLAY_PAGES: usize = DISPLAY_HEIGHT / 8;
/// One byte per column per page.
pub const DISPLAY_BYTES: usize = DISPLAY_WIDTH * DISPLAY_PAGES; // 2048
pub const DISPLAY_CHUNKS: usize = DISPLAY_PAGES; // 8 chunks, one page each
pub const DISPLAY_CHUNK_BYTES: usize = DISPLAY_WIDTH; // 256

// ---- LED report sizes (data bytes, excluding the leading report id) ------

// Report sizes confirmed against the open-maschine project: the report id is
// the first byte and the totals below are the DATA bytes that follow it.
/// Report id 0x80 + 48 data bytes (16 pads * 3 RGB) = 49 total. (CONFIRMED)
pub const PAD_LED_REPORT_ID: u8 = 0x80;
pub const PAD_LED_DATA_LEN: usize = 48;
/// Report id 0x81 + 56 data bytes = 57 total. (CONFIRMED size)
pub const GROUP_LED_REPORT_ID: u8 = 0x81;
pub const GROUP_LED_DATA_LEN: usize = 56;
/// Report id 0x82 + 31 data bytes = 32 total. (CONFIRMED size)
pub const BUTTON_LED_REPORT_ID: u8 = 0x82;
pub const BUTTON_LED_DATA_LEN: usize = 31;

// ---- Input report ids ----------------------------------------------------

/// Buttons + encoders report. (CONFIRMED)
pub const INPUT_REPORT_BUTTONS: u8 = 0x01;
/// Pad pressure report. (CONFIRMED)
pub const INPUT_REPORT_PADS: u8 = 0x20;

/// Mapping from a pad's position in the USB report (0..15) to its physical pad
/// number (1..16, where 1 is the bottom-left pad). The MK2 reports the top row
/// first. (CONFIRMED ordering, per cabl.)
pub const PAD_REPORT_TO_PHYSICAL: [u8; NUM_PADS] = [
    13, 14, 15, 16, // top row
    9, 10, 11, 12, //
    5, 6, 7, 8, //
    1, 2, 3, 4, // bottom row
];

/// Convert a report position (0..15) to a logical pad index (0..15) where 0 is
/// the bottom-left pad.
pub fn report_pos_to_logical(pos: usize) -> usize {
    (PAD_REPORT_TO_PHYSICAL[pos] - 1) as usize
}

/// Convert a logical pad index (0 = bottom-left) to its position in the LED /
/// input report.
pub fn logical_to_report_pos(logical: usize) -> usize {
    let physical = (logical + 1) as u8;
    PAD_REPORT_TO_PHYSICAL
        .iter()
        .position(|&p| p == physical)
        .unwrap_or(logical)
}
