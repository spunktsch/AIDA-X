//! Golden-reference tests: Rust biquad / tone control vs. the original C++ code (TICKET-022).
//!
//! `tests/fixtures/biquad_golden.{bin,idx}` are produced by
//! `tests/cpp_reference/gen_biquad_ref.cpp` (run `build_and_run.bat` / `.sh` to regenerate),
//! which compiles `src/Biquad.cpp`, DPF's `ValueSmoother.hpp` and verbatim copies of the
//! tone-control code of `src/aidadsp-plugin.cpp`. The scenarios below must stay in sync with
//! that file; every fixture record must be consumed by exactly one Rust scenario.
//!
//! Tolerance: `|rust - cpp| <= 1e-5 * max(1, peak(|cpp|))` per record. All inputs are at most
//! 1.0 in magnitude, so for unity-ish outputs this is an absolute 1e-5 (about 2^-17, i.e.
//! ~7 f32 ulps at full scale); boosted outputs (up to +8 dB EQ and +15 dB master) are allowed
//! the same error relative to their peak, since f32 rounding error scales with magnitude.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use aidadx_core::biquad::{
    db_to_coeff, AidaToneControl, Biquad, BiquadType, CascadeParameters, ExponentialValueSmoother,
    LinearValueSmoother, ValueSmoother,
};

const TOLERANCE: f64 = 1e-5;
const FILTER_LEN: usize = 1024;
const CHAIN_LEN: usize = 4096;
const BLOCK: usize = 256;
const SMOOTHER_LEN: usize = 12000;

// ---------------------------------------------------------------------------------------------
// Fixture loading

struct Fixture {
    data: Vec<f32>,
    index: BTreeMap<String, (usize, usize)>,
}

impl Fixture {
    fn load() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let bytes = std::fs::read(dir.join("biquad_golden.bin")).expect("read biquad_golden.bin");
        assert_eq!(bytes.len() % 4, 0);
        let data = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect::<Vec<_>>();
        let idx = std::fs::read_to_string(dir.join("biquad_golden.idx")).expect("read idx");
        let mut index = BTreeMap::new();
        for line in idx.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            let name = it.next().unwrap().to_string();
            let off: usize = it.next().unwrap().parse().unwrap();
            let len: usize = it.next().unwrap().parse().unwrap();
            assert!(off + len <= data.len(), "record {name} out of range");
            index.insert(name, (off, len));
        }
        Self { data, index }
    }

    fn get(&self, name: &str) -> &[f32] {
        let (off, len) = *self
            .index
            .get(name)
            .unwrap_or_else(|| panic!("fixture record {name} missing"));
        &self.data[off..off + len]
    }

    fn names_with_prefix(&self, prefix: &str) -> BTreeSet<String> {
        self.index
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
}

/// Max absolute error, or a description of the failure.
fn compare(name: &str, rust: &[f32], cpp: &[f32]) -> Result<f64, String> {
    assert_eq!(rust.len(), cpp.len(), "{name}: length");
    let peak = cpp.iter().fold(1.0_f64, |m, &v| m.max((v as f64).abs()));
    let mut max_err = 0.0_f64;
    let mut worst = 0;
    for (i, (&r, &c)) in rust.iter().zip(cpp).enumerate() {
        let e = (r as f64 - c as f64).abs();
        if e > max_err || e.is_nan() {
            max_err = e;
            worst = i;
        }
    }
    let allowed = TOLERANCE * peak;
    if max_err <= allowed {
        Ok(max_err)
    } else {
        Err(format!(
            "{name}: max |rust - cpp| = {max_err:.3e} at sample {worst} \
             (rust {}, cpp {}), allowed {allowed:.3e}",
            rust[worst], cpp[worst]
        ))
    }
}

struct Report {
    group: &'static str,
    worst: f64,
    worst_name: String,
    checked: BTreeSet<String>,
    failures: Vec<String>,
}

impl Report {
    fn new(group: &'static str) -> Self {
        Self {
            group,
            worst: 0.0,
            worst_name: String::new(),
            checked: BTreeSet::new(),
            failures: Vec::new(),
        }
    }

