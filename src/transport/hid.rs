//! Real USB transport for the Maschine MK2 via libusb (the `rusb` crate).
//!
//! Built only with `--features hardware`. The MK2 is a *composite* USB device:
//!   interface 0  USB audio control
//!   interface 1  USB-MIDI (bulk)
//!   interface 2  HID  <- pads, buttons, encoders, LEDs and displays
//!   interface 3  DFU
//!
//! We must talk to the **HID** interface (class 0x03), and the exact endpoint
//! addresses vary, so we discover them from the configuration descriptor rather
//! than hardcoding. All output (LED reports 0x80/0x81/0x82 and display frames
//! 0xE0/0xE1) goes out the single HID interrupt OUT endpoint; all input comes in
//! on the HID interrupt IN endpoint. No wake-up handshake is required.

use super::Transport;
use crate::protocol::device::*;
use rusb::{Context, Device, DeviceHandle, Direction, TransferType, UsbContext};
use std::io;
use std::time::Duration;

/// A control interface with its discovered endpoints.
#[derive(Clone, Copy, Debug)]
struct Control {
    interface: u8,
    alt: u8,
    ep_in: u8,
    ep_in_interrupt: bool,
    ep_out: u8,
    ep_out_interrupt: bool,
}

pub struct HidTransport {
    handle: DeviceHandle<Context>,
    ctrl: Control,
}

fn to_io<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

fn find_device(context: &Context) -> io::Result<Device<Context>> {
    for dev in context.devices().map_err(to_io)?.iter() {
        if let Ok(dd) = dev.device_descriptor() {
            if dd.vendor_id() == VENDOR_ID && dd.product_id() == PRODUCT_ID {
                return Ok(dev);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Maschine MK2 (17cc:1140) not found on USB",
    ))
}

/// Choose the control interface and its IN/OUT endpoints. Prefers the HID
/// interface (class 0x03); falls back to any interface that has both an IN and
/// an OUT endpoint. If `forced` is set, only that interface number is considered.
fn select_control(dev: &Device<Context>, forced: Option<u8>) -> io::Result<Control> {
    let config = dev.active_config_descriptor().map_err(to_io)?;
    let mut hid: Option<Control> = None;
    let mut fallback: Option<Control> = None;

    for iface in config.interfaces() {
        for desc in iface.descriptors() {
            if let Some(n) = forced {
                if desc.interface_number() != n {
                    continue;
                }
            }
            // First IN and first OUT endpoint (preferring interrupt).
            let mut ep_in: Option<(u8, bool)> = None;
            let mut ep_out: Option<(u8, bool)> = None;
            for ep in desc.endpoint_descriptors() {
                let is_int = ep.transfer_type() == TransferType::Interrupt;
                let better = |cur: &Option<(u8, bool)>| match cur {
                    None => true,
                    Some((_, was_int)) => !*was_int && is_int,
                };
                match ep.direction() {
                    Direction::In if better(&ep_in) => ep_in = Some((ep.address(), is_int)),
                    Direction::Out if better(&ep_out) => ep_out = Some((ep.address(), is_int)),
                    _ => {}
                }
            }
            if let (Some((i, ii)), Some((o, oi))) = (ep_in, ep_out) {
                let c = Control {
                    interface: desc.interface_number(),
                    alt: desc.setting_number(),
                    ep_in: i,
                    ep_in_interrupt: ii,
                    ep_out: o,
                    ep_out_interrupt: oi,
                };
                if desc.class_code() == 0x03 && hid.is_none() {
                    hid = Some(c);
                } else if fallback.is_none() {
                    fallback = Some(c);
                }
            }
        }
    }

    hid.or(fallback).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no HID/control interface with IN+OUT endpoints found",
        )
    })
}

impl HidTransport {
    /// Open the first connected Maschine MK2, auto-detecting the HID interface.
    pub fn open() -> io::Result<Self> {
        Self::open_inner(None)
    }

    /// Open and claim a specific interface number (override for debugging).
    pub fn open_interface(interface: u8) -> io::Result<Self> {
        Self::open_inner(Some(interface))
    }

