//! Real USB transport for the Maschine MK2 via hidapi (hidraw backend).
//!
//! Built only with `--features hardware`. We deliberately use the hidraw backend
//! (the kernel HID driver) rather than raw libusb: the MK2 only starts emitting
//! its button/encoder report (report id 0x10) once the kernel's HID
//! initialization has run. Pads (0x20) stream regardless. All output (LED
//! reports 0x80/0x81/0x82 and display frames 0xE0/0xE1) is written with the
//! report id as the first byte, exactly as the open-maschine project does.

use super::Transport;
use crate::protocol::device::{PRODUCT_ID, VENDOR_ID};
use crate::protocol::LedState;
use hidapi::{HidApi, HidDevice};
use std::io;

/// The MK2's HID interface number (interfaces 0/1 are audio/MIDI, 3 is DFU).
const HID_INTERFACE: i32 = 2;

pub struct HidTransport {
    _api: HidApi,
    device: HidDevice,
}

fn to_io<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

/// Open the MK2's HID interface (or a forced interface number).
fn open_device(api: &HidApi, forced: Option<i32>) -> io::Result<HidDevice> {
    let want = forced.unwrap_or(HID_INTERFACE);
    // Prefer the requested interface; fall back to any MK2 HID node.
    let path = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VENDOR_ID
                && d.product_id() == PRODUCT_ID
                && d.interface_number() == want
        })
        .or_else(|| {
            api.device_list()
                .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
        })
        .map(|d| d.path().to_owned());

    match path {
        Some(p) => api.open_path(&p).map_err(|e| {
            to_io(format!(
                "found the MK2 but could not open it ({e}). On Linux this is \
                 usually a permissions issue — install the udev rule (run \
                 ./setup.sh) or use sudo."
            ))
        }),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Maschine MK2 (17cc:1140) HID interface not found",
        )),
    }
}

impl HidTransport {
    pub fn open() -> io::Result<Self> {
        Self::open_inner(None)
    }

    pub fn open_interface(interface: u8) -> io::Result<Self> {
        Self::open_inner(Some(interface as i32))
    }

    fn open_inner(forced: Option<i32>) -> io::Result<Self> {
        let api = HidApi::new().map_err(to_io)?;
        let device = open_device(&api, forced)?;
        device.set_blocking_mode(false).map_err(to_io)?;
        Ok(HidTransport { _api: api, device })
    }

    /// List every Native Instruments HID interface present.
    pub fn describe() -> io::Result<String> {
        use std::fmt::Write;
        let api = HidApi::new().map_err(to_io)?;
        let mut out = String::new();
        let mut any = false;
        for d in api.device_list() {
            if d.vendor_id() != VENDOR_ID {
                continue;
            }
            any = true;
            let _ = writeln!(
                out,
                "NI {:04x}:{:04x}  interface {}  product {:?}  path {:?}",
                d.vendor_id(),
                d.product_id(),
                d.interface_number(),
                d.product_string().unwrap_or("?"),
                d.path()
            );
        }
        if !any {
            out.push_str(
                "No Native Instruments (17cc) HID interfaces found.\n\
                 (The audio/MIDI interfaces are not HID and won't appear here.)\n",
            );
        }
        Ok(out)
    }

    /// Print raw input reports as the user presses controls. The pad report
    /// (0x20) is muted so button/encoder reports (0x10) are visible.
    pub fn monitor_raw() -> io::Result<()> {
        use std::time::{Duration, Instant};

        let api = HidApi::new().map_err(to_io)?;
        let device = open_device(&api, None)?;
        device.set_blocking_mode(false).map_err(to_io)?;

        // Wake the device into full reporting by sending a blank LED frame,
        // exactly like open-maschine does.
        let blank = LedState::new().encode();
        let _ = device.write(&blank.buttons);

        println!("Press buttons/encoders (pads are muted). Ctrl-C to stop.\n");

        let mut buf = [0u8; 512];
        let mut last: Vec<u8> = Vec::new();
        let mut last_out = Instant::now();

        loop {
            // Periodic keepalive output keeps the device fully active.
            if last_out.elapsed() >= Duration::from_millis(20) {
                last_out = Instant::now();
                let _ = device.write(&blank.buttons);
            }
            match device.read_timeout(&mut buf, 50) {
                Ok(0) => continue,
                Ok(n) => {
                    let report = &buf[..n];
                    if report.first() == Some(&0x20) {
                        continue; // mute the pad stream
                    }
                    if report == last.as_slice() {
                        continue;
                    }
                    last = report.to_vec();
                    let hex: Vec<String> = report.iter().map(|b| format!("{b:02x}")).collect();
                    println!("[{n:2}] {}", hex.join(" "));
                }
                Err(e) => {
                    eprintln!("read error: {e}");
                    break;
                }
            }
        }
        Ok(())
    }
}

impl Transport for HidTransport {
    fn read_input(&mut self, buf: &mut [u8], timeout_ms: u32) -> io::Result<usize> {
        self.device
            .read_timeout(buf, timeout_ms.min(i32::MAX as u32) as i32)
            .map_err(to_io)
    }

    fn write_output(&mut self, report: &[u8]) -> io::Result<()> {
        self.device.write(report).map(|_| ()).map_err(to_io)
    }

    fn write_display(&mut self, chunk: &[u8]) -> io::Result<()> {
        self.device.write(chunk).map(|_| ()).map_err(to_io)
    }
}
