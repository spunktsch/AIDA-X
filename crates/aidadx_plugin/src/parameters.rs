//! Host-visible parameters and persisted state, ported from `kParameters[]` in
//! `src/DistrhoPluginCommon.hpp` and `initParameter()`/`initState()` in
//! `src/aidadsp-plugin.cpp`.
//!
//! Parameter IDs are the DPF *symbols* (e.g. `PREGAIN`, `MASTER`, `dpf_bypass`) so that
//! automation lanes and presets keep a stable, recognisable identity. Fields are declared
//! in the C++ enum order (`kParameterINLPF` .. `kParameterDCBLOCKER`), which nih-plug uses
//! as the host-facing order.
//!
//! The three DPF *output* parameters (`ModelInSize`, `MeterIn`, `MeterOut`) have no
//! equivalent in nih-plug (neither the CLAP nor the VST3 wrapper supports read-only
//! output parameters), so they are exposed through [`crate::PluginOutputs`] instead.

use std::sync::{Arc, RwLock};

use nih_plug::prelude::*;

/// DPF state key for the neural model file (`kStateModelFile`).
pub const STATE_KEY_MODEL: &str = "json";
/// DPF state key for the cabinet impulse response file (`kStateImpulseFile`).
pub const STATE_KEY_CABINET: &str = "cabinet";
/// DPF default value for file states; means "load the bundled default".
pub const STATE_DEFAULT_FILE: &str = "default";

/// EQ position (`enum EqPos` / `kEQPOS`). Variant order is the plain value: POST=0, PRE=1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum EqPosition {
    #[id = "post"]
    #[name = "POST"]
    Post,
    #[id = "pre"]
    #[name = "PRE"]
    Pre,
}

/// Mid band type (`enum MidEqType` / `kMTYPE`). PEAK=0, BANDPASS=1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum MidType {
    #[id = "peak"]
    #[name = "PEAK"]
    Peak,
    #[id = "bandpass"]
    #[name = "BANDPASS"]
    Bandpass,
}

/// Host parameter IDs in C++ `enum Parameters` order (input parameters only).
pub const PARAMETER_IDS: [&str; 21] = [
    "ANTIALIASING",
    "PREGAIN",
    "NETBYPASS",
    "EQBYPASS",
    "EQPOS",
    "BASS",
    "BFREQ",
    "MID",
    "MFREQ",
    "MIDQ",
    "MTYPE",
    "TREBLE",
    "TFREQ",
    "DEPTH",
    "PRESENCE",
    "MASTER",
    "CABSIMBYPASS",
    "dpf_bypass",
    "PARAM1",
    "PARAM2",
    "DCBLOCKER",
];

#[derive(Params)]
pub struct AidaParams {
    /// `kParameterINLPF`: input anti-aliasing low-pass amount. 0 disables the filter.
    #[id = "ANTIALIASING"]
    pub antialiasing: FloatParam,
    /// `kParameterINLEVEL`: input gain.
    #[id = "PREGAIN"]
    pub input: FloatParam,
    /// `kParameterNETBYPASS`: bypass the neural model stage.
    #[id = "NETBYPASS"]
    pub net_bypass: BoolParam,
    /// `kParameterEQBYPASS`: bypass the tone stack.
    #[id = "EQBYPASS"]
    pub eq_bypass: BoolParam,
    /// `kParameterEQPOS`: tone stack before or after the model.
    #[id = "EQPOS"]
    pub eq_pos: EnumParam<EqPosition>,
    /// `kParameterBASSGAIN`
    #[id = "BASS"]
    pub bass: FloatParam,
    /// `kParameterBASSFREQ`
    #[id = "BFREQ"]
    pub bass_freq: FloatParam,
    /// `kParameterMIDGAIN`
    #[id = "MID"]
    pub mid: FloatParam,
    /// `kParameterMIDFREQ`
    #[id = "MFREQ"]
    pub mid_freq: FloatParam,
    /// `kParameterMIDQ`
    #[id = "MIDQ"]
    pub mid_q: FloatParam,
    /// `kParameterMTYPE`
    #[id = "MTYPE"]
    pub mid_type: EnumParam<MidType>,
    /// `kParameterTREBLEGAIN`
    #[id = "TREBLE"]
    pub treble: FloatParam,
    /// `kParameterTREBLEFREQ`
    #[id = "TFREQ"]
    pub treble_freq: FloatParam,
    /// `kParameterDEPTH`
    #[id = "DEPTH"]
    pub depth: FloatParam,
    /// `kParameterPRESENCE`
    #[id = "PRESENCE"]
    pub presence: FloatParam,
    /// `kParameterOUTLEVEL`: output gain.
    #[id = "MASTER"]
    pub output: FloatParam,
    /// `kParameterCABSIMBYPASS`
    #[id = "CABSIMBYPASS"]
    pub cabsim_bypass: BoolParam,
    /// `kParameterGLOBALBYPASS` (designation: bypass).
    #[id = "dpf_bypass"]
    pub bypass: BoolParam,
    /// `kParameterPARAM1`: first conditioning input for parametric models.
    #[id = "PARAM1"]
    pub param1: FloatParam,
    /// `kParameterPARAM2`: second conditioning input for parametric models.
    #[id = "PARAM2"]
    pub param2: FloatParam,
    /// `kParameterDCBLOCKER`
    #[id = "DCBLOCKER"]
    pub dc_blocker: BoolParam,