    fn check(&mut self, fx: &Fixture, name: String, rust: &[f32]) {
        let err = match compare(&name, rust, fx.get(&name)) {
            Ok(err) => err,
            Err(msg) => {
                self.failures.push(msg);
                f64::INFINITY
            }
        };
        if err >= self.worst {
            self.worst = err;
            self.worst_name = name.clone();
        }
        assert!(self.checked.insert(name.clone()), "{name} checked twice");
    }

    /// Every fixture record of this group must have been reproduced.
    fn finish(&self, fx: &Fixture, prefix: &str) {
        let expected = fx.names_with_prefix(prefix);
        let missing: Vec<_> = expected.difference(&self.checked).collect();
        assert!(
            missing.is_empty(),
            "fixture records not covered: {missing:?}"
        );
        println!(
            "{}: {} records, max abs error {:.3e} ({})",
            self.group,
            self.checked.len(),
            self.worst,
            self.worst_name
        );
        assert!(
            self.failures.is_empty(),
            "{} of {} records exceed the tolerance:\n{}",
            self.failures.len(),
            self.checked.len(),
            self.failures.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 1. Single filters (gen_biquad_ref.cpp: kFilterCases)

#[derive(Clone, Copy)]
enum Change {
    None,
    PeakGain(f64),
    Fc(f64),
    Q(f64),
}

#[rustfmt::skip]
const FILTER_CASES: &[(&str, BiquadType, f64, f64, f64, Change)] = &[
    ("lowpass_1k_q0707", BiquadType::LowPass, 1000.0 / 48000.0, 0.707, 0.0, Change::None),
    ("lowpass_0.3_q2", BiquadType::LowPass, 0.3, 2.0, 0.0, Change::None),
    ("lowpass_inlpf_max", BiquadType::LowPass, 0.495, 0.707, 0.0, Change::None),
    ("highpass_35hz", BiquadType::HighPass, 35.0 / 48000.0, 0.707, 0.0, Change::None),
    ("highpass_0.1_q0.5", BiquadType::HighPass, 0.1, 0.5, 0.0, Change::None),
    ("bandpass_750_q0707", BiquadType::BandPass, 750.0 / 48000.0, 0.707, 0.0, Change::None),
    ("bandpass_0.05_q5", BiquadType::BandPass, 0.05, 5.0, 0.0, Change::None),
    ("notch_0.02_q1", BiquadType::Notch, 0.02, 1.0, 0.0, Change::None),
    ("notch_0.2_q0.3", BiquadType::Notch, 0.2, 0.3, 0.0, Change::None),
    ("peak_boost_75_q0707_+8", BiquadType::Peak, 75.0 / 48000.0, 0.707, 8.0, Change::None),
    ("peak_cut_0.1_q5_-8", BiquadType::Peak, 0.1, 5.0, -8.0, Change::None),
    ("peak_flat_0.05_q1_0", BiquadType::Peak, 0.05, 1.0, 0.0, Change::None),
    ("lowshelf_boost_305_+8", BiquadType::LowShelf, 305.0 / 48000.0, 0.707, 8.0, Change::None),
    ("lowshelf_cut_75_-8", BiquadType::LowShelf, 75.0 / 48000.0, 0.707, -8.0, Change::None),
    ("lowshelf_flat_0.01_0", BiquadType::LowShelf, 0.01, 0.707, 0.0, Change::None),
    ("highshelf_boost_2k_+8", BiquadType::HighShelf, 2000.0 / 48000.0, 0.707, 8.0, Change::None),
    ("highshelf_cut_900_-8", BiquadType::HighShelf, 900.0 / 48000.0, 0.707, -8.0, Change::None),
    ("highshelf_boost_0.3_+3", BiquadType::HighShelf, 0.3, 0.707, 3.0, Change::None),
    ("peak_gain_change", BiquadType::Peak, 0.02, 1.5, 6.0, Change::PeakGain(-6.0)),
    ("lowshelf_fc_change", BiquadType::LowShelf, 0.005, 0.707, 5.0, Change::Fc(0.02)),
    ("bandpass_q_change", BiquadType::BandPass, 0.03, 0.5, 0.0, Change::Q(4.0)),
];

const FILTER_SIGNALS: &[&str] = &["impulse", "sweep", "noise"];

#[test]
fn golden_single_filters() {
    let fx = Fixture::load();
    let mut report = Report::new("filters");
    for &(name, ty, fc, q, gain, change) in FILTER_CASES {
        for sig in FILTER_SIGNALS {
            let input = fx.get(&format!("input/filter/{sig}"));
            assert_eq!(input.len(), FILTER_LEN);
            let mut bq = Biquad::new();
            bq.set_biquad(ty, fc, q, gain);
            let half = FILTER_LEN / 2;
            // First half sample by sample, second half through the block API after the change.
            let mut out: Vec<f32> = input[..half].iter().map(|&x| bq.process(x)).collect();
            match change {
                Change::None => {}
                Change::PeakGain(g) => bq.set_peak_gain_db(g),
                Change::Fc(f) => bq.set_cutoff(f),
                Change::Q(v) => bq.set_q(v),
            }
            let mut tail = vec![0.0; FILTER_LEN - half];
            bq.process_buffer(&input[half..], &mut tail);
            out.extend_from_slice(&tail);
            report.check(&fx, format!("filter/{name}/{sig}"), &out);
        }
    }
    report.finish(&fx, "filter/");
}

// ---------------------------------------------------------------------------------------------
// 2. Tone-control chain (gen_biquad_ref.cpp: chainCases)

// C++ parameter indices (src/DistrhoPluginCommon.hpp)
const INLPF: u32 = 0;
const INLEVEL: u32 = 1;
const EQBYPASS: u32 = 3;
const EQPOS: u32 = 4;
const BASSGAIN: u32 = 5;
const BASSFREQ: u32 = 6;
const MIDGAIN: u32 = 7;
const MIDFREQ: u32 = 8;
const MIDQ: u32 = 9;
const MTYPE: u32 = 10;
const TREBLEGAIN: u32 = 11;
const TREBLEFREQ: u32 = 12;
const DEPTH: u32 = 13;
const PRESENCE: u32 = 14;
const OUTLEVEL: u32 = 15;
const DCBLOCKER: u32 = 20;
/// Pseudo-parameter: call `set_sample_rate` again (C++ `sampleRateChanged`).
const EV_SAMPLE_RATE: u32 = 1000;

/// Mirror of the C++ `parameters[]` array (float values, as the host passes them).
fn set_param_value(p: &mut CascadeParameters, index: u32, value: f32) {
    let v = value as f64;
    match index {
        INLPF => p.in_lpf = v,
        INLEVEL => p.in_level = v,
        EQBYPASS => p.eq_bypass = v,
        EQPOS => p.eq_pos = v,
        BASSGAIN => p.bass_gain = v,
        BASSFREQ => p.bass_freq = v,
        MIDGAIN => p.mid_gain = v,
        MIDFREQ => p.mid_freq = v,
        MIDQ => p.mid_q = v,
        MTYPE => p.mid_type = v,
        TREBLEGAIN => p.treble_gain = v,
        TREBLEFREQ => p.treble_freq = v,
        DEPTH => p.depth = v,
        PRESENCE => p.presence = v,
        OUTLEVEL => p.out_level = v,
        DCBLOCKER => p.dc_blocker = v,
        _ => panic!("unhandled parameter {index}"),
    }
}

struct ChainCase {
    name: &'static str,
    sample_rate: f64,
    activate: bool,
    init: &'static [(u32, f32)],
    events: &'static [(usize, u32, f32)],
}

#[rustfmt::skip]
const CHAIN_CASES: &[ChainCase] = &[
    ChainCase { name: "defaults", sample_rate: 48000.0, activate: true, init: &[], events: &[] },
    ChainCase {
        name: "eq_post_all_bands",
        sample_rate: 48000.0,
        activate: true,
        init: &[
            (BASSGAIN, 6.0), (BASSFREQ, 150.0), (MIDGAIN, -4.0), (MIDFREQ, 1200.0), (MIDQ, 2.0),
            (TREBLEGAIN, 3.0), (TREBLEFREQ, 3500.0), (DEPTH, 5.0), (PRESENCE, -6.0),
        ],
        events: &[],
    },
    ChainCase {
        name: "eq_pre_all_bands",
        sample_rate: 48000.0,
        activate: true,
        init: &[
            (EQPOS, 1.0), (BASSGAIN, -7.0), (BASSFREQ, 500.0), (MIDGAIN, 8.0), (MIDFREQ, 400.0),
            (MIDQ, 0.5), (TREBLEGAIN, -8.0), (TREBLEFREQ, 1000.0), (DEPTH, -3.0), (PRESENCE, 8.0),
        ],
        events: &[],
    },
    ChainCase {
        name: "mid_bandpass",
        sample_rate: 48000.0,
        activate: true,
        init: &[(MIDFREQ, 1000.0), (MIDQ, 3.0), (BASSGAIN, 8.0)],
        events: &[(0, MTYPE, 1.0), (0, EV_SAMPLE_RATE, 0.0)],
    },
    ChainCase {
        name: "mid_type_switch_quirk",
        sample_rate: 48000.0,
        activate: true,
        init: &[(MIDGAIN, 6.0), (MIDFREQ, 900.0)],
        events: &[(1024, MTYPE, 1.0), (2048, EV_SAMPLE_RATE, 0.0), (3072, MTYPE, 0.0)],
    },
    ChainCase {
        name: "dc_blocker_off",
        sample_rate: 48000.0,
        activate: true,
        init: &[(DCBLOCKER, 0.0), (BASSGAIN, 4.0)],
        events: &[],
    },
    ChainCase {
        name: "lpf_off",
        sample_rate: 48000.0,
        activate: true,
        init: &[],
        events: &[(0, INLPF, 0.0)],
    },
    ChainCase {
        name: "lpf_max",
        sample_rate: 48000.0,
        activate: true,
        init: &[(INLPF, 100.0)],
        events: &[],
    },
    ChainCase {
        name: "lpf_sweep_param",
        sample_rate: 48000.0,
        activate: true,
        init: &[],
        events: &[(1024, INLPF, 10.0), (2048, INLPF, 90.0), (3072, INLPF, 50.0)],
    },
    ChainCase {
        name: "eq_bypass",
        sample_rate: 48000.0,
        activate: true,
        init: &[(EQBYPASS, 1.0), (BASSGAIN, 8.0)],
        events: &[],
    },
    ChainCase {
        name: "gain_ramp_cold_start",
        sample_rate: 48000.0,
        activate: false,
        init: &[(INLEVEL, 6.0), (OUTLEVEL, -3.0)],
        events: &[],
    },
    ChainCase {
        name: "automation",
        sample_rate: 48000.0,
        activate: true,
        init: &[(INLEVEL, -6.0)],
        events: &[
            (512, INLEVEL, 12.0), (768, OUTLEVEL, -15.0), (1024, BASSGAIN, 8.0),
            (1280, TREBLEFREQ, 4000.0), (1536, MIDQ, 5.0), (1792, MIDGAIN, -8.0), (2048, EQPOS, 1.0),
            (2304, BASSFREQ, 75.0), (2560, DEPTH, 8.0), (2816, PRESENCE, -8.0), (3072, OUTLEVEL, 15.0),
            (3328, INLEVEL, -12.0), (3584, DCBLOCKER, 0.0), (3840, MIDFREQ, 5000.0),
        ],
    },
    ChainCase {
        name: "sr44100_all_bands",
        sample_rate: 44100.0,
        activate: true,
        init: &[
            (BASSGAIN, 3.0), (MIDGAIN, 5.0), (TREBLEGAIN, -5.0), (DEPTH, 8.0), (PRESENCE, 2.0),
            (OUTLEVEL, 4.0),
        ],
        events: &[],
    },
];

const CHAIN_SIGNALS: &[&str] = &["impulse", "step", "sweep", "noise"];

fn run_chain(case: &ChainCase, input: &[f32]) -> Vec<f32> {
    let sr = case.sample_rate;
    // C++: constructor loads defaults into parameters[], then sampleRateChanged().
    let mut params = CascadeParameters::default();
    for &(i, v) in case.init {
        set_param_value(&mut params, i, v);
    }
    let mut tc = AidaToneControl::new();
    tc.set_sample_rate(&params, sr);
    if case.activate {
        tc.reset();
    }
    let mut out = vec![0.0_f32; input.len()];
    for pos in (0..input.len()).step_by(BLOCK) {
        for &(at, index, value) in case.events {
            if at != pos {
                continue;
            }
            if index == EV_SAMPLE_RATE {
                tc.set_sample_rate(&params, sr);
            } else {
                set_param_value(&mut params, index, value);
                tc.set_parameter(index, value as f64, sr);
            }
        }
        tc.process(&input[pos..pos + BLOCK], &mut out[pos..pos + BLOCK]);
    }
    out
}

#[test]
fn golden_tone_control_chain() {
    let fx = Fixture::load();
    let mut report = Report::new("chain");
    for case in CHAIN_CASES {
        for sig in CHAIN_SIGNALS {
            let input = fx.get(&format!("input/chain/{sig}"));
            assert_eq!(input.len(), CHAIN_LEN);
            let out = run_chain(case, input);
            report.check(&fx, format!("chain/{}/{sig}", case.name), &out);
        }
    }
    report.finish(&fx, "chain/");
}

// ---------------------------------------------------------------------------------------------
// 3. Smoothers (gen_biquad_ref.cpp: runSmootherCases)

#[test]
fn golden_smoothers() {
    let fx = Fixture::load();
    let mut report = Report::new("smoothers");

    for (name, t60) in [
        ("exp_t60_1", 1.0),
        ("exp_t60_0.25", 0.25),
        ("exp_t60_0.1", 0.1),
    ] {
        let mut sm = ExponentialValueSmoother::new();
        sm.set_time_constant(t60);
        sm.set_sample_rate(48000.0);
        sm.set_target_value(db_to_coeff(6.0));
        let out: Vec<f32> = (0..SMOOTHER_LEN)
            .map(|i| {
                if i == SMOOTHER_LEN / 3 {
                    sm.set_target_value(db_to_coeff(-15.0));
                }
                if i == 2 * SMOOTHER_LEN / 3 {
                    sm.set_target_value(0.0);
                }
                sm.next() as f32
            })
            .collect();
        report.check(&fx, format!("smoother/{name}"), &out);
    }

    {
        let mut sm = ExponentialValueSmoother::new();
        sm.set_sample_rate(44100.0);
        sm.set_time_constant(1.0);
        sm.set_target_value(db_to_coeff(-6.0));
        sm.clear_to_target_value();
        sm.set_target_value(db_to_coeff(12.0));
        let out: Vec<f32> = (0..SMOOTHER_LEN).map(|_| sm.next() as f32).collect();
        report.check(&fx, "smoother/exp_t60_1_sr44100_cleared".into(), &out);
    }

    {
        let mut sm = LinearValueSmoother::new();
        sm.set_time_constant(0.1);
        sm.set_sample_rate(48000.0);
        sm.set_target_value(0.25);
        sm.clear_to_target_value();
        let out: Vec<f32> = (0..SMOOTHER_LEN)
            .map(|i| {
                match i {
                    100 => sm.set_target_value(1.0),
                    3000 => sm.set_target_value(0.0),
                    7000 | 10000 => sm.set_target_value(0.7),
                    _ => {}
                }
                sm.next() as f32
            })
            .collect();
        report.check(&fx, "smoother/linear_tau_0.1".into(), &out);
    }

    report.finish(&fx, "smoother/");
}