    fn open_inner(forced: Option<u8>) -> io::Result<Self> {
        let context = Context::new().map_err(to_io)?;
        let dev = find_device(&context)?;
        let ctrl = select_control(&dev, forced)?;

        let handle = dev.open().map_err(|e| {
            to_io(format!(
                "found the MK2 but could not open it ({e}). On Linux this is \
                 usually a permissions issue — install the udev rule (run \
                 ./setup.sh) or use sudo."
            ))
        })?;

        // Steal the interface back from the kernel HID driver when we claim it.
        #[cfg(target_os = "linux")]
        {
            let _ = handle.set_auto_detach_kernel_driver(true);
        }

        handle
            .claim_interface(ctrl.interface)
            .map_err(|e| to_io(format!("could not claim interface {}: {e}", ctrl.interface)))?;
        if ctrl.alt != 0 {
            let _ = handle.set_alternate_setting(ctrl.interface, ctrl.alt);
        }

        Ok(HidTransport { handle, ctrl })
    }

    /// Human-readable dump of every NI USB device and its interfaces/endpoints,
    /// marking the interface SoupMashine would drive. Used by `--list`.
    pub fn describe() -> io::Result<String> {
        use std::fmt::Write;
        let context = Context::new().map_err(to_io)?;
        let mut out = String::new();
        let mut any = false;

        for dev in context.devices().map_err(to_io)?.iter() {
            let dd = match dev.device_descriptor() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if dd.vendor_id() != VENDOR_ID {
                continue;
            }
            any = true;
            let _ = writeln!(
                out,
                "Native Instruments device {:04x}:{:04x}  (bus {}, addr {})",
                dd.vendor_id(),
                dd.product_id(),
                dev.bus_number(),
                dev.address()
            );
            if dd.product_id() != PRODUCT_ID {
                let _ = writeln!(out, "  note: not the Maschine MK2 PID (1140)");
            }

            let config = match dev.active_config_descriptor() {
                Ok(c) => c,
                Err(e) => {
                    let _ = writeln!(out, "  (no active configuration: {e})");
                    continue;
                }
            };
            for iface in config.interfaces() {
                for desc in iface.descriptors() {
                    let kind = match desc.class_code() {
                        0x01 => "audio",
                        0x03 => "HID",
                        0xfe => "DFU/vendor",
                        _ => "other",
                    };
                    let _ = writeln!(
                        out,
                        "  interface {} (alt {})  class={:#04x} [{}]",
                        desc.interface_number(),
                        desc.setting_number(),
                        desc.class_code(),
                        kind
                    );
                    for ep in desc.endpoint_descriptors() {
                        let dir = match ep.direction() {
                            Direction::In => "IN ",
                            Direction::Out => "OUT",
                        };
                        let _ = writeln!(
                            out,
                            "      endpoint {:#04x}  {} {:?}",
                            ep.address(),
                            dir,
                            ep.transfer_type()
                        );
                    }
                }
            }
            match select_control(&dev, None) {
                Ok(c) => {
                    let _ = writeln!(
                        out,
                        "  -> will drive interface {} (IN {:#04x}, OUT {:#04x})",
                        c.interface, c.ep_in, c.ep_out
                    );
                }
                Err(e) => {
                    let _ = writeln!(out, "  -> WARNING: {e}");
                }
            }
        }

        if !any {
            out.push_str("No Native Instruments (vendor 17cc) USB devices found.\n");
        }
        Ok(out)
    }
}

impl Transport for HidTransport {
    fn read_input(&mut self, buf: &mut [u8], timeout_ms: u32) -> io::Result<usize> {
        let timeout = Duration::from_millis(timeout_ms as u64);
        let res = if self.ctrl.ep_in_interrupt {
            self.handle.read_interrupt(self.ctrl.ep_in, buf, timeout)
        } else {
            self.handle.read_bulk(self.ctrl.ep_in, buf, timeout)
        };
        match res {
            Ok(n) => Ok(n),
            Err(rusb::Error::Timeout) => Ok(0),
            Err(e) => Err(to_io(e)),
        }
    }

    fn write_output(&mut self, report: &[u8]) -> io::Result<()> {
        self.write_out(report)
    }

    fn write_display(&mut self, chunk: &[u8]) -> io::Result<()> {
        self.write_out(chunk)
    }
}

impl HidTransport {
    fn write_out(&mut self, data: &[u8]) -> io::Result<()> {
        let timeout = Duration::from_millis(100);
        let res = if self.ctrl.ep_out_interrupt {
            self.handle.write_interrupt(self.ctrl.ep_out, data, timeout)
        } else {
            self.handle.write_bulk(self.ctrl.ep_out, data, timeout)
        };
        res.map(|_| ()).map_err(to_io)
    }
}

impl Drop for HidTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.ctrl.interface);
    }
}
