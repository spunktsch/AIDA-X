pub mod plugin_info;
pub mod parameters;
pub mod audio;
pub mod state;

#[cfg(feature = "clap")]
pub mod clap_backend;

#[cfg(feature = "vst3")]
pub mod vst3_backend;

pub use plugin_info::PluginInfo;
pub use parameters::PluginParameters;
pub use audio::AidaAudioProcessor;
pub use state::PluginState;

#[cfg(feature = "clap")]
pub use clap_backend;

#[cfg(feature = "vst3")]
pub use vst3_backend;
