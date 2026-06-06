//! SoupMashine — standalone groovebox firmware for the Native Instruments
//! Maschine MK2.
//!
//! The crate is split into four layers:
//! - [`protocol`]: pure, dependency-free encoding/decoding of the MK2 USB
//!   protocol (input reports, LED reports, display frames).
//! - [`transport`]: byte-level access to the device (a mock for tests, plus a
//!   real libusb backend behind the `hardware` feature).
//! - [`engine`]: the sampler + step-sequencer audio engine.
//! - [`app`] / [`ui`]: glue that turns hardware events into engine actions and
//!   renders state back to the pads and displays.

pub mod app;
pub mod engine;
pub mod protocol;
pub mod transport;
pub mod ui;

#[cfg(feature = "audio")]
pub mod audio;
