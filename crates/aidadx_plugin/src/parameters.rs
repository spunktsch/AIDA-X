//! Host-visible parameters and persisted state, ported from `kParameters[]` in
//! `src/DistrhoPluginCommon.hpp` and `initParameter()`/`initState()` in
//! `src/aidadsp-plugin.cpp`.
//!
//! Fields are declared in the C++ enum order (`kParameterINLPF` .. `kParameterDCBLOCKER`),
//! which nih-plug uses as the host-facing order.
//!
//! # Host parameter IDs (DPF compatibility, TICKET-020)
//!
//! DPF does not hash anything: it exposes parameter *indices* as host IDs.
//!
//! - CLAP (`DistrhoPluginCLAP.cpp`, `getParameterInfo`): `info->id = index`.
//! - VST3 (`DistrhoPluginVST3.cpp`, `getParameterInfo`): `info->param_id = rindex`, where
//!   the first `kVst3InternalParameterCount` IDs are DPF-internal. AIDA-X has a UI and no
//!   direct access, so `DPF_VST3_USES_SEPARATE_CONTROLLER` is 1 and the internal IDs are
//!   `0 = Buffer Size`, `1 = Sample Rate` (hidden, read-only); no latency/programs/MIDI.
//!   Plugin parameter `index` is therefore VST3 ID `index + 2`.
//!
//! nih-plug derives host IDs from the string ID with a fixed, non-overridable hash
//! (`hash_param_id` in `src/wrapper/util.rs`, mirrored in [`nih_plug_param_hash`]). That hash
//! is invertible, so instead of forking nih-plug each string ID is *constructed* such that it
//! hashes to the DPF ID: `"<DPF symbol>#"` followed by a 7-character suffix solved for the
//! target hash (see [`dpf_compatible_param_id`]). Because CLAP and VST3 need different
//! targets, the ID scheme is chosen per exported plugin type ([`ParamIdScheme`]).
//!
//! The three DPF *output* parameters (`ModelInSize`, `MeterIn`, `MeterOut`) have no
//! equivalent in nih-plug (neither the CLAP nor the VST3 wrapper supports read-only
//! output parameters), so they are exposed through [`crate::PluginOutputs`] instead. Their
//! DPF IDs (CLAP 21..=23, VST3 23..=25) are simply left unused.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use nih_plug::params::persist::{self, PersistentField};
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

/// DPF parameter *symbols* in C++ `enum Parameters` order (input parameters only). The
/// position in this table is the DPF parameter index.
pub const PARAMETER_SYMBOLS: [&str; 21] = [
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

/// Number of DPF-internal VST3 parameters in front of the plugin parameters for AIDA-X
/// (`kVst3InternalParameterCount`: buffer size + sample rate, see the module docs).
pub const DPF_VST3_INTERNAL_PARAMETER_COUNT: u32 = 2;

/// Which host-ID layout to reproduce. Selected per exported plugin type, because the same
/// string ID is hashed identically by nih-plug's CLAP and VST3 wrappers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamIdScheme {
    /// DPF CLAP: host ID = DPF parameter index.
    Clap,
    /// DPF VST3: host ID = DPF parameter index + [`DPF_VST3_INTERNAL_PARAMETER_COUNT`].
    Vst3,
}

impl ParamIdScheme {
    /// The host-facing ID DPF uses for the parameter at `index` (an index into
    /// [`PARAMETER_SYMBOLS`]).
    pub const fn dpf_host_id(self, index: u32) -> u32 {
        match self {
            Self::Clap => index,
            Self::Vst3 => index + DPF_VST3_INTERNAL_PARAMETER_COUNT,
        }
    }

    /// The nih-plug string IDs for all input parameters, in [`PARAMETER_SYMBOLS`] order.
    pub fn string_ids(self) -> [String; PARAMETER_SYMBOLS.len()] {
        std::array::from_fn(|i| {
            dpf_compatible_param_id(PARAMETER_SYMBOLS[i], self.dpf_host_id(i as u32))
        })
    }
}

/// nih-plug masks the top bit (reserved for host parameters in VST3).
const HASH_MASK: u32 = !(1 << 31);
/// Length of the solved suffix. `31^7 > 2^31`, so 7 base-31 digits reach every hash.
const SUFFIX_LEN: usize = 7;
/// Suffix digits are the 31 consecutive ASCII characters `'0'..='N'`, so a digit `d` adds
/// exactly `b'0' + d` to the polynomial hash.
const SUFFIX_FIRST_CHAR: u8 = b'0';

