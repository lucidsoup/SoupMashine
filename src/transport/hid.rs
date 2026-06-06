//! Real USB transport for the Maschine MK2 via libusb (the `rusb` crate).
//!
//! Built only with `--features hardware`. The MK2 is a *composite* USB device:
//! besides the control interface we care about (pads, buttons, encoders, LEDs,
//! displays) it also exposes a USB audio interface and MIDI. So we cannot assume
//! the control endpoints live on interface 0 — we scan the configuration
//! descriptor for the interface(s) that actually own endpoints 0x84 / 0x01 /
//! 0x08 and claim those.
//!
//! The MK2 needs no wake-up handshake: it leaves its boot/splash screen as soon
//! as it receives valid display data on the claimed interface.

use super::Transport;
use crate::protocol::device::*;
use rusb::{Context, Device, DeviceHandle, Direction, UsbContext};
use std::io;
use std::time::Duration;

pub struct HidTransport {
    handle: DeviceHandle<Context>,
    claimed: Vec<u8>,
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

/// Return the `(interface, alt-setting)` pairs whose endpoints include any of
/// our control endpoints (input 0x84, output 0x01, display 0x08).
fn control_interfaces(dev: &Device<Context>) -> io::Result<Vec<(u8, u8)>> {
    let wanted = [EP_INPUT, EP_OUTPUT, EP_DISPLAY];
    let config = dev.active_config_descriptor().map_err(to_io)?;
    let mut found = Vec::new();
    for iface in config.interfaces() {
        for desc in iface.descriptors() {
            let owns = desc
                .endpoint_descriptors()
                .any(|ep| wanted.contains(&ep.address()));
            if owns {
                found.push((desc.interface_number(), desc.setting_number()));
                break; // one matching alt-setting per interface is enough
            }
        }
    }
    Ok(found)
}

impl HidTransport {
    /// Open the first connected Maschine MK2, auto-detecting the control interface.
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
        let handle = dev.open().map_err(|e| {
            to_io(format!(
                "found the MK2 but could not open it ({e}). On Linux this is \
                 usually a permissions issue — install the udev rule (see \
                 deploy/install.sh) or run with sudo."
            ))
        })?;

        // Let libusb steal the interface back from any kernel driver (the audio
        // class driver in particular) when we claim it.
        #[cfg(target_os = "linux")]
        {
            let _ = handle.set_auto_detach_kernel_driver(true);
        }

        let targets: Vec<(u8, u8)> = match forced {
            Some(n) => vec![(n, 0)],
            None => {
                let detected = control_interfaces(&dev)?;
                if detected.is_empty() {
                    // Fall back to the historical default and let the claim fail
                    // loudly if that's wrong.
                    vec![(USB_INTERFACE, 0)]
                } else {
                    detected
                }
            }
        };

        let mut claimed = Vec::new();
        for (iface, alt) in &targets {
            handle
                .claim_interface(*iface)
                .map_err(|e| to_io(format!("could not claim interface {iface}: {e}")))?;
            claimed.push(*iface);
            if *alt != 0 {
                let _ = handle.set_alternate_setting(*iface, *alt);
            }
        }

        Ok(HidTransport { handle, claimed })
    }

    /// Human-readable dump of every Native Instruments USB device and its
    /// interfaces/endpoints, marking the control endpoints and the interface(s)
    /// SoupMashine would claim. Used by `soupmashine --list`.
    pub fn describe() -> io::Result<String> {
        use std::fmt::Write;
        let context = Context::new().map_err(to_io)?;
        let mut out = String::new();
        let wanted = [EP_INPUT, EP_OUTPUT, EP_DISPLAY];
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
                    let _ = writeln!(
                        out,
                        "  interface {} (alt {})  class={:#04x} sub={:#04x} proto={:#04x}",
                        desc.interface_number(),
                        desc.setting_number(),
                        desc.class_code(),
                        desc.sub_class_code(),
                        desc.protocol_code()
                    );
                    for ep in desc.endpoint_descriptors() {
                        let dir = match ep.direction() {
                            Direction::In => "IN ",
                            Direction::Out => "OUT",
                        };
                        let mark = if wanted.contains(&ep.address()) {
                            "   <== control endpoint"
                        } else {
                            ""
                        };
                        let _ = writeln!(
                            out,
                            "      endpoint {:#04x}  {} {:?}{}",
                            ep.address(),
                            dir,
                            ep.transfer_type(),
                            mark
                        );
                    }
                }
            }
            match control_interfaces(&dev) {
                Ok(t) if !t.is_empty() => {
                    let ifaces: Vec<u8> = t.iter().map(|(i, _)| *i).collect();
                    let _ = writeln!(out, "  -> will claim interface(s): {ifaces:?}");
                }
                _ => {
                    let _ = writeln!(
                        out,
                        "  -> WARNING: no interface owns endpoints 0x84/0x01/0x08; \
                         protocol constants may be wrong for this unit"
                    );
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
        for iface in &self.claimed {
            let _ = self.handle.release_interface(*iface);
        }
    }
}
