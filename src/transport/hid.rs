//! Real USB transport for the Maschine MK2 via libusb (the `rusb` crate).
//!
//! Built only with `--features hardware`. The MK2 is addressed as a raw USB
//! device (not through the HID class) because the display lives on its own bulk
//! endpoint, so we claim the interface and do interrupt / bulk transfers
//! directly.

use super::Transport;
use crate::protocol::device::*;
use rusb::{Context, DeviceHandle, UsbContext};
use std::io;
use std::time::Duration;

pub struct HidTransport {
    handle: DeviceHandle<Context>,
}

fn to_io<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

impl HidTransport {
    /// Open the first connected Maschine MK2.
    pub fn open() -> io::Result<Self> {
        let context = Context::new().map_err(to_io)?;
        let handle = context
            .open_device_with_vid_pid(VENDOR_ID, PRODUCT_ID)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "Maschine MK2 not found on USB")
            })?;

        // On Linux a kernel driver may already be attached; detach it so we can
        // claim the interface. Ignore platforms where this is unsupported.
        #[cfg(target_os = "linux")]
        {
            let _ = handle.set_auto_detach_kernel_driver(true);
        }

        handle.claim_interface(USB_INTERFACE).map_err(to_io)?;
        Ok(HidTransport { handle })
    }
}

impl Transport for HidTransport {
    fn read_input(&mut self, buf: &mut [u8], timeout_ms: u32) -> io::Result<usize> {
        match self
            .handle
            .read_interrupt(EP_INPUT, buf, Duration::from_millis(timeout_ms as u64))
        {
            Ok(n) => Ok(n),
            Err(rusb::Error::Timeout) => Ok(0),
            Err(e) => Err(to_io(e)),
        }
    }

    fn write_output(&mut self, report: &[u8]) -> io::Result<()> {
        self.handle
            .write_interrupt(EP_OUTPUT, report, Duration::from_millis(100))
            .map(|_| ())
            .map_err(to_io)
    }

    fn write_display(&mut self, chunk: &[u8]) -> io::Result<()> {
        self.handle
            .write_bulk(EP_DISPLAY, chunk, Duration::from_millis(100))
            .map(|_| ())
            .map_err(to_io)
    }
}

impl Drop for HidTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(USB_INTERFACE);
    }
}
