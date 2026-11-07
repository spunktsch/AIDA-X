//! Plugin metadata, from `src/DistrhoPluginCommon.hpp`.

/// `DISTRHO_PLUGIN_NAME`
pub const PLUGIN_NAME: &str = "AIDA-X";
/// `DISTRHO_PLUGIN_BRAND`
pub const PLUGIN_BRAND: &str = "AIDA DSP";
/// DPF plugin label (`getLabel()`).
pub const PLUGIN_LABEL: &str = "AIDA_X";
/// `DISTRHO_PLUGIN_URI` (LV2 URI; also used as the homepage/manual link).
pub const PLUGIN_URI: &str = "http://aidadsp.cc/plugins/aidadsp-bundle/rt-neural-loader";
/// `DISTRHO_PLUGIN_CLAP_ID`
pub const CLAP_ID: &str = "cc.aidadsp.rt-neural-loader";
/// Plugin description (`getDescription()`).
pub const PLUGIN_DESCRIPTION: &str =
    "Simple loader for neural models using RTNeural inference engine.";
/// `getHomePage()`
pub const PLUGIN_HOMEPAGE: &str = "https://aidadsp.github.io/";
/// Support / issue tracker.
pub const PLUGIN_SUPPORT_URL: &str = "https://github.com/AidaDSP/AIDA-X/issues";

/// VST3 class ID.
///
/// TODO(compat): DPF derives its VST3 class UID from `DISTRHO_PLUGIN_UNIQUE_ID` (`aida`)
/// with `DPF_VST3_DONT_USE_BRAND_ID`; reproduce that exact 16-byte UID (the DPF sources
/// are not vendored in this repository) so hosts treat the Rust build as a drop-in
/// replacement for existing projects. Until then this is a distinct, stable ID.
pub const VST3_CLASS_ID: [u8; 16] = *b"AidaDspAidaXRust";

/// `kDefaultModelName`: bundled default neural model.
pub const DEFAULT_MODEL: &str = "tw40_california_clean.json";
/// `kDefaultCabinetName`: bundled default cabinet IR.
pub const DEFAULT_CABINET: &str = "V30-P2-audix-i5.wav";
