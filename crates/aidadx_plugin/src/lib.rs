//! AIDA-X as a CLAP and VST3 plugin, built on [nih-plug](https://github.com/robbert-vdh/nih-plug).
//!
//! - [`parameters`]: host parameters (ported 1:1 from DPF `kParameters[]`) and persisted
//!   model/IR file paths.
//! - [`audio`]: the format-agnostic signal chain (`aidadx_core` biquads, RTNeural model,
//!   two-stage convolver).
//! - [`loader`]: background thread that loads models/IRs and hands them to the engine
//!   through lock-free [`handoff`] slots.
//! - This module: nih-plug glue and the `clap_entry` / `GetPluginFactory` exports.
//!
//! No editor is provided yet; hosts show their generic parameter UI.

pub mod audio;
pub mod handoff;
pub mod loader;
pub mod parameters;
pub mod plugin_info;
#[doc(hidden)]
pub mod test_models;

use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::Arc;

use nih_plug::prelude::*;

pub use audio::{AidaEngine, PluginOutputs};
pub use loader::{FileLoader, FileSource};
pub use parameters::{AidaParams, ParamIdScheme, ParamSnapshot};

/// Selects the DPF host-ID layout an exported plugin type reproduces.
///
/// nih-plug hashes the same string parameter ID identically for CLAP and VST3, but DPF's
/// CLAP and VST3 IDs differ by two (VST3 has two internal parameters first), so each format
/// gets its own plugin type with its own string IDs (see [`parameters`]).
pub trait HostFormat: 'static + Send + Sync {
    const PARAM_ID_SCHEME: ParamIdScheme;
}

/// Marker for the CLAP export ([`AidaXClap`]).
pub struct ClapFormat;
/// Marker for the VST3 export ([`AidaXVst3`]).
pub struct Vst3Format;

impl HostFormat for ClapFormat {
    const PARAM_ID_SCHEME: ParamIdScheme = ParamIdScheme::Clap;
}
impl HostFormat for Vst3Format {
    const PARAM_ID_SCHEME: ParamIdScheme = ParamIdScheme::Vst3;
}

/// The plugin, generic only over which DPF parameter-ID layout it reproduces.
pub struct AidaX<F: HostFormat> {
    params: Arc<AidaParams>,
    outputs: Arc<PluginOutputs>,
    engine: AidaEngine,
    loader: FileLoader,
    _format: PhantomData<fn() -> F>,
}

/// The type exported through `clap_entry`.
pub type AidaXClap = AidaX<ClapFormat>;
/// The type exported through `GetPluginFactory`.
pub type AidaXVst3 = AidaX<Vst3Format>;

impl<F: HostFormat> Default for AidaX<F> {
    fn default() -> Self {
        let engine = AidaEngine::new();
        let loader = FileLoader::spawn(engine.model_slot(), engine.cabinet_slot());
        Self {
            params: Arc::new(AidaParams::new(F::PARAM_ID_SCHEME)),
            outputs: Arc::new(PluginOutputs::default()),
            engine,
            loader,
            _format: PhantomData,
        }
    }
}

impl<F: HostFormat> AidaX<F> {
    /// Values DPF exposed as output parameters (model input size, meters).
    pub fn outputs(&self) -> Arc<PluginOutputs> {
        self.outputs.clone()
    }

    /// The background loader (e.g. for [`FileLoader::last_error`]).
    pub fn loader(&self) -> &FileLoader {
        &self.loader
    }

    /// `setState("json", path)`: persist `path` and load it in the background.
    /// `""`/`"default"` selects the bundled model. Never blocks on file I/O.
    pub fn set_model_file(&self, path: &str) {
        if let Ok(mut s) = self.params.model_file.write() {
            *s = path.to_owned();
        }
        let p = self.params.snapshot();
        self.loader
            .request_model(FileSource::from_state(path), p.param1, p.param2, true);
    }

    /// `setState("cabinet", path)`: persist `path` and load it in the background, at the
    /// current sample rate (or at the next `initialize()` if none is known yet).
    pub fn set_cabinet_file(&self, path: &str) {
        if let Ok(mut s) = self.params.cabinet_file.write() {
            *s = path.to_owned();
        }
        if let Some(sr) = self.loader.sample_rate() {
            self.loader
                .request_cabinet(FileSource::from_state(path), sr, true);
        }
    }

    /// Queue the model and cabinet IR named by the persisted state (`"default"`/empty =
    /// the bundled [`loader::DEFAULT_MODEL_JSON`] / [`loader::DEFAULT_CABINET_WAV`]).
    /// Called from `initialize()`, i.e. after a state restore and on sample-rate changes;
    /// the loader skips requests identical to the previous one, so only a changed path
    /// (model) or a changed path/sample rate (cabinet) triggers work.
    fn load_files_from_state(&self, sample_rate: f64) {
        let model = self
            .params
            .model_file
            .read()
            .map(|s| s.clone())
            .unwrap_or_default();
        let cabinet = self
            .params
            .cabinet_file
            .read()
            .map(|s| s.clone())
            .unwrap_or_default();
        let p = self.params.snapshot();
        self.loader
            .request_model(FileSource::from_state(&model), p.param1, p.param2, false);
        self.loader
            .request_cabinet(FileSource::from_state(&cabinet), sample_rate, false);
    }
}

