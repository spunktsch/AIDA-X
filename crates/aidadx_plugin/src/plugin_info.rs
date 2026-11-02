/// Plugin metadata matching the DPF DistrhoPluginCommon.hpp definitions.

/// Unique plugin label (max 127 chars, used for host identification)
pub const PLUGIN_LABEL: &str = "AIDA_X";

/// Brand / manufacturer name
pub const PLUGIN_BRAND: &str = "AIDA DSP";

/// Unique plugin ID ("aida")
pub const PLUGIN_UNIQUE_ID: &str = "aida";

/// Plugin URI (used by CLAP, LV2)
pub const PLUGIN_URI: &str =
    "http://aidadsp.cc/plugins/aidadsp-bundle/rt-neural-loader";

/// Plugin version string
pub const PLUGIN_VERSION: &str = "1.1.0";

/// Plugin description
pub const PLUGIN_DESCRIPTION: &str =
    "Simple loader for neural models using RTNeural inference engine.";

/// Plugin author / homepage
pub const PLUGIN_HOMEPAGE: &str = "https://aidadsp.github.io/";

/// Plugin license
pub const PLUGIN_LICENSE: &str = "GPL-3.0-or-later";

/// CLAP vendor name
pub const CLAP_VENDOR: &str = "AIDA DSP";

/// CLAP features for audio-effect plugins
pub const CLAP_FEATURES: &[&str] = &["audio-effect", "multi-effects", "mono"];

/// VST3 categories (pipe-separated)
pub const VST3_CATEGORIES: &str = "Fx|Dynamics|Mono";

/// Default model filename
pub const DEFAULT_MODEL: &str = "tw40_california_clean.json";

/// Default cabinet IR filename
pub const DEFAULT_CABINET: &str = "V30-P2-audix-i5.wav";

/// Plugin sample rate
pub const PLUGIN_SAMPLE_RATE: f64 = 48_000.0;

/// Plugin buffer size
pub const PLUGIN_BUFFER_SIZE: u32 = 512;