    /// `kStateModelFile` ("json"): absolute path of the loaded `.json` model, or
    /// [`STATE_DEFAULT_FILE`] / empty for the bundled default.
    #[persist = "json"]
    pub model_file: Arc<RwLock<String>>,
    /// `kStateImpulseFile` ("cabinet"): absolute path of the loaded `.wav`/`.flac` IR, or
    /// [`STATE_DEFAULT_FILE`] / empty for the bundled default.
    #[persist = "cabinet"]
    pub cabinet_file: Arc<RwLock<String>>,
}

fn linear(name: &str, default: f32, min: f32, max: f32, unit: &'static str) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min, max })
        .with_unit(unit)
        .with_value_to_string(formatters::v2s_f32_rounded(2))
}

fn db(name: &str, min: f32, max: f32) -> FloatParam {
    linear(name, 0.0, min, max, " dB")
}

fn hz(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    linear(name, default, min, max, " Hz").with_value_to_string(formatters::v2s_f32_rounded(0))
}

/// DPF boolean parameters without enumeration labels.
fn on_off(name: &str, default: bool) -> BoolParam {
    BoolParam::new(name, default)
}

impl Default for AidaParams {
    fn default() -> Self {
        Self {
            // Value 0 is labelled "Off" in initParameter().
            antialiasing: linear("ANTIALIASING", 66.216, 0.0, 100.0, " %").with_value_to_string(
                Arc::new(|v| {
                    if v == 0.0 {
                        String::from("Off")
                    } else {
                        format!("{v:.1}")
                    }
                }),
            ),
            input: db("INPUT", -12.0, 12.0),
            net_bypass: on_off("NETBYPASS", false),
            eq_bypass: on_off("EQBYPASS", false),
            eq_pos: EnumParam::new("EQPOS", EqPosition::Post),
            bass: db("BASS", -8.0, 8.0),
            bass_freq: hz("BFREQ", 305.0, 75.0, 600.0),
            mid: db("MID", -8.0, 8.0),
            mid_freq: hz("MFREQ", 750.0, 150.0, 5000.0),
            mid_q: linear("MIDQ", 0.707, 0.2, 5.0, "").with_value_to_string(
                formatters::v2s_f32_rounded(3),
            ),
            mid_type: EnumParam::new("MTYPE", MidType::Peak),
            treble: db("TREBLE", -8.0, 8.0),
            treble_freq: hz("TFREQ", 2000.0, 1000.0, 4000.0),
            depth: db("DEPTH", -8.0, 8.0),
            presence: db("PRESENCE", -8.0, 8.0),
            output: db("OUTPUT", -15.0, 15.0),
            cabsim_bypass: on_off("CABSIMBYPASS", false),
            // initParameter() overrides kBYPASS with PROCESSING/BYPASSED labels.
            bypass: BoolParam::new("Bypass", false)
                .make_bypass()
                .with_value_to_string(Arc::new(|v| {
                    String::from(if v { "BYPASSED" } else { "PROCESSING" })
                }))
                .with_string_to_value(Arc::new(|s| match s.trim() {
                    s if s.eq_ignore_ascii_case("BYPASSED") => Some(true),
                    s if s.eq_ignore_ascii_case("PROCESSING") => Some(false),
                    _ => None,
                })),
            param1: linear("PARAM1", 0.0, 0.0, 1.0, ""),
            param2: linear("PARAM2", 0.0, 0.0, 1.0, ""),
            dc_blocker: on_off("DCBLOCKER", true),

            model_file: Arc::new(RwLock::new(String::from(STATE_DEFAULT_FILE))),
            cabinet_file: Arc::new(RwLock::new(String::from(STATE_DEFAULT_FILE))),
        }
    }
}

