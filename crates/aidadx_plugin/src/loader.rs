//! Non-real-time loading of neural models and cabinet IRs (`loadModelFromFile`,
//! `loadDefaultModel`, `loadCabinetFromFile`, `loadDefaultCabinet` in
//! `src/aidadsp-plugin.cpp`).
//!
//! A [`FileLoader`] owns one background thread. Requests are sent over a channel (from
//! `initialize()`, i.e. state restore / sample-rate change, or from an editor), the thread
//! parses/decodes/resamples and builds the objects, pre-warms models, and publishes them
//! into the engine's lock-free [`Handoff`] slots. Every replaced object comes back through
//! the slot's retired cell and is freed by this thread (after each request and every
//! [`GC_INTERVAL`]), never by the audio thread.
//!
//! Fallbacks, as in C++:
//! - `"default"` or an empty value selects the bundled model/IR ([`DEFAULT_MODEL_JSON`],
//!   [`DEFAULT_CABINET_WAV`], embedded with `include_bytes!`).
//! - A file that fails to load is reported (log + [`FileLoader::last_error`]) and the
//!   currently active object is kept. If nothing usable is active yet (first load, or for
//!   the cabinet: nothing loaded at the current sample rate), the bundled default is
//!   loaded instead, so the plugin never ends up without a model or with an IR at the
//!   wrong rate.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use aidadx_core::convolver::TwoStageConvolver;
use aidadx_core::ir::{load_ir_bytes, load_ir_file};
use aidadx_core::model::DynamicModel;
use nih_plug::{nih_error, nih_log};

use crate::audio::{CabinetSlot, ModelSlot};
use crate::handoff::Handoff;
use crate::parameters::STATE_DEFAULT_FILE;

/// Bundled default model (`files/tw40_california_clean_deerinkstudios.json`).
pub static DEFAULT_MODEL_JSON: &[u8] =
    include_bytes!("../../../files/tw40_california_clean_deerinkstudios.json");
/// Bundled default cabinet IR (`files/V30-P2-audix-i5-deerinkstudios.wav`).
pub static DEFAULT_CABINET_WAV: &[u8] =
    include_bytes!("../../../files/V30-P2-audix-i5-deerinkstudios.wav");

/// How often the loader thread frees objects retired by the audio thread when idle.
pub const GC_INTERVAL: Duration = Duration::from_millis(100);

/// Where a model or IR comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
    /// The bundled default.
    Default,
    /// A file on disk.
    Path(PathBuf),
}

impl FileSource {
    /// Interprets a persisted `json`/`cabinet` state value like C++ `setState()`:
    /// empty or `"default"` means the bundled default.
    pub fn from_state(value: &str) -> Self {
        if value.is_empty() || value == STATE_DEFAULT_FILE {
            Self::Default
        } else {
            Self::Path(PathBuf::from(value))
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Default => "bundled default".to_owned(),
            Self::Path(p) => format!("'{}'", p.display()),
        }
    }
}

/// Parses and builds a model. Allocates; call off the audio thread.
pub fn load_model(source: &FileSource) -> Result<DynamicModel, String> {
    let json = match source {
        FileSource::Default => std::str::from_utf8(DEFAULT_MODEL_JSON)
            .map_err(|e| e.to_string())?
            .to_owned(),
        FileSource::Path(path) => std::fs::read_to_string(path).map_err(|e| e.to_string())?,
    };
    DynamicModel::from_json(&json).map_err(|e| e.to_string())
}

/// Decodes, resamples to `sample_rate` and builds a cabinet convolver (C++ block sizes).
/// Allocates; call off the audio thread.
pub fn load_cabinet(source: &FileSource, sample_rate: f64) -> Result<TwoStageConvolver, String> {
    let ir = match source {
        FileSource::Default => load_ir_bytes(DEFAULT_CABINET_WAV, sample_rate),
        FileSource::Path(path) => load_ir_file(path, sample_rate),
    }
    .map_err(|e| e.to_string())?;
    nih_log!(
        "AIDA-X: cabinet IR: {} channel(s), {} Hz, {} frames -> {} frames at {} Hz",
        ir.source_channels,
        ir.source_sample_rate,
        ir.source_frames,
        ir.samples.len(),
        ir.sample_rate
    );
    ir.build_convolver().map_err(|e| e.to_string())
}

struct ModelRequest {
    source: FileSource,
    param1: f32,
    param2: f32,
}

#[derive(Clone, PartialEq)]
struct CabinetRequest {
    source: FileSource,
    sample_rate: f64,
}

