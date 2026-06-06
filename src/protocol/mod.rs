//! Pure, dependency-free implementation of the Maschine MK2 USB protocol:
//! device constants, input report decoding, and LED / display encoding.
//!
//! Nothing in this module touches USB, the OS, or audio, so it builds and tests
//! anywhere. The actual byte-level transport lives in `crate::transport`.

pub mod button;
pub mod color;
pub mod device;
pub mod display;
pub mod font;
pub mod input;
pub mod led;

pub use button::Button;
pub use color::Color;
pub use display::Framebuffer;
pub use input::{InputEvent, InputParser};
pub use led::{LedReports, LedState};
