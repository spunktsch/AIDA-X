//! Model and cabinet IR loading helpers (`loadDefaultModel`, `loadDefaultCabinet` and the
//! file loaders in `src/aidadsp-plugin.cpp`). Everything here allocates; call it off the
//! audio thread and hand the result to the engine through its `Handoff` slots.

use std::path::PathBuf;

use aidadx_core::convolver::TwoStageConvolver;
use aidadx_core::ir::{load_ir_bytes, load_ir_file};
use aidadx_core::model::DynamicModel;
use nih_plug::nih_log;

use crate::parameters::STATE_DEFAULT_FILE;


/// Bundled default model (`files/tw40_california_clean_deerinkstudios.json`).
pub static DEFAULT_MODEL_JSON: &[u8] =
    include_bytes!("../../../files/tw40_california_clean_deerinkstudios.json");
/// Bundled default cabinet IR (`files/V30-P2-audix-i5-deerinkstudios.wav`).
pub static DEFAULT_CABINET_WAV: &[u8] =
    include_bytes!("../../../files/V30-P2-audix-i5-deerinkstudios.wav");

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

