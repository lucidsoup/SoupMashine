//! Byte-level transport to the controller.
//!
//! The [`Transport`] trait abstracts the three Maschine MK2 endpoints. The
//! always-available [`MockTransport`] lets the whole stack run and be tested
//! without hardware; the `hardware` feature adds a real libusb backend.

use std::io;

pub mod mock;
pub use mock::MockTransport;

#[cfg(feature = "hardware")]
pub mod hid;
#[cfg(feature = "hardware")]
pub use hid::HidTransport;

/// Low-level access to the controller's three USB endpoints.
pub trait Transport {
    /// Read one input report into `buf`. Returns the number of bytes read, or 0
    /// on timeout. The first byte is the report id.
    fn read_input(&mut self, buf: &mut [u8], timeout_ms: u32) -> io::Result<usize>;

    /// Write one LED output report (report id included as `report[0]`).
    fn write_output(&mut self, report: &[u8]) -> io::Result<()>;

    /// Write one display chunk (9-byte header + 256 bytes) to the display endpoint.
    fn write_display(&mut self, chunk: &[u8]) -> io::Result<()>;
}
