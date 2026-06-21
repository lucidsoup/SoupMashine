//! Audio plugin hosting abstraction.
//!
//! The DAW talks to plugins only through [`PluginHost`] / [`PluginInstance`], so
//! the concrete format (CLAP first, VST3/VST2 later) is swappable. The default
//! [`NullHost`] lets the whole app build and run with no plugin SDKs. Real CLAP
//! loading lives behind the `clap-host` feature.

/// Metadata about an available plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginDescriptor {
    /// Stable identifier used in the project document (e.g. a file path or id).
    pub uri: String,
    pub name: String,
    pub format: PluginFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFormat {
    Clap,
    Vst3,
    Vst2,
}

/// A MIDI event handed to a plugin for one process block.
#[derive(Debug, Clone, Copy)]
pub struct MidiEvent {
    pub frame: u32,
    pub data: [u8; 3],
}

/// A loaded, instantiated plugin ready to process audio.
pub trait PluginInstance: Send {
    /// Process one block in place. `io` is interleaved stereo; `midi` is sorted
    /// by frame offset within the block.
    fn process(&mut self, io: &mut [f32], midi: &[MidiEvent]);
    /// Set a parameter by index in [0.0, 1.0].
    fn set_param(&mut self, _index: u32, _value: f32) {}
}

/// A plugin backend able to scan for and instantiate plugins.
pub trait PluginHost {
    /// Discover available plugins.
    fn scan(&mut self) -> Vec<PluginDescriptor>;
    /// Instantiate a plugin by its `uri` at the given sample rate.
    fn instantiate(
        &mut self,
        uri: &str,
        sample_rate: u32,
    ) -> Result<Box<dyn PluginInstance>, String>;
}

/// A host that finds nothing and loads nothing. Always available.
#[derive(Default)]
pub struct NullHost;

impl PluginHost for NullHost {
    fn scan(&mut self) -> Vec<PluginDescriptor> {
        Vec::new()
    }
    fn instantiate(&mut self, uri: &str, _sr: u32) -> Result<Box<dyn PluginInstance>, String> {
        Err(format!("no plugin host available to load '{uri}'"))
    }
}

/// Standard CLAP search locations on Linux.
pub fn default_clap_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(std::path::Path::new(&home).join(".clap"));
    }
    paths.push("/usr/lib/clap".into());
    paths.push("/usr/local/lib/clap".into());
    paths
}

#[cfg(feature = "clap-host")]
pub mod clap;
