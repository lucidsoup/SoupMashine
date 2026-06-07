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
use crate::protocol::LedState;
use rusb::{Context, Device, DeviceHandle, Direction, TransferType, UsbContext};
use std::io;
use std::time::{Duration, Instant};

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

/// Issue HID SET_IDLE(0) and SET_PROTOCOL(report) on an interface. This is what
/// the kernel HID driver does on enumeration; the MK2 needs it before it will
/// emit button/encoder reports. Errors are ignored (best effort).
fn hid_set_idle<T: UsbContext>(handle: &DeviceHandle<T>, interface: u8) {
    // bmRequestType = class | interface | host-to-device = 0x21
    // SET_IDLE (0x0A): wValue = (duration<<8) | reportId = 0 (report on change)
    let _ = handle.write_control(
        0x21,
        0x0A,
        0x0000,
        interface as u16,
        &[],
        Duration::from_millis(50),
    );
    // SET_PROTOCOL (0x0B): wValue = 1 (report protocol)
    let _ = handle.write_control(
        0x21,
        0x0B,
        0x0001,
        interface as u16,
        &[],
        Duration::from_millis(50),
    );
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

        // The kernel HID driver normally issues these; since we bypass it via
        // libusb we must do it ourselves so the device sends button/encoder
        // reports (not just the always-on pad stream).
        hid_set_idle(&handle, ctrl.interface);

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

            let num_configs = dd.num_configurations();
            let active = dev.active_config_descriptor().ok().map(|c| c.number());
            let _ = writeln!(
                out,
                "  {num_configs} configuration(s); active = {}",
                active.map(|n| n.to_string()).unwrap_or_else(|| "?".into())
            );

            for ci in 0..num_configs {
                let config = match dev.config_descriptor(ci) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = writeln!(out, "  config #{ci}: (unreadable: {e})");
                        continue;
                    }
                };
                let is_active = Some(config.number()) == active;
                let _ = writeln!(
                    out,
                    "  configuration {}{}:",
                    config.number(),
                    if is_active { " (ACTIVE)" } else { "" }
                );
                for iface in config.interfaces() {
                    for desc in iface.descriptors() {
                        let kind = match desc.class_code() {
                            0x01 => "audio",
                            0x03 => "HID",
                            0xfe => "DFU/vendor",
                            0xff => "vendor-specific",
                            _ => "other",
                        };
                        let _ = writeln!(
                            out,
                            "    interface {} (alt {})  class={:#04x} [{}]",
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
                                "        endpoint {:#04x}  {} {:?}",
                                ep.address(),
                                dir,
                                ep.transfer_type()
                            );
                        }
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

    /// Poll *every* IN endpoint on the device and print each report in hex,
    /// labeled with the interface and endpoint it came from. Consecutive
    /// identical reports per endpoint are collapsed so held pads don't flood.
    /// Runs until Ctrl-C. Used to discover where buttons/encoders are reported.
    pub fn monitor_raw() -> io::Result<()> {
        let context = Context::new().map_err(to_io)?;
        let dev = find_device(&context)?;
        let config = dev.active_config_descriptor().map_err(to_io)?;

        // (interface, endpoint address, is_interrupt)
        let mut ins: Vec<(u8, u8, bool)> = Vec::new();
        for iface in config.interfaces() {
            for desc in iface.descriptors() {
                for ep in desc.endpoint_descriptors() {
                    if ep.direction() == Direction::In {
                        ins.push((
                            desc.interface_number(),
                            ep.address(),
                            ep.transfer_type() == TransferType::Interrupt,
                        ));
                    }
                }
            }
        }
        if ins.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "device has no IN endpoints",
            ));
        }

        let handle = dev.open().map_err(to_io)?;
        #[cfg(target_os = "linux")]
        {
            let _ = handle.set_auto_detach_kernel_driver(true);
        }

        println!("Claiming interfaces:");
        let mut claimed: Vec<u8> = Vec::new();
        let mut ifaces: Vec<u8> = ins.iter().map(|(i, _, _)| *i).collect();
        ifaces.sort_unstable();
        ifaces.dedup();
        for iface in &ifaces {
            match handle.claim_interface(*iface) {
                Ok(()) => {
                    println!("  interface {iface}: claimed OK");
                    hid_set_idle(&handle, *iface);
                    claimed.push(*iface);
                }
                Err(e) => println!("  interface {iface}: FAILED ({e})"),
            }
        }

        println!("\nListening on IN endpoints:");
        for (iface, ep, is_int) in &ins {
            let kind = if *is_int { "interrupt" } else { "bulk" };
            println!("  interface {iface}, endpoint {ep:#04x} ({kind})");
        }

        // Keep the device in active mode by sending blank LED reports, like the
        // real app does. Discover the OUT endpoint for that.
        let ctrl = select_control(&dev, None).ok();
        let blank = LedState::new().encode();

        println!("\nCALIBRATING for 3 seconds — do NOT touch the controller...");

        let mut buf = [0u8; 512];
        let mut last: Vec<(u8, u8, Vec<u8>)> = Vec::new();
        let mut errored: Vec<(u8, u8)> = Vec::new();
        let timeout = Duration::from_millis(5);

        // Pad-report (0x20) change detection: learn which byte indices flicker
        // on their own, then report only *new* changes (i.e. button presses).
        let mut baseline: Option<Vec<u8>> = None;
        let mut noisy = [false; 32];
        let start = Instant::now();
        let calib = Duration::from_secs(3);
        let mut calibrated = false;
        let mut last_out = Instant::now() - Duration::from_secs(1);

        loop {
            // Keepalive output so the controller reports fully.
            if let Some(c) = &ctrl {
                if last_out.elapsed() >= Duration::from_millis(16) {
                    last_out = Instant::now();
                    for rep in [&blank.pads, &blank.groups, &blank.buttons] {
                        let _ = if c.ep_out_interrupt {
                            handle.write_interrupt(c.ep_out, rep, Duration::from_millis(20))
                        } else {
                            handle.write_bulk(c.ep_out, rep, Duration::from_millis(20))
                        };
                    }
                }
            }
            if !calibrated && start.elapsed() >= calib {
                calibrated = true;
                println!("READY — press buttons/encoders now (Ctrl-C to stop).\n");
            }

            for (iface, ep, is_int) in &ins {
                let res = if *is_int {
                    handle.read_interrupt(*ep, &mut buf, timeout)
                } else {
                    handle.read_bulk(*ep, &mut buf, timeout)
                };
                let n = match res {
                    Ok(n) if n > 0 => n,
                    Ok(_) | Err(rusb::Error::Timeout) => continue,
                    Err(e) => {
                        if !errored.contains(&(*iface, *ep)) {
                            errored.push((*iface, *ep));
                            println!("iface {iface} ep {ep:#04x}: read error ({e})");
                        }
                        continue;
                    }
                };
                let report = &buf[..n];

                // Ignore lone framing bytes.
                if n == 1 {
                    continue;
                }

                // Pad report: diff against the calibrated baseline.
                if report.first() == Some(&0x20) {
                    let frame = &report[..n.min(32)];
                    match &baseline {
                        None => baseline = Some(frame.to_vec()),
                        Some(base) => {
                            let len = frame.len().min(base.len());
                            if !calibrated {
                                for i in 0..len {
                                    if frame[i] != base[i] {
                                        noisy[i] = true;
                                    }
                                }
                            } else {
                                let changes: Vec<String> = (0..len)
                                    .filter(|&i| frame[i] != base[i] && !noisy[i])
                                    .map(|i| format!("[{i}]={:#04x}", frame[i]))
                                    .collect();
                                if !changes.is_empty() {
                                    println!("PAD-REPORT changed: {}", changes.join(" "));
                                }
                            }
                        }
                    }
                    continue;
                }

                // Any other report: collapse repeats, then print raw.
                let slot = last.iter_mut().find(|(i, e, _)| i == iface && e == ep);
                match slot {
                    Some((_, _, prev)) if prev.as_slice() == report => continue,
                    Some((_, _, prev)) => *prev = report.to_vec(),
                    None => last.push((*iface, *ep, report.to_vec())),
                }
                let hex: Vec<String> = report.iter().map(|b| format!("{b:02x}")).collect();
                println!("iface {iface} ep {ep:#04x} [{n:2}]  {}", hex.join(" "));
            }
        }
    }
}

impl Drop for HidTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(self.ctrl.interface);
    }
}