/// Unmasked polynomial (Rabin) hash, `h = h * 31 + byte`, wrapping.
const fn raw_hash(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0;
    let mut i = 0;
    while i < bytes.len() {
        hash = hash.wrapping_mul(31).wrapping_add(bytes[i] as u32);
        i += 1;
    }
    hash
}

/// Exact mirror of nih-plug's private `nih_plug::wrapper::util::hash_param_id` at the
/// pinned revision (`de42101`), which both the CLAP and VST3 wrappers use to turn a string
/// parameter ID into the host-facing `u32` ID.
pub const fn nih_plug_param_hash(id: &str) -> u32 {
    raw_hash(id.as_bytes()) & HASH_MASK
}

/// Build a readable string parameter ID for `symbol` whose [`nih_plug_param_hash`] is
/// exactly `target` (`target < 2^31`).
///
/// With prefix `p = "<symbol>#"` and suffix digits `d_0..d_6` (characters `'0' + d_i`):
///
/// ```text
/// hash = raw(p) * 31^7 + sum_i ('0' + d_i) * 31^(6-i)            (mod 2^32)
///      = raw(p) * 31^7 + raw("0000000") + D,   D = sum_i d_i * 31^(6-i)
/// ```
///
/// so `D = target - raw(p) * 31^7 - raw("0000000") (mod 2^31)`, written in base 31. Any
/// `D < 2^31 < 31^7` fits in 7 digits, and the masked hash then equals `target`.
///
/// The result is part of nih-plug's saved state (parameter keys), so this construction
/// must never change.
pub fn dpf_compatible_param_id(symbol: &str, target: u32) -> String {
    assert!(
        target <= HASH_MASK,
        "VST3/nih-plug parameter IDs are 31 bit"
    );

    let mut id = String::with_capacity(symbol.len() + 1 + SUFFIX_LEN);
    id.push_str(symbol);
    id.push('#');

    let pow = 31u32.wrapping_pow(SUFFIX_LEN as u32);
    let base = raw_hash(&[SUFFIX_FIRST_CHAR; SUFFIX_LEN]);
    let mut remaining = target
        .wrapping_sub(raw_hash(id.as_bytes()).wrapping_mul(pow))
        .wrapping_sub(base)
        & HASH_MASK;

    let mut digits = [SUFFIX_FIRST_CHAR; SUFFIX_LEN];
    for digit in digits.iter_mut().rev() {
        *digit = SUFFIX_FIRST_CHAR + (remaining % 31) as u8;
        remaining /= 31;
    }
    debug_assert_eq!(remaining, 0);
    id.extend(digits.iter().map(|&d| d as char));

    debug_assert_eq!(nih_plug_param_hash(&id), target);
    id
}

/// All AIDA-X host parameters plus the persisted file paths.
///
/// `Params` is implemented by hand (not derived) so that the string IDs can depend on the
/// [`ParamIdScheme`]; see the module docs.
pub struct AidaParams {
    /// String IDs handed to nih-plug, in [`PARAMETER_SYMBOLS`] order.
    ids: [String; PARAMETER_SYMBOLS.len()],
    scheme: ParamIdScheme,

    /// `kParameterINLPF`: input anti-aliasing low-pass amount. 0 disables the filter.
    pub antialiasing: FloatParam,
    /// `kParameterINLEVEL`: input gain.
    pub input: FloatParam,
    /// `kParameterNETBYPASS`: bypass the neural model stage.
    pub net_bypass: BoolParam,
    /// `kParameterEQBYPASS`: bypass the tone stack.
    pub eq_bypass: BoolParam,
    /// `kParameterEQPOS`: tone stack before or after the model.
    pub eq_pos: EnumParam<EqPosition>,
    /// `kParameterBASSGAIN`
    pub bass: FloatParam,
    /// `kParameterBASSFREQ`
    pub bass_freq: FloatParam,
    /// `kParameterMIDGAIN`
    pub mid: FloatParam,
    /// `kParameterMIDFREQ`
    pub mid_freq: FloatParam,
    /// `kParameterMIDQ`
    pub mid_q: FloatParam,
    /// `kParameterMTYPE`
    pub mid_type: EnumParam<MidType>,
    /// `kParameterTREBLEGAIN`
    pub treble: FloatParam,
    /// `kParameterTREBLEFREQ`
    pub treble_freq: FloatParam,
    /// `kParameterDEPTH`
    pub depth: FloatParam,
    /// `kParameterPRESENCE`
    pub presence: FloatParam,
    /// `kParameterOUTLEVEL`: output gain.
    pub output: FloatParam,
    /// `kParameterCABSIMBYPASS`
    pub cabsim_bypass: BoolParam,
    /// `kParameterGLOBALBYPASS` (designation: bypass).
    pub bypass: BoolParam,
    /// `kParameterPARAM1`: first conditioning input for parametric models.
    pub param1: FloatParam,
    /// `kParameterPARAM2`: second conditioning input for parametric models.
    pub param2: FloatParam,
    /// `kParameterDCBLOCKER`
    pub dc_blocker: BoolParam,

