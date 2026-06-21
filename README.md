# SoupMashine

A **collaborative, multiplayer DAW** (Ableton/Bitwig‑style) written in Rust — up
to 4 players editing one shared session in real time, with a built‑in audio
engine, an audio‑plugin host (CLAP first), and the Native Instruments **Maschine
MK2** supported as a hardware control surface.

> Status: early but real. The collaborative session core, the audio engine, and
> the MK2 surface work today and are covered by tests. The arrangement GUI and
> full plugin instantiation are the active milestones (see the roadmap).

## What works now

- **Multiplayer session sync** — an authoritative server replicates one
  [`Session`](src/daw/mod.rs) (tracks, clips, MIDI notes, mixer, transport,
  players) to up to 4 clients. Clients submit `Op`s; the server assigns ids,
  orders them, and broadcasts so every peer converges to identical state. Audio
  renders locally on each machine; only the lightweight project document travels
  over the network.
- **Audio engine** — polyphonic sampler + step sequencer with a built‑in
  synthesized kit, dependency‑free WAV I/O.
- **Maschine MK2 control surface** — pads (velocity), LEDs and the protocol layer
  over USB HID (hidraw), usable as an input/feedback surface for the DAW.
- **Plugin host abstraction** — a `PluginHost`/`PluginInstance` trait with a
  working CLAP **discovery+validation** backend; full instantiation is next.

## Quick start

```bash
cargo test            # pure-Rust core, no system deps — 24 tests

# Multiplayer (works in any build):
cargo run -- serve 0.0.0.0:8421          # host a session
cargo run -- join  <server-ip>:8421 me   # join from each player's machine
```

`join` runs a short demo that creates a track + clip + note and prints the
shared session; edits from other players appear live.

### Optional features

| Feature | Adds | Needs |
|---------|------|-------|
| `audio` | real-time output via cpal | ALSA (`libasound2-dev`) |
| `hardware` | Maschine MK2 surface | libudev (`libudev-dev`) |
| `clap-host` | scan/validate CLAP plugins | — (libloading, vendored) |
| `full` | all of the above | ALSA + libudev |

```bash
cargo run --features full -- serve 0.0.0.0:8421
```

On Debian/Ubuntu/Mint, `./setup.sh` installs the toolchain + system libs and a
udev rule for the MK2.

## Architecture

```
src/
  daw/      Session document + collaborative Op model (apply is deterministic)
  net/      Authoritative server + client replica, newline-framed JSON over TCP
  host/     PluginHost/PluginInstance traits; CLAP backend (feature clap-host)
  engine/   Sampler + step sequencer + synth kit + WAV (the local audio renderer)
  protocol/ Maschine MK2 USB protocol (input decode, LED + display encode)
  transport/ MK2 byte transport (mock + hidapi/hidraw backend)
  app/ui    MK2 groovebox/control-surface logic and rendering
  main.rs   `serve` / `join` subcommands + the MK2 surface modes
```

Convergence guarantee: `Session::apply` is a pure function of the ordered op
stream, so replaying the server's broadcast on any peer reproduces identical
state. Creation ops get their ids from the server's `Authority`, avoiding
conflicts between players.

## Roadmap

1. **Arrangement/clip GUI** (egui) — session view, timeline, piano roll, mixer.
2. **Clip playback** — schedule clip MIDI through the engine on the synced clock.
3. **Full CLAP hosting** — drive the CLAP factory/process ABI to actually run
   plugins; then **VST3** (Steinberg SDK FFI) and **VST2** adapters behind the
   same `PluginHost` trait.
4. **Per‑player presence & track ownership/locking** in the UI.
5. **Audio clips & recording**, automation lanes.
6. **MK2 surface mapping** to DAW transport/clips once button decode is finalized.

## Plugin format reality check

CLAP is first because it's the only format with a clean, open, Rust‑hostable ABI.
VST3 hosting requires Steinberg's C++ SDK via unsafe FFI; VST2's SDK can't be
redistributed. All three are planned behind one host trait, but CLAP is the
pragmatic path to "load a plugin and hear it."

## License

MIT