/// Plain-value snapshot of all input parameters, read once per block on the audio thread.
///
/// This is the boundary between nih-plug and the format-agnostic [`crate::audio`] engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSnapshot {
    pub antialiasing: f32,
    pub input_db: f32,
    pub net_bypass: bool,
    pub eq_bypass: bool,
    pub eq_pos: EqPosition,
    pub bass_db: f32,
    pub bass_freq: f32,
    pub mid_db: f32,
    pub mid_freq: f32,
    pub mid_q: f32,
    pub mid_type: MidType,
    pub treble_db: f32,
    pub treble_freq: f32,
    pub depth_db: f32,
    pub presence_db: f32,
    pub output_db: f32,
    pub cabsim_bypass: bool,
    pub bypass: bool,
    pub param1: f32,
    pub param2: f32,
    pub dc_blocker: bool,
}

impl Default for ParamSnapshot {
    /// The DPF defaults from `kParameters[]`.
    fn default() -> Self {
        AidaParams::default().snapshot()
    }
}

impl AidaParams {
    /// Read the current (unsmoothed) plain value of every input parameter.
    pub fn snapshot(&self) -> ParamSnapshot {
        ParamSnapshot {
            antialiasing: self.antialiasing.value(),
            input_db: self.input.value(),
            net_bypass: self.net_bypass.value(),
            eq_bypass: self.eq_bypass.value(),
            eq_pos: self.eq_pos.value(),
            bass_db: self.bass.value(),
            bass_freq: self.bass_freq.value(),
            mid_db: self.mid.value(),
            mid_freq: self.mid_freq.value(),
            mid_q: self.mid_q.value(),
            mid_type: self.mid_type.value(),
            treble_db: self.treble.value(),
            treble_freq: self.treble_freq.value(),
            depth_db: self.depth.value(),
            presence_db: self.presence.value(),
            output_db: self.output.value(),
            cabsim_bypass: self.cabsim_bypass.value(),
            bypass: self.bypass.value(),
            param1: self.param1.value(),
            param2: self.param2.value(),
            dc_blocker: self.dc_blocker.value(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (symbol, name, unit, default, min, max) straight from `kParameters[]`, plus the
    /// discrete-ness implied by `kParameterIsBoolean|kParameterIsInteger` / enum values.
    const DPF_TABLE: [(&str, &str, &str, f32, f32, f32, bool); 21] = [
        ("ANTIALIASING", "ANTIALIASING", "%", 66.216, 0.0, 100.0, false),
        ("PREGAIN", "INPUT", "dB", 0.0, -12.0, 12.0, false),
        ("NETBYPASS", "NETBYPASS", "", 0.0, 0.0, 1.0, true),
        ("EQBYPASS", "EQBYPASS", "", 0.0, 0.0, 1.0, true),
        ("EQPOS", "EQPOS", "", 0.0, 0.0, 1.0, true),
        ("BASS", "BASS", "dB", 0.0, -8.0, 8.0, false),
        ("BFREQ", "BFREQ", "Hz", 305.0, 75.0, 600.0, false),
        ("MID", "MID", "dB", 0.0, -8.0, 8.0, false),
        ("MFREQ", "MFREQ", "Hz", 750.0, 150.0, 5000.0, false),
        ("MIDQ", "MIDQ", "", 0.707, 0.2, 5.0, false),
        ("MTYPE", "MTYPE", "", 0.0, 0.0, 1.0, true),
        ("TREBLE", "TREBLE", "dB", 0.0, -8.0, 8.0, false),
        ("TFREQ", "TFREQ", "Hz", 2000.0, 1000.0, 4000.0, false),
        ("DEPTH", "DEPTH", "dB", 0.0, -8.0, 8.0, false),
        ("PRESENCE", "PRESENCE", "dB", 0.0, -8.0, 8.0, false),
        ("MASTER", "OUTPUT", "dB", 0.0, -15.0, 15.0, false),
        ("CABSIMBYPASS", "CABSIMBYPASS", "", 0.0, 0.0, 1.0, true),
        ("dpf_bypass", "Bypass", "", 0.0, 0.0, 1.0, true),
        ("PARAM1", "PARAM1", "", 0.0, 0.0, 1.0, false),
        ("PARAM2", "PARAM2", "", 0.0, 0.0, 1.0, false),
        ("DCBLOCKER", "DCBLOCKER", "", 1.0, 0.0, 1.0, true),
    ];

    #[test]
    fn parameters_match_dpf_table() {
        let params = AidaParams::default();
        let map = params.param_map();
        assert_eq!(map.len(), DPF_TABLE.len());

        for ((id, ptr, _group), (sym, name, unit, def, min, max, discrete)) in
            map.iter().zip(DPF_TABLE.iter())
        {
            assert_eq!(id, sym, "parameter order/ID");
            // SAFETY: `params` outlives every use of `ptr`.
            unsafe {
                assert_eq!(ptr.name(), *name, "{sym}: name");
                assert_eq!(ptr.unit().trim(), *unit, "{sym}: unit");
                assert!((ptr.default_plain_value() - def).abs() < 1e-4, "{sym}: default");
                assert!((ptr.preview_plain(0.0) - min).abs() < 1e-4, "{sym}: min");
                assert!((ptr.preview_plain(1.0) - max).abs() < 1e-4, "{sym}: max");
                assert_eq!(ptr.step_count() == Some(1), *discrete, "{sym}: discrete");
            }
        }
        assert_eq!(
            map.iter().map(|(id, ..)| id.as_str()).collect::<Vec<_>>(),
            PARAMETER_IDS
        );
    }

    #[test]
    fn bypass_is_flagged_and_labelled() {
        let params = AidaParams::default();
        assert!(params.bypass.flags().contains(ParamFlags::BYPASS));
        assert_eq!(params.bypass.normalized_value_to_string(0.0, false), "PROCESSING");
        assert_eq!(params.bypass.normalized_value_to_string(1.0, false), "BYPASSED");
        assert_eq!(params.antialiasing.normalized_value_to_string(0.0, false), "Off");
        assert_eq!(params.eq_pos.normalized_value_to_string(0.0, false), "POST");
        assert_eq!(params.eq_pos.normalized_value_to_string(1.0, false), "PRE");
        assert_eq!(params.mid_type.normalized_value_to_string(0.0, false), "PEAK");
        assert_eq!(params.mid_type.normalized_value_to_string(1.0, false), "BANDPASS");
    }

    #[test]
    fn file_paths_round_trip_through_persisted_state() {
        let params = AidaParams::default();
        *params.model_file.write().unwrap() = String::from("C:/models/amp.json");
        *params.cabinet_file.write().unwrap() = String::from("C:/irs/cab.wav");
        let fields = params.serialize_fields();
        assert!(fields.contains_key(STATE_KEY_MODEL));
        assert!(fields.contains_key(STATE_KEY_CABINET));

        let restored = AidaParams::default();
        assert_eq!(*restored.model_file.read().unwrap(), STATE_DEFAULT_FILE);
        restored.deserialize_fields(&fields);
        assert_eq!(*restored.model_file.read().unwrap(), "C:/models/amp.json");
        assert_eq!(*restored.cabinet_file.read().unwrap(), "C:/irs/cab.wav");
    }
}