    /// `kStateModelFile` ("json"): absolute path of the loaded `.json` model, or
    /// [`STATE_DEFAULT_FILE`] / empty for the bundled default.
    pub model_file: Arc<RwLock<String>>,
    /// `kStateImpulseFile` ("cabinet"): absolute path of the loaded `.wav`/`.flac` IR, or
    /// [`STATE_DEFAULT_FILE`] / empty for the bundled default.
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

/// ANTIALIASING display: exactly 0 disables the filter and reads "Off"; any other value is
/// shown with one decimal, but never as "0.0" (the filter is still active), so that
/// value -> text -> value -> text is stable (hosts and clap-validator check this).
fn antialiasing_to_string(v: f32) -> String {
    if v == 0.0 {
        String::from("Off")
    } else {
        format!("{:.1}", v.max(0.1))
    }
}

/// Inverse of [`antialiasing_to_string`]. Hosts may pass the text with the unit appended
/// (nih-plug's CLAP wrapper does: `"Off %"`, `"66.2 %"`).
fn antialiasing_from_string(s: &str) -> Option<f32> {
    let s = s.trim().trim_end_matches('%').trim_end();
    if s.eq_ignore_ascii_case("off") {
        Some(0.0)
    } else {
        s.parse().ok()
    }
}

/// DPF boolean parameters without enumeration labels.
fn on_off(name: &str, default: bool) -> BoolParam {
    BoolParam::new(name, default)
}

impl Default for AidaParams {
    /// The CLAP layout (host ID = DPF index). The plugin types use [`AidaParams::new`].
    fn default() -> Self {
        Self::new(ParamIdScheme::Clap)
    }
}

impl AidaParams {
    /// Create the parameters with DPF-compatible host IDs for `scheme`.
    pub fn new(scheme: ParamIdScheme) -> Self {
        Self {
            ids: scheme.string_ids(),
            scheme,

            // Value 0 is labelled "Off" in initParameter().
            antialiasing: linear("ANTIALIASING", 66.216, 0.0, 100.0, " %")
                .with_value_to_string(Arc::new(antialiasing_to_string))
                .with_string_to_value(Arc::new(antialiasing_from_string)),
            input: db("INPUT", -12.0, 12.0),
            net_bypass: on_off("NETBYPASS", false),
            eq_bypass: on_off("EQBYPASS", false),
            eq_pos: EnumParam::new("EQPOS", EqPosition::Post),
            bass: db("BASS", -8.0, 8.0),
            bass_freq: hz("BFREQ", 305.0, 75.0, 600.0),
            mid: db("MID", -8.0, 8.0),
            mid_freq: hz("MFREQ", 750.0, 150.0, 5000.0),
            mid_q: linear("MIDQ", 0.707, 0.2, 5.0, "")
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
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

    /// The host-ID layout these parameters reproduce.
    pub fn scheme(&self) -> ParamIdScheme {
        self.scheme
    }

    /// The nih-plug string IDs, in [`PARAMETER_SYMBOLS`] order.
    pub fn string_ids(&self) -> &[String; PARAMETER_SYMBOLS.len()] {
        &self.ids
    }

    /// Parameter pointers in [`PARAMETER_SYMBOLS`] order.
    fn param_ptrs(&self) -> [ParamPtr; PARAMETER_SYMBOLS.len()] {
        [
            self.antialiasing.as_ptr(),
            self.input.as_ptr(),
            self.net_bypass.as_ptr(),
            self.eq_bypass.as_ptr(),
            self.eq_pos.as_ptr(),
            self.bass.as_ptr(),
            self.bass_freq.as_ptr(),
            self.mid.as_ptr(),
            self.mid_freq.as_ptr(),
            self.mid_q.as_ptr(),
            self.mid_type.as_ptr(),
            self.treble.as_ptr(),
            self.treble_freq.as_ptr(),
            self.depth.as_ptr(),
            self.presence.as_ptr(),
            self.output.as_ptr(),
            self.cabsim_bypass.as_ptr(),
            self.bypass.as_ptr(),
            self.param1.as_ptr(),
            self.param2.as_ptr(),
            self.dc_blocker.as_ptr(),
        ]
    }

    /// Persisted (`#[persist]`-style) fields under their DPF state keys.
    fn persisted_fields(&self) -> [(&'static str, &Arc<RwLock<String>>); 2] {
        [
            (STATE_KEY_MODEL, &self.model_file),
            (STATE_KEY_CABINET, &self.cabinet_file),
        ]
    }
}

/// Rename parameter keys of a nih-plug state saved by an earlier Rust build (which used the
/// bare DPF symbols as string IDs) to the IDs of `scheme`. Unknown keys are left alone.
pub fn migrate_symbol_keys<V>(params: &mut BTreeMap<String, V>, scheme: ParamIdScheme) {
    for (symbol, id) in PARAMETER_SYMBOLS.iter().zip(scheme.string_ids()) {
        if !params.contains_key(&id) {
            if let Some(value) = params.remove(*symbol) {
                params.insert(id, value);
            }
        }
    }
}

// SAFETY: every `ParamPtr` points into `self`, which nih-plug keeps alive (behind an `Arc`)
// for as long as it uses the pointers; the same contract `#[derive(Params)]` relies on.
unsafe impl Params for AidaParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        self.ids
            .iter()
            .zip(self.param_ptrs())
            .map(|(id, ptr)| (id.clone(), ptr, String::new()))
            .collect()
    }

    /// Same encoding as the `#[persist = "..."]` derive: each field as its own JSON string.
    fn serialize_fields(&self) -> BTreeMap<String, String> {
        let mut serialized = BTreeMap::new();
        for (key, field) in self.persisted_fields() {
            match PersistentField::map(field, persist::serialize_field) {
                Ok(data) => {
                    serialized.insert(String::from(key), data);
                }
                Err(err) => {
                    nih_debug_assert_failure!("Could not serialize '{}': {}", key, err);
                }
            }
        }
        serialized
    }

    fn deserialize_fields(&self, serialized: &BTreeMap<String, String>) {
        for (key, field) in self.persisted_fields() {
            let Some(data) = serialized.get(key) else {
                continue;
            };
            match persist::deserialize_field::<String>(data) {
                Ok(value) => PersistentField::set(field, value),
                Err(err) => {
                    nih_debug_assert_failure!("Could not deserialize '{}': {}", key, err);
                }
            }
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
        (
            "ANTIALIASING",
            "ANTIALIASING",
            "%",
            66.216,
            0.0,
            100.0,
            false,
        ),
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
            assert_eq!(
                id.split_once('#').map(|(s, _)| s),
                Some(*sym),
                "parameter order/ID"
            );
            // SAFETY: `params` outlives every use of `ptr`.
            unsafe {
                assert_eq!(ptr.name(), *name, "{sym}: name");
                assert_eq!(ptr.unit().trim(), *unit, "{sym}: unit");
                assert!(
                    (ptr.default_plain_value() - def).abs() < 1e-4,
                    "{sym}: default"
                );
                assert!((ptr.preview_plain(0.0) - min).abs() < 1e-4, "{sym}: min");
                assert!((ptr.preview_plain(1.0) - max).abs() < 1e-4, "{sym}: max");
                assert_eq!(ptr.step_count() == Some(1), *discrete, "{sym}: discrete");
            }
        }
        assert_eq!(
            DPF_TABLE.iter().map(|row| row.0).collect::<Vec<_>>(),
            PARAMETER_SYMBOLS
        );
    }

    /// DPF host IDs: CLAP `info->id = index`; VST3 `info->param_id = rindex` with
    /// `rindex = index + kVst3InternalParameterCount` (2 for AIDA-X).
    #[test]
    fn host_ids_match_dpf_indices() {
        for (scheme, offset) in [(ParamIdScheme::Clap, 0u32), (ParamIdScheme::Vst3, 2)] {
            let params = AidaParams::new(scheme);
            let map = params.param_map();
            assert_eq!(map.len(), PARAMETER_SYMBOLS.len());
            for (index, (id, ..)) in map.iter().enumerate() {
                assert_eq!(
                    nih_plug_param_hash(id),
                    index as u32 + offset,
                    "{scheme:?} {id}"
                );
            }
        }
        // Spot checks: bypass (kParameterGLOBALBYPASS = 17), first and last parameter.
        assert_eq!(
            nih_plug_param_hash(&ParamIdScheme::Vst3.string_ids()[17]),
            19
        );
        assert_eq!(nih_plug_param_hash(&ParamIdScheme::Clap.string_ids()[0]), 0);
        assert_eq!(
            nih_plug_param_hash(&ParamIdScheme::Clap.string_ids()[20]),
            20
        );
    }

    /// The string IDs are stored in nih-plug state; pin the construction.
    #[test]
    fn string_ids_are_stable() {
        let clap = ParamIdScheme::Clap.string_ids();
        let vst3 = ParamIdScheme::Vst3.string_ids();
        assert_eq!(clap[1], PINNED_CLAP_PREGAIN);
        assert_eq!(vst3[17], PINNED_VST3_BYPASS);
        for id in clap.iter().chain(vst3.iter()) {
            assert!(
                id.is_ascii() && !id.contains('"') && !id.contains('\\'),
                "{id}"
            );
        }
    }

    const PINNED_CLAP_PREGAIN: &str = "PREGAIN#08NL4:N";
    const PINNED_VST3_BYPASS: &str = "dpf_bypass#227=;E9";

    #[test]
    fn hash_mirror_matches_nih_plug_definition() {
        // h = h * 31 + byte, top bit cleared (see nih-plug src/wrapper/util.rs).
        assert_eq!(nih_plug_param_hash(""), 0);
        assert_eq!(nih_plug_param_hash("a"), 97);
        assert_eq!(nih_plug_param_hash("ab"), 97 * 31 + 98);
        // Wrapping + masking: 31^7 overflows u32.
        let long = "zzzzzzzzzzzz";
        let mut h: u64 = 0;
        for b in long.bytes() {
            h = (h * 31 + b as u64) % (1 << 32);
        }
        assert_eq!(nih_plug_param_hash(long), (h as u32) & !(1 << 31));
        for target in [0, 1, 2, 22, 0x7fff_ffff, 123_456_789] {
            assert_eq!(
                nih_plug_param_hash(&dpf_compatible_param_id("X", target)),
                target
            );
        }
    }

    #[test]
    fn symbol_keyed_state_is_migrated() {
        let mut map = BTreeMap::new();
        map.insert(String::from("PREGAIN"), 3.0f32);
        map.insert(String::from("unknown"), 1.0f32);
        migrate_symbol_keys(&mut map, ParamIdScheme::Vst3);
        assert_eq!(map.get(&ParamIdScheme::Vst3.string_ids()[1]), Some(&3.0));
        assert!(!map.contains_key("PREGAIN"));
        assert!(map.contains_key("unknown"));
    }

    #[test]
    fn bypass_is_flagged_and_labelled() {
        let params = AidaParams::default();
        assert!(params.bypass.flags().contains(ParamFlags::BYPASS));
        assert_eq!(
            params.bypass.normalized_value_to_string(0.0, false),
            "PROCESSING"
        );
        assert_eq!(
            params.bypass.normalized_value_to_string(1.0, false),
            "BYPASSED"
        );
        assert_eq!(
            params.antialiasing.normalized_value_to_string(0.0, false),
            "Off"
        );
        assert_eq!(params.eq_pos.normalized_value_to_string(0.0, false), "POST");
        assert_eq!(params.eq_pos.normalized_value_to_string(1.0, false), "PRE");
        assert_eq!(
            params.mid_type.normalized_value_to_string(0.0, false),
            "PEAK"
        );
        assert_eq!(
            params.mid_type.normalized_value_to_string(1.0, false),
            "BANDPASS"
        );
    }

    /// "Off" must parse back (with and without the unit the CLAP wrapper appends), and no
    /// non-zero value may display as "Off"/"0.0" (TICKET-021).
    #[test]
    fn antialiasing_text_round_trips() {
        let p = AidaParams::default();
        let aa = &p.antialiasing;
        assert_eq!(aa.normalized_value_to_string(0.0, true), "Off %");
        for text in ["Off", "Off %", "off", " OFF% "] {
            assert_eq!(aa.string_to_normalized_value(text), Some(0.0), "{text:?}");
        }
        assert_eq!(aa.normalized_value_to_string(1e-7, false), "0.1");
        for normalized in [0.0, 1e-7, 0.0004, 0.123_45, 0.5, 0.662_16, 1.0] {
            let text = aa.normalized_value_to_string(normalized, true);
            let back = aa.string_to_normalized_value(&text).expect(&text);
            assert_eq!(aa.normalized_value_to_string(back, true), text);
        }
        assert_eq!(aa.string_to_normalized_value("garbage"), None);
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