/// A request plus its `force` flag (bypass the "same as last request" check).
enum Request {
    Model(ModelRequest, bool),
    Cabinet(CabinetRequest, bool),
}

#[derive(Default)]
struct Shared {
    /// Requests sent but not yet fully handled.
    outstanding: AtomicUsize,
    /// Sample rate of the most recent cabinet request (`f64` bits; 0 = unknown).
    sample_rate: AtomicU64,
    last_error: Mutex<Option<String>>,
}

impl Shared {
    fn report(&self, msg: String) {
        nih_error!("AIDA-X: {msg}");
        if let Ok(mut e) = self.last_error.lock() {
            *e = Some(msg);
        }
    }
}

/// Handle to the background loading thread. Dropping it stops and joins the thread.
pub struct FileLoader {
    tx: Option<Sender<Request>>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl FileLoader {
    /// Starts the loader thread, publishing into the given engine slots.
    pub fn spawn(model_slot: Arc<ModelSlot>, cabinet_slot: Arc<CabinetSlot>) -> Self {
        let (tx, rx) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let worker = Worker {
            model_slot,
            cabinet_slot,
            shared: shared.clone(),
            model_requested: None,
            model_loaded: false,
            cabinet_requested: None,
            cabinet_loaded_rate: None,
        };
        let thread = std::thread::Builder::new()
            .name("aidax-loader".into())
            .spawn(move || worker.run(rx))
            .expect("failed to spawn the AIDA-X loader thread");
        Self {
            tx: Some(tx),
            shared,
            thread: Some(thread),
        }
    }

    fn send(&self, req: Request) {
        if let Some(tx) = &self.tx {
            self.shared.outstanding.fetch_add(1, Ordering::AcqRel);
            if tx.send(req).is_err() {
                self.shared.outstanding.fetch_sub(1, Ordering::AcqRel);
                self.shared
                    .report("loader thread is gone; request dropped".into());
            }
        }
    }

    /// Requests a model. `param1`/`param2` are the conditioning values used for the
    /// 2048-sample pre-roll. Unless `force`, a request for the source requested last is
    /// ignored (hosts call `initialize()` repeatedly).
    pub fn request_model(&self, source: FileSource, param1: f32, param2: f32, force: bool) {
        self.send(Request::Model(
            ModelRequest {
                source,
                param1,
                param2,
            },
            force,
        ));
    }

    /// Requests a cabinet IR prepared for `sample_rate`. Unless `force`, a request equal
    /// to the last one (same source and rate) is ignored.
    pub fn request_cabinet(&self, source: FileSource, sample_rate: f64, force: bool) {
        self.shared
            .sample_rate
            .store(sample_rate.to_bits(), Ordering::Relaxed);
        self.send(Request::Cabinet(
            CabinetRequest {
                source,
                sample_rate,
            },
            force,
        ));
    }

    /// Sample rate of the last cabinet request, if any.
    pub fn sample_rate(&self) -> Option<f64> {
        let sr = f64::from_bits(self.shared.sample_rate.load(Ordering::Relaxed));
        (sr > 0.0).then_some(sr)
    }

    /// True when every request sent so far has been handled.
    pub fn is_idle(&self) -> bool {
        self.shared.outstanding.load(Ordering::Acquire) == 0
    }

    /// Blocks (polling) until [`is_idle`](Self::is_idle) or `timeout`. For tests/tools;
    /// never call from the audio thread.
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while !self.is_idle() {
            if start.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    /// The most recent load error, if any (for an editor to display).
    pub fn last_error(&self) -> Option<String> {
        self.shared.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Drop for FileLoader {
    fn drop(&mut self) {
        self.tx = None; // disconnects the channel, the thread exits its loop
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Worker {
    model_slot: Arc<ModelSlot>,
    cabinet_slot: Arc<CabinetSlot>,
    shared: Arc<Shared>,
    model_requested: Option<FileSource>,
    model_loaded: bool,
    cabinet_requested: Option<CabinetRequest>,
    cabinet_loaded_rate: Option<f64>,
}

impl Worker {
    fn run(mut self, rx: Receiver<Request>) {
        loop {
            let first = match rx.recv_timeout(GC_INTERVAL) {
                Ok(r) => r,
                Err(RecvTimeoutError::Timeout) => {
                    self.collect_garbage();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            };
            // Coalesce a burst (hosts may call initialize() several times): the latest
            // request of each kind wins, a forced one stays forced.
            let (mut model, mut cabinet, mut handled) = (None, None, 0);
            let (mut model_force, mut cabinet_force) = (false, false);
            for req in std::iter::once(first).chain(rx.try_iter()) {
                handled += 1;
                match req {
                    Request::Model(r, force) => {
                        model = Some(r);
                        model_force |= force;
                    }
                    Request::Cabinet(r, force) => {
                        cabinet = Some(r);
                        cabinet_force |= force;
                    }
                }
            }
            if let Some(r) = model {
                self.handle_model(r, model_force);
            }
            if let Some(r) = cabinet {
                self.handle_cabinet(r, cabinet_force);
            }
            self.collect_garbage();
            self.shared.outstanding.fetch_sub(handled, Ordering::AcqRel);
        }
        self.collect_garbage();
    }

    fn collect_garbage(&self) {
        self.model_slot.collect_garbage();
        self.cabinet_slot.collect_garbage();
    }

    fn handle_model(&mut self, req: ModelRequest, force: bool) {
        if !force && self.model_requested.as_ref() == Some(&req.source) {
            return;
        }
        self.model_requested = Some(req.source.clone());
        let ok = self.load_model(&req.source, req.param1, req.param2);
        if !ok && !self.model_loaded && req.source != FileSource::Default {
            nih_log!("AIDA-X: falling back to the bundled default model");
            self.load_model(&FileSource::Default, req.param1, req.param2);
        }
    }

    fn handle_cabinet(&mut self, req: CabinetRequest, force: bool) {
        if !force && self.cabinet_requested.as_ref() == Some(&req) {
            return;
        }
        self.cabinet_requested = Some(req.clone());
        let ok = self.load_cabinet(&req.source, req.sample_rate);
        // Keep the current IR on failure only if it was prepared for this rate.
        if !ok
            && self.cabinet_loaded_rate != Some(req.sample_rate)
            && req.source != FileSource::Default
        {
            nih_log!("AIDA-X: falling back to the bundled default cabinet IR");
            self.load_cabinet(&FileSource::Default, req.sample_rate);
        }
    }

    fn load_model(&mut self, source: &FileSource, param1: f32, param2: f32) -> bool {
        let result = load_model(source).and_then(|mut model| {
            // Pre-buffer to avoid clicks: reset + 2048 zeros (`loadModelFromStream`).
            model.prewarm(param1, param2).map_err(|e| e.to_string())?;
            Ok(model)
        });
        match result {
            Ok(model) => {
                nih_log!(
                    "AIDA-X: loaded model {} ({}, input size {})",
                    source.describe(),
                    model.arch,
                    model.input_size()
                );
                publish(&self.model_slot, model);
                self.model_loaded = true;
                true
            }
            Err(e) => {
                self.shared
                    .report(format!("unable to load model {}: {e}", source.describe()));
                false
            }
        }
    }

    fn load_cabinet(&mut self, source: &FileSource, sample_rate: f64) -> bool {
        match load_cabinet(source, sample_rate) {
            Ok(conv) => {
                nih_log!("AIDA-X: loaded cabinet IR {}", source.describe());
                publish(&self.cabinet_slot, conv);
                self.cabinet_loaded_rate = Some(sample_rate);
                true
            }
            Err(e) => {
                self.shared.report(format!(
                    "unable to load cabinet IR {}: {e}",
                    source.describe()
                ));
                false
            }
        }
    }
}

fn publish<T>(slot: &Handoff<T>, value: T) {
    slot.publish(Box::new(value));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AidaEngine;
    use crate::parameters::ParamSnapshot;
    use crate::PluginOutputs;

    const WAIT: Duration = Duration::from_secs(30);
    const FILES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../files/");

    fn setup() -> (AidaEngine, PluginOutputs, FileLoader) {
        let mut engine = AidaEngine::new();
        engine.prepare(48_000.0, 256, &ParamSnapshot::default());
        let loader = FileLoader::spawn(engine.model_slot(), engine.cabinet_slot());
        (engine, PluginOutputs::default(), loader)
    }

    fn file(name: &str) -> FileSource {
        FileSource::Path(PathBuf::from(FILES).join(name))
    }

    fn missing() -> FileSource {
        file("does-not-exist.json")
    }

    #[test]
    fn state_values_map_to_sources() {
        assert_eq!(FileSource::from_state(""), FileSource::Default);
        assert_eq!(FileSource::from_state("default"), FileSource::Default);
        assert_eq!(
            FileSource::from_state("/a/b.json"),
            FileSource::Path("/a/b.json".into())
        );
    }

    #[test]
    fn bundled_defaults_load() {
        let m = load_model(&FileSource::Default).unwrap();
        assert_eq!(m.input_size(), 1);
        let c = load_cabinet(&FileSource::Default, 44_100.0).unwrap();
        assert!(c.is_initialized());
    }

    #[test]
    fn loads_in_background_and_hands_off() {
        let (mut engine, out, loader) = setup();
        loader.request_model(
            file("tw40_california_clean_deerinkstudios.json"),
            0.0,
            0.0,
            false,
        );
        loader.request_cabinet(file("V30-P2-audix-i5-deerinkstudios.wav"), 48_000.0, false);
        assert!(loader.wait_idle(WAIT));
        assert!(engine.model_slot().has_pending() && engine.cabinet_slot().has_pending());
        engine.process(&mut [0.0; 256], &out);
        assert!(engine.model().is_some() && engine.cabinet().is_some());
        assert_eq!(out.model_input_size.load(Ordering::Relaxed), 1);
        assert_eq!(loader.last_error(), None);
    }

    #[test]
    fn failed_first_load_falls_back_to_default() {
        let (mut engine, out, loader) = setup();
        loader.request_model(missing(), 0.0, 0.0, false);
        loader.request_cabinet(missing(), 48_000.0, false);
        assert!(loader.wait_idle(WAIT));
        let err = loader.last_error().expect("error reported");
        assert!(err.contains("does-not-exist"), "{err}");
        engine.process(&mut [0.0; 256], &out);
        assert!(engine.model().is_some(), "default model loaded instead");
        assert!(engine.cabinet().is_some(), "default IR loaded instead");
    }

    #[test]
    fn failed_later_load_keeps_current_objects() {
        let (mut engine, out, loader) = setup();
        loader.request_model(FileSource::Default, 0.0, 0.0, false);
        loader.request_cabinet(FileSource::Default, 48_000.0, false);
        assert!(loader.wait_idle(WAIT));
        engine.process(&mut [0.0; 256], &out);

        // A WAV is not a model; a missing IR at the same rate keeps the current IR.
        loader.request_model(file("V30-P2-audix-i5-deerinkstudios.wav"), 0.0, 0.0, false);
        loader.request_cabinet(missing(), 48_000.0, false);
        assert!(loader.wait_idle(WAIT));
        assert!(loader.last_error().is_some());
        assert!(!engine.model_slot().has_pending(), "current model kept");
        assert!(!engine.cabinet_slot().has_pending(), "current IR kept");
    }

    #[test]
    fn duplicate_requests_are_skipped_and_rate_change_reloads_ir() {
        let (mut engine, out, loader) = setup();
        loader.request_model(FileSource::Default, 0.0, 0.0, false);
        loader.request_cabinet(FileSource::Default, 48_000.0, false);
        assert!(loader.wait_idle(WAIT));
        engine.process(&mut [0.0; 256], &out);

        // initialize() again with the same state and rate: nothing to do.
        loader.request_model(FileSource::Default, 0.0, 0.0, false);
        loader.request_cabinet(FileSource::Default, 48_000.0, false);
        assert!(loader.wait_idle(WAIT));
        assert!(!engine.model_slot().has_pending() && !engine.cabinet_slot().has_pending());

        // Host sample-rate change: the IR is re-prepared, the model is not reloaded.
        engine.prepare(96_000.0, 256, &ParamSnapshot::default());
        loader.request_model(FileSource::Default, 0.0, 0.0, false);
        loader.request_cabinet(FileSource::Default, 96_000.0, false);
        assert!(loader.wait_idle(WAIT));
        assert!(!engine.model_slot().has_pending());
        assert!(engine.cabinet_slot().has_pending());
        assert_eq!(loader.sample_rate(), Some(96_000.0));

        // A missing IR at a new rate falls back to the default at that rate.
        engine.process(&mut [0.0; 256], &out);
        loader.request_cabinet(missing(), 44_100.0, false);
        assert!(loader.wait_idle(WAIT));
        assert!(engine.cabinet_slot().has_pending());

        // Forced requests (user re-selects the same file) always reload.
        engine.process(&mut [0.0; 256], &out);
        loader.request_model(FileSource::Default, 0.0, 0.0, true);
        assert!(loader.wait_idle(WAIT));
        assert!(engine.model_slot().has_pending());
    }

    #[test]
    fn retired_objects_are_freed_by_the_loader_thread() {
        let (mut engine, out, loader) = setup();
        for _ in 0..2 {
            loader.request_model(FileSource::Default, 0.0, 0.0, true);
            assert!(loader.wait_idle(WAIT));
            engine.process(&mut [0.0; 64], &out);
        }
        assert!(engine.model_slot().has_retired() || engine.model().is_some());
        let start = Instant::now();
        while engine.model_slot().has_retired() {
            assert!(start.elapsed() < WAIT, "periodic garbage collection");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
