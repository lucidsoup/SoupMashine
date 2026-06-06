# SoupMashine

Standalone groovebox firmware for the **Native Instruments Maschine MK2**.

Plug a Maschine MK2 into a small Linux host (a Raspberry Pi, say), run this, and
the controller becomes a self-contained sampler + step sequencer — no NI Maschine
software, no host computer DAW. Inspired by the
[Maschinio project](https://cdm.link/tag/maschinio/).

The pads play sounds, the 16-step sequencer drives a built-in synthesized drum
kit, and both 256×64 displays show the transport, tempo, and step grid.

## Why it builds everywhere

The codebase is layered so the **core is pure Rust with zero system
dependencies** — it compiles and its full test suite runs on any machine
(including CI and a freshly-imaged Pi). The two pieces that need OS libraries are
opt-in Cargo features:

| Build | Command | Needs |
|-------|---------|-------|
| Core + headless demo | `cargo run` | nothing — renders a demo beat to `soupmashine-demo.wav` |
| Live audio (no controller) | `cargo run --features audio` | ALSA (`libasound2-dev`) |
| Controller, no sound (LED/display diag) | `cargo run --features hardware` | libusb (vendored) |
| **Full standalone groovebox** | `cargo run --release --features full` | ALSA + libusb |

```
cargo test          # 17 tests, no hardware or audio libs required
```

## Architecture

```
src/
  protocol/    Pure MK2 USB protocol: input decode, LED + display encode
    device.rs    VID/PID, endpoints, geometry, pad mapping (constants)
    input.rs     raw report bytes  -> InputEvent (pads, buttons, encoders)
    led.rs       LedState          -> the three output reports (0x80/0x81/0x82)
    display.rs   256x64 framebuffer -> 8 display chunks + drawing primitives
    font.rs      tiny 5x7 bitmap font for the UI
  transport/   Byte-level device access
    mock.rs      in-memory transport (used by tests + headless)
    hid.rs       real libusb backend          [feature = "hardware"]
  engine/      Audio engine
    sampler.rs   32-voice polyphonic one-shot sampler
    sequencer.rs 16-track step sequencer, 16th-note grid
    synth.rs     procedurally generated drum/synth kit (no sample files needed)
    sample.rs    mono samples + a dependency-free WAV reader/writer
  audio/       cpal output stream                [feature = "audio"]
  app.rs       turns InputEvents into engine actions (the groovebox logic)
  ui.rs        renders app state to pad LEDs + both displays
  main.rs      wires it all together per feature set
```

Data flow on real hardware:

```
MK2 --usb--> transport.read_input --> InputParser --> App.handle_event --> Engine
                                                                            |
cpal audio callback <------------------------- Engine.render ---------------+
MK2 <--usb-- transport.write_* <-- ui::build_leds / build_displays <-- App + Engine
```

## Controls

- **Pads** — play the 16-sound kit (kick, snare, hats, clap, toms, cowbell,
  tonal hits). The last pad pressed becomes the *active track*.
- **PLAY** — start/stop the sequencer.
- **REC** — arm recording; pad hits while playing are written to the nearest step.
- **GRID** (or PAD MODE) — toggle **Step mode**: pads 1–16 toggle the active
  track's steps instead of playing. Hold **SHIFT** + pad to pick the track.
- **ERASE** — clear the active track (SHIFT+ERASE clears the whole pattern).
- **STEP ◀ / ▶** — page through steps when a pattern is longer than 16.
- **Main encoder** — tempo. **Encoder 1** — master volume.

Left display: tempo, transport, mode, active track, current step.
Right display: the active track's step grid with a moving playhead.

## Running on a Raspberry Pi

```bash
sudo apt install build-essential libasound2-dev libudev-dev pkg-config
curl https://sh.rustup.rs -sSf | sh        # if Rust isn't installed
cargo build --release --features full

# Let non-root users talk to the device (VID 17cc):
echo 'SUBSYSTEM=="usb", ATTR{idVendor}=="17cc", MODE="0666"' \
  | sudo tee /etc/udev/rules.d/50-maschine.rules
sudo udevadm control --reload-rules && sudo udevadm trigger

./target/release/soupmashine          # plug in the MK2 first
```

To launch on boot, point a small systemd unit at the release binary.

## Protocol reference

Confirmed against the open-source [`cabl`](https://github.com/shaduzlabs/cabl)
library and community reverse-engineering
([writeup](https://lerner98.medium.com/rage-against-the-maschine-3357be1abc48)):

- USB **VID `0x17CC`**, **PID `0x1140`**, HID/USB class.
- Endpoints: input `0x84` (interrupt IN), output `0x01` (interrupt OUT),
  display `0x08` (bulk OUT).
- Input report `0x01`: 48 buttons packed 1-bit-each into 6 bytes, then the main
  encoder nibble, then eight 16-bit display-encoder values.
- Input report `0x20`: pads as 2-byte pairs — 12-bit pressure + 4-bit pad index;
  press threshold 200, full scale ~1024.
- LED reports: `0x80` pads (49 B), `0x81` groups (57 B), `0x82` buttons (32 B).
- Displays: two 256×64 monochrome panels, each sent as 8 chunks of 256 bytes,
  page-major, with header `{0xE0|idx, 0,0, chunk, 0,0x20,0,0x08,0}`.

Marked `VERIFY` in the source (correct against hardware if needed — they are all
centralized in `protocol/`): the exact button bit positions, the button/group
LED byte offsets, and the display chunk-header `chunk` field semantics. Pad
input/LED layout and display geometry are confirmed.

## License

MIT