impl<F: HostFormat> Plugin for AidaX<F> {
    const NAME: &'static str = plugin_info::PLUGIN_NAME;
    const VENDOR: &'static str = plugin_info::PLUGIN_BRAND;
    const URL: &'static str = plugin_info::PLUGIN_HOMEPAGE;
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // DISTRHO_PLUGIN_NUM_INPUTS/OUTPUTS = 1, DISTRHO_PLUGIN_EXTRA_IO = { 2, 2 }.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    /// Accept nih-plug states from earlier Rust builds, which keyed parameters by the bare
    /// DPF symbol. (DPF's own state blob is not JSON and never reaches this point, see
    /// TICKET-020 findings.)
    fn filter_state(state: &mut PluginState) {
        parameters::migrate_symbol_keys(&mut state.params, F::PARAM_ID_SCHEME);
    }

    /// `sampleRateChanged()` + `bufferSizeChanged()`. Also called by nih-plug after a
    /// state restore, which is where persisted model/IR paths are (re)applied.
    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let snapshot = self.params.snapshot();
        self.engine.prepare(
            buffer_config.sample_rate as f64,
            buffer_config.max_buffer_size as usize,
            &snapshot,
        );
        self.load_files_from_state(buffer_config.sample_rate as f64);
        true
    }

    /// `activate()`
    fn reset(&mut self) {
        self.engine.reset();
        self.outputs
            .reset_meters
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.engine.set_params(&self.params.snapshot());

        // Mono processing on the first channel; with the 2-in/2-out layout the result is
        // copied to the second channel, as the C++ plugin does for stereo AU / extra IO.
        if let Some((first, rest)) = buffer.as_slice().split_first_mut() {
            self.engine.process(first, &self.outputs);
            for channel in rest {
                channel.copy_from_slice(first);
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for AidaXClap {
    const CLAP_ID: &'static str = plugin_info::CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(plugin_info::PLUGIN_DESCRIPTION);
    const CLAP_MANUAL_URL: Option<&'static str> = Some(plugin_info::PLUGIN_HOMEPAGE);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(plugin_info::PLUGIN_SUPPORT_URL);
    // DISTRHO_PLUGIN_CLAP_FEATURES
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::MultiEffects,
        ClapFeature::Mono,
    ];
}

#[cfg(feature = "vst3")]
impl Vst3Plugin for AidaXVst3 {
    const VST3_CLASS_ID: [u8; 16] = plugin_info::VST3_CLASS_ID;
    // DISTRHO_PLUGIN_VST3_CATEGORIES "Fx|Dynamics|Mono"
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Dynamics,
        Vst3SubCategory::Mono,
    ];
}

#[cfg(feature = "clap")]
nih_export_clap!(AidaXClap);

/// Scoped so the allow only covers the upstream `vst3-com` vtable macro, which trips this
/// future-incompatibility lint. `#[no_mangle]` exports still reach the DLL from here.
#[cfg(feature = "vst3")]
#[allow(semicolon_in_expressions_from_non_local_macros)]
mod vst3_export {
    use super::AidaXVst3;
    use nih_plug::nih_export_vst3;

    nih_export_vst3!(AidaXVst3);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    const WAIT: Duration = Duration::from_secs(30);

    /// What `initialize()` does after nih-plug restored the persisted `json`/`cabinet`.
    fn restore(plugin: &mut AidaXClap, model: &str, cabinet: &str, sr: f64) {
        *plugin.params.model_file.write().unwrap() = model.to_owned();
        *plugin.params.cabinet_file.write().unwrap() = cabinet.to_owned();
        let snapshot = plugin.params.snapshot();
        plugin.engine.prepare(sr, 256, &snapshot);
        plugin.load_files_from_state(sr);
        assert!(plugin.loader.wait_idle(WAIT));
        plugin.engine.process(&mut [0.0; 256], &plugin.outputs);
    }

    #[test]
    fn state_restore_loads_persisted_files() {
        let files = concat!(env!("CARGO_MANIFEST_DIR"), "/../../files/");
        let model = format!("{files}tw40_california_clean_deerinkstudios.json");
        let cabinet = format!("{files}V30-P2-audix-i5-deerinkstudios.wav");
        let mut plugin = AidaXClap::default();
        restore(&mut plugin, &model, &cabinet, 44_100.0);
        assert!(plugin.engine.model().is_some());
        assert!(plugin.engine.cabinet().is_some());
        assert_eq!(plugin.outputs.model_input_size.load(Ordering::Relaxed), 1);
        assert_eq!(plugin.loader.last_error(), None);
        assert_eq!(plugin.loader.sample_rate(), Some(44_100.0));
    }

    #[test]
    fn default_and_missing_state_use_bundled_files() {
        let mut plugin = AidaXClap::default();
        restore(&mut plugin, "default", "", 48_000.0);
        assert!(plugin.engine.model().is_some() && plugin.engine.cabinet().is_some());
        assert_eq!(plugin.loader.last_error(), None);

        let mut plugin = AidaXClap::default();
        restore(&mut plugin, "/nonexistent/x.json", "/nonexistent/x.wav", 48_000.0);
        assert!(plugin.loader.last_error().is_some(), "error reported");
        assert!(plugin.engine.model().is_some() && plugin.engine.cabinet().is_some());
    }

    #[test]
    fn set_files_persist_and_load() {
        let mut plugin = AidaXClap::default();
        restore(&mut plugin, "default", "default", 48_000.0);
        plugin.set_cabinet_file("/nonexistent/x.wav");
        plugin.set_model_file("default");
        assert!(plugin.loader.wait_idle(WAIT));
        assert_eq!(*plugin.params.cabinet_file.read().unwrap(), "/nonexistent/x.wav");
        assert!(plugin.loader.last_error().unwrap().contains("x.wav"));
        assert!(plugin.engine.model_slot().has_pending(), "forced reload");
    }
}
