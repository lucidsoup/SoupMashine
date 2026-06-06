use soupmashine::protocol::device::{self, NUM_PADS};
use soupmashine::protocol::{Button, Color, Framebuffer, InputEvent, InputParser, LedState};

#[test]
fn pad_report_decodes_press_with_velocity() {
    let mut parser = InputParser::new();
    // Pad at report position 0 (physical pad 13 => logical 12), pressure 800.
    let pressure: u16 = 800;
    let low = (pressure & 0xFF) as u8;
    let high = ((0u16 << 4) | ((pressure >> 8) & 0x0F)) as u8;
    let report = [device::INPUT_REPORT_PADS, low, high];

    let events = parser.parse(&report);
    assert_eq!(events.len(), 1);
    match events[0] {
        InputEvent::PadPressed { pad, velocity } => {
            assert_eq!(pad as usize, device::report_pos_to_logical(0));
            assert!((velocity - 800.0 / 1024.0).abs() < 1e-4);
        }
        other => panic!("expected PadPressed, got {other:?}"),
    }
}

#[test]
fn pad_below_threshold_is_not_a_press() {
    let mut parser = InputParser::new();
    let pressure: u16 = 10; // below PAD_THRESHOLD (200)
    let report = [
        device::INPUT_REPORT_PADS,
        (pressure & 0xFF) as u8,
        ((pressure >> 8) & 0x0F) as u8,
    ];
    let events = parser.parse(&report);
    assert!(events
        .iter()
        .all(|e| !matches!(e, InputEvent::PadPressed { .. })));
}

#[test]
fn button_report_decodes_edges() {
    let mut parser = InputParser::new();
    let idx = Button::Play.bit_index();
    let mut report = [0u8; 1 + 6];
    report[0] = device::INPUT_REPORT_BUTTONS;
    report[1 + idx / 8] = 1 << (idx % 8);

    let events = parser.parse(&report);
    assert!(events.contains(&InputEvent::Button {
        button: Button::Play,
        pressed: true,
    }));

    // Releasing it (all zero) yields the release edge.
    let release = [device::INPUT_REPORT_BUTTONS, 0, 0, 0, 0, 0, 0];
    let events = parser.parse(&release);
    assert!(events.contains(&InputEvent::Button {
        button: Button::Play,
        pressed: false,
    }));
}

#[test]
fn main_encoder_delta_wraps() {
    let mut parser = InputParser::new();
    let make = |val: u8| [device::INPUT_REPORT_BUTTONS, 0, 0, 0, 0, 0, 0, val];
    // Prime state.
    parser.parse(&make(0x0F));
    // Wrap from 0x0F to 0x00 should read as +1.
    let events = parser.parse(&make(0x00));
    assert!(events.contains(&InputEvent::Encoder { index: 0, delta: 1 }));
}

#[test]
fn pad_led_report_has_correct_layout() {
    let mut leds = LedState::new();
    let logical = 12usize;
    leds.pads[logical] = Color::RED;
    let reports = leds.encode();

    assert_eq!(reports.pads[0], device::PAD_LED_REPORT_ID);
    assert_eq!(reports.pads.len(), 1 + device::PAD_LED_DATA_LEN);

    let pos = device::logical_to_report_pos(logical);
    let off = 1 + 1 + pos * 3;
    assert_eq!(reports.pads[off], 255);
    assert_eq!(reports.pads[off + 1], 0);
    assert_eq!(reports.pads[off + 2], 0);
}

#[test]
fn pad_logical_mapping_is_a_bijection() {
    let mut seen = [false; NUM_PADS];
    for pos in 0..NUM_PADS {
        let logical = device::report_pos_to_logical(pos);
        assert!(!seen[logical], "logical pad {logical} mapped twice");
        seen[logical] = true;
        assert_eq!(device::logical_to_report_pos(logical), pos);
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn display_encodes_eight_chunks() {
    let mut fb = Framebuffer::new();
    fb.set_pixel(0, 0, true);
    fb.set_pixel(5, 9, true); // page 1

    let chunks = fb.encode_chunks(1);
    assert_eq!(chunks.len(), device::DISPLAY_CHUNKS);
    for chunk in &chunks {
        assert_eq!(chunk.len(), 9 + device::DISPLAY_CHUNK_BYTES);
        assert_eq!(chunk[0], 0xE0 | 1);
    }
    // Pixel (0,0) -> page 0, column 0, bit 0.
    assert_eq!(chunks[0][9] & 1, 1);
    // Pixel (5,9) -> page 1, column 5, bit 1.
    assert_eq!(chunks[1][9 + 5] & (1 << 1), 1 << 1);
}

#[test]
fn framebuffer_text_sets_pixels() {
    let mut fb = Framebuffer::new();
    fb.draw_text("A1", 0, 0, 1);
    // The glyph 'A' has a set pixel somewhere in its top-left 5x7 box.
    let any = (0..7).any(|y| (0..5).any(|x| fb.get_pixel(x, y)));
    assert!(any, "expected text rendering to set some pixels");
}
