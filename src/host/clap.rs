//! CLAP plugin host (feature `clap-host`).
//!
//! Discovery and validation are implemented: we scan the standard CLAP
//! directories and confirm each candidate exports the `clap_entry` symbol.
//! Full audio instantiation (driving the CLAP factory/process ABI) is the next
//! milestone; until then `instantiate` returns a clear error so the rest of the
//! DAW keeps working.

use super::{default_clap_paths, PluginDescriptor, PluginFormat, PluginHost, PluginInstance};
use std::fs;

#[derive(Default)]
pub struct ClapHost;

impl ClapHost {
    pub fn new() -> Self {
        ClapHost
    }
}

fn is_clap(path: &std::path::Path) -> bool {
    path.extension().map(|e| e == "clap").unwrap_or(false)
}

impl PluginHost for ClapHost {
    fn scan(&mut self) -> Vec<PluginDescriptor> {
        let mut found = Vec::new();
        for dir in default_clap_paths() {
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !is_clap(&path) {
                    continue;
                }
                // Validate it's a real CLAP binary by checking the entry symbol.
                let valid = unsafe {
                    libloading::Library::new(&path)
                        .ok()
                        .and_then(|lib| {
                            lib.get::<*const std::ffi::c_void>(b"clap_entry\0")
                                .ok()
                                .map(|_| ())
                        })
                        .is_some()
                };
                if valid {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("plugin")
                        .to_string();
                    found.push(PluginDescriptor {
                        uri: path.to_string_lossy().into_owned(),
                        name,
                        format: PluginFormat::Clap,
                    });
                }
            }
        }
        found
    }

    fn instantiate(
        &mut self,
        uri: &str,
        _sample_rate: u32,
    ) -> Result<Box<dyn PluginInstance>, String> {
        // Confirm the binary is loadable so the error is meaningful.
        let _lib = unsafe { libloading::Library::new(uri) }
            .map_err(|e| format!("failed to load CLAP '{uri}': {e}"))?;
        Err(format!(
            "'{uri}' is a valid CLAP plugin; full instantiation (factory + \
             process) is the next milestone and is not wired up yet"
        ))
    }
}
