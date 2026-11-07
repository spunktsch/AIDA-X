//! Format-agnostic AIDA-X signal chain, ported from `AidaDSPLoaderPlugin::run()` in
//! `src/aidadsp-plugin.cpp`.
//!
//! Chain (mono, in place):
//!
//! ```text
//! input ─┬─ [anti-aliasing LPF] ─ [pre-gain] ─ [EQ if PRE] ─ [neural model] ─ [DC blocker]
//!        │   ─ [cabinet IR] ─ [EQ if POST] ─ [master] ─┬─ bypass crossfade ─ output
//!        └────────────────── dry copy ─────────────────┘
//! ```
//!
//! The neural model and cabinet IR stages are currently **pass-through TODO hooks**
//! (see [`AidaEngine::apply_model`] and [`AidaEngine::apply_cabinet`]).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use aidadx_core::biquad::{
    db_to_coeff, map, Biquad, BiquadType, ExponentialValueSmoother, GainRamp, LinearValueSmoother,
    ValueSmoother,
};
use nih_plug::prelude::AtomicF32;

use crate::parameters::{EqPosition, MidType, ParamSnapshot};

/// Q used by all fixed-Q tone filters (`COMMON_Q`).
const COMMON_Q: f64 = 0.707;
/// Fixed centre frequency of the DEPTH peak filter (`DEPTH_FREQ`).
const DEPTH_FREQ: f64 = 75.0;
/// Fixed corner frequency of the PRESENCE high shelf (`PRESENCE_FREQ`).
const PRESENCE_FREQ: f64 = 900.0;
/// DC blocker high-pass corner, in Hz.
const DC_BLOCKER_FREQ: f64 = 35.0;
/// Anti-aliasing LPF normalized cutoff at ANTIALIASING = 0 % (`INLPF_MAX_CO`).
const INLPF_MAX_CO: f64 = 0.99 * 0.5;
/// Anti-aliasing LPF normalized cutoff at ANTIALIASING = 100 % (`INLPF_MIN_CO`).
const INLPF_MIN_CO: f64 = 0.25 * 0.5;
/// Gain compensation for the cabinet IR, -12 dB (`kCabinetMaxGain`).
pub const CABINET_MAX_GAIN: f64 = 0.251;
/// Meter floor in dB (`kMinimumMeterDb`).
pub const MINIMUM_METER_DB: f64 = -60.0;

/// Values DPF exposed as *output* parameters (`ModelInSize`, `MeterIn`, `MeterOut`), plus
/// the `reset-meters` state command. nih-plug has no output parameters, so a future
/// editor reads these directly.
#[derive(Debug)]
pub struct PluginOutputs {
    /// `kParameterModelInputSize`: 0 = ERROR, 1 = SNAPSHOT, 2 = WITH 1 PARAM, 3 = WITH 2 PARAMS.
    pub model_input_size: AtomicU32,
    /// `kParameterMeterIn`: linear input peak (0..2).
    pub meter_in: AtomicF32,
    /// `kParameterMeterOut`: linear output peak (0..2).
    pub meter_out: AtomicF32,
    /// Equivalent of `setState("reset-meters", ...)`.
    pub reset_meters: AtomicBool,
}

impl Default for PluginOutputs {
    fn default() -> Self {
        Self {
            model_input_size: AtomicU32::new(0),
            meter_in: AtomicF32::new(0.0),
            meter_out: AtomicF32::new(0.0),
            reset_meters: AtomicBool::new(true),
        }
    }
}

/// The AIDA-X DSP engine: owns all filters, smoothers and scratch buffers.
pub struct AidaEngine {
    sample_rate: f64,
    params: ParamSnapshot,

    dc_blocker: Biquad,
    in_lpf: Biquad,
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
    depth: Biquad,
    presence: Biquad,

    in_level: GainRamp,
    out_level: GainRamp,
    bypass_gain: ExponentialValueSmoother,
    cabsim_gain: ExponentialValueSmoother,
    param1: LinearValueSmoother,
    param2: LinearValueSmoother,

    /// Copy of the unprocessed input (`bypassInplaceBuffer`).
    dry: Vec<f32>,

    tmp_meter_in: f32,
    tmp_meter_out: f32,
    tmp_meter_frames: u32,
    meter_max_frame_count: u32,
}

impl AidaEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            sample_rate: 48_000.0,
            params: ParamSnapshot::default(),
            dc_blocker: Biquad::new(),
            in_lpf: Biquad::new(),
            bass: Biquad::new(),
            mid: Biquad::new(),
            treble: Biquad::new(),
            depth: Biquad::new(),
            presence: Biquad::new(),
            in_level: GainRamp::new(),
            out_level: GainRamp::new(),
            bypass_gain: ExponentialValueSmoother::with_time_constant(0.25),
            cabsim_gain: ExponentialValueSmoother::with_time_constant(0.1),
            param1: LinearValueSmoother::with_time_constant(0.1),
            param2: LinearValueSmoother::with_time_constant(0.1),
            dry: Vec::new(),
            tmp_meter_in: 0.0,
            tmp_meter_out: 0.0,
            tmp_meter_frames: 0,
            meter_max_frame_count: 0,
        };
        engine.in_level.set_time_constant(1.0);
        engine.out_level.set_time_constant(1.0);
        let params = engine.params;
        engine.prepare(48_000.0, 0, &params);
        engine
    }

    /// `bufferSizeChanged()` + `sampleRateChanged()`: (re)allocate scratch buffers and
    /// recompute every coefficient. Not real-time safe.
    pub fn prepare(&mut self, sample_rate: f64, max_block_size: usize, params: &ParamSnapshot) {
        self.sample_rate = sample_rate;
        self.dry.resize(max_block_size.max(1), 0.0);
        self.params = *params;
        let p = *params;
        let sr = sample_rate;

        // AidaToneControl::setSampleRate()
        self.dc_blocker
            .set_biquad(BiquadType::HighPass, DC_BLOCKER_FREQ / sr, COMMON_Q, 0.0);
        self.in_lpf.set_biquad(
            BiquadType::LowPass,
            in_lpf_cutoff(p.antialiasing),
            COMMON_Q,
            0.0,
        );
        self.bass.set_biquad(
            BiquadType::LowShelf,
            p.bass_freq as f64 / sr,
            COMMON_Q,
            p.bass_db as f64,
        );
        self.mid.set_biquad(
            mid_filter_type(p.mid_type),
            p.mid_freq as f64 / sr,
            p.mid_q as f64,
            p.mid_db as f64,
        );
        self.treble.set_biquad(
            BiquadType::HighShelf,
            p.treble_freq as f64 / sr,
            COMMON_Q,
            p.treble_db as f64,
        );
        self.depth
            .set_biquad(BiquadType::Peak, DEPTH_FREQ / sr, COMMON_Q, p.depth_db as f64);
        self.presence.set_biquad(
            BiquadType::HighShelf,
            PRESENCE_FREQ / sr,
            COMMON_Q,
            p.presence_db as f64,
        );

        // GainRamp::set_sample_rate() rebuilds its smoother, so set targets afterwards.
        self.in_level.set_sample_rate(sr);
        self.in_level.set_target(p.input_db as f64);
        self.out_level.set_sample_rate(sr);
        self.out_level.set_target(p.output_db as f64);

        self.bypass_gain.set_sample_rate(sr);
        self.bypass_gain
            .set_target_value(if p.bypass { 0.0 } else { 1.0 });
        self.cabsim_gain.set_sample_rate(sr);
        self.cabsim_gain.set_target_value(if p.cabsim_bypass {
            0.0
        } else {
            CABINET_MAX_GAIN
        });
        self.param1.set_sample_rate(sr);
        self.param1.set_target_value(p.param1 as f64);
        self.param2.set_sample_rate(sr);
        self.param2.set_target_value(p.param2 as f64);

        // max 60 fps meter refresh
        self.meter_max_frame_count = (sr * 0.016666) as u32;

        self.reset();
    }

    /// `activate()`: jump all smoothers to their targets, clear filter state and meters.
    pub fn reset(&mut self) {
        self.in_level.clear_to_target();
        self.out_level.clear_to_target();
        self.bypass_gain.clear_to_target_value();
        self.cabsim_gain.clear_to_target_value();
        self.param1.clear_to_target_value();
        self.param2.clear_to_target_value();
        for bq in [
            &mut self.dc_blocker,
            &mut self.in_lpf,
            &mut self.bass,
            &mut self.mid,
            &mut self.treble,
            &mut self.depth,
            &mut self.presence,
        ] {
            bq.reset();
        }
        self.tmp_meter_frames = 0;
        self.tmp_meter_in = 0.0;
        self.tmp_meter_out = 0.0;
        // TODO(model): reset the model's recurrent state and pre-roll 2048 samples of
        // silence through it, as `activate()` does in C++.
    }

    /// `setParameterValue()` for every parameter whose value differs from the last
    /// applied snapshot. Real-time safe (coefficient recalculation only).
    pub fn set_params(&mut self, p: &ParamSnapshot) {
        let old = self.params;
        if *p == old {
            return;
        }
        let sr = self.sample_rate;

        if p.antialiasing != old.antialiasing {
            self.in_lpf.set_cutoff(in_lpf_cutoff(p.antialiasing));
        }
        if p.input_db != old.input_db {
            self.in_level.set_target(p.input_db as f64);
        }
        if p.bass_db != old.bass_db {
            self.bass.set_peak_gain_db(p.bass_db as f64);
        }
        if p.bass_freq != old.bass_freq {
            self.bass.set_cutoff(p.bass_freq as f64 / sr);
        }
        if p.mid_db != old.mid_db {
            self.mid.set_peak_gain_db(p.mid_db as f64);
        }
        if p.mid_freq != old.mid_freq {
            self.mid.set_cutoff(p.mid_freq as f64 / sr);
        }
        if p.mid_q != old.mid_q {
            self.mid.set_q(p.mid_q as f64);
        }
        if p.mid_type != old.mid_type {
            // Deviation from C++: DPF only switched the mid filter *type* in
            // sampleRateChanged(), so toggling MTYPE at runtime kept a peak filter in the
            // bandpass routing until the next sample-rate change. Apply it immediately.
            self.mid.set_type(mid_filter_type(p.mid_type));
        }
        if p.treble_db != old.treble_db {
            self.treble.set_peak_gain_db(p.treble_db as f64);
        }
        if p.treble_freq != old.treble_freq {
            self.treble.set_cutoff(p.treble_freq as f64 / sr);
        }
        if p.depth_db != old.depth_db {
            self.depth.set_peak_gain_db(p.depth_db as f64);
        }
        if p.presence_db != old.presence_db {
            self.presence.set_peak_gain_db(p.presence_db as f64);
        }
        if p.output_db != old.output_db {
            self.out_level.set_target(p.output_db as f64);
        }
        if p.cabsim_bypass != old.cabsim_bypass {
            self.cabsim_gain.set_target_value(if p.cabsim_bypass {
                0.0
            } else {
                CABINET_MAX_GAIN
            });
        }
        if p.bypass != old.bypass {
            self.bypass_gain
                .set_target_value(if p.bypass { 0.0 } else { 1.0 });
        }
        if p.param1 != old.param1 {
            self.param1.set_target_value(p.param1 as f64);
        }
        if p.param2 != old.param2 {
            self.param2.set_target_value(p.param2 as f64);
        }
        // NETBYPASS, EQBYPASS, EQPOS and DCBLOCKER are read from `self.params` in process().
        self.params = *p;
    }

    /// Process one mono block in place. Blocks longer than the prepared maximum are split.
    pub fn process(&mut self, buffer: &mut [f32], outputs: &PluginOutputs) {
        let chunk = self.dry.len();
        for block in buffer.chunks_mut(chunk) {
            self.process_block(block, outputs);
        }
    }

    fn process_block(&mut self, out: &mut [f32], outputs: &PluginOutputs) {
        let n = out.len();
        let p = self.params;
        let dry = &mut self.dry[..n];
        dry.copy_from_slice(out);

        // Peak meters
        let (mut meter_in, mut meter_out);
        if outputs.reset_meters.swap(false, Ordering::Relaxed) {
            let floor = db_to_coeff(MINIMUM_METER_DB) as f32;
            meter_in = floor;
            meter_out = floor;
            self.tmp_meter_in = floor;
            self.tmp_meter_out = floor;
            self.tmp_meter_frames = 0;
        } else {
            meter_in = self.tmp_meter_in;
            meter_out = self.tmp_meter_out;
            self.tmp_meter_frames += n as u32;
        }
        meter_in = dry.iter().fold(meter_in, |m, s| m.max(s.abs()));

        // High-frequency roll-off (anti-aliasing low-pass). Value 0 = "Off".
        if p.antialiasing != 0.0 {
            self.in_lpf.process_buffer(dry, out);
        }

        // Pre-gain
        for s in out.iter_mut() {
            *s *= self.in_level.next() as f32;
        }

        if !p.eq_bypass && p.eq_pos == EqPosition::Pre {
            self.apply_tone_controls(out);
        }

        if !p.net_bypass {
            self.apply_model(out, outputs);
        }

        if p.dc_blocker {
            self.dc_blocker.process_inplace(out);
        }

        self.apply_cabinet(out);

        if !p.eq_bypass && p.eq_pos == EqPosition::Post {
            self.apply_tone_controls(out);
        }

        // Master volume
        for s in out.iter_mut() {
            *s *= self.out_level.next() as f32;
        }

        // Smooth global bypass crossfade + output meter
        let dry = &self.dry[..n];
        for (s, d) in out.iter_mut().zip(dry) {
            let b = self.bypass_gain.next() as f32;
            *s = *s * b + *d * (1.0 - b);
            meter_out = meter_out.max(s.abs());
        }

        if self.tmp_meter_frames >= self.meter_max_frame_count {
            outputs.meter_in.store(meter_in, Ordering::Relaxed);
            outputs.meter_out.store(meter_out, Ordering::Relaxed);
            self.tmp_meter_frames -= self.meter_max_frame_count;
        }
        self.tmp_meter_in = meter_in;
        self.tmp_meter_out = meter_out;
    }

    /// `applyToneControls()`: in BANDPASS mode only the mid filter runs.
    fn apply_tone_controls(&mut self, buf: &mut [f32]) {
        if self.params.mid_type == MidType::Bandpass {
            self.mid.process_inplace(buf);
        } else {
            self.depth.process_inplace(buf);
            self.bass.process_inplace(buf);
            self.mid.process_inplace(buf);
            self.treble.process_inplace(buf);
            self.presence.process_inplace(buf);
        }
    }

    /// Neural model stage (`applyModel()`). **Currently pass-through.**
    ///
    /// TODO(model): once `aidadx_core::model` inference builds (feature `candle`), hold a
    /// loaded model here and run it per sample: apply `input_gain`, feed `[x]`,
    /// `[x, param1.next()]` or `[x, param1.next(), param2.next()]` depending on the
    /// model's input size, honour `input_skip` (residual) and `output_gain`, and publish
    /// the input size to `outputs.model_input_size`. Model swaps must not block or
    /// allocate on the audio thread.
    fn apply_model(&mut self, _buf: &mut [f32], _outputs: &PluginOutputs) {}

    /// Cabinet IR stage. **Currently pass-through.**
    ///
    /// TODO(convolver): once a convolver exists in `aidadx_core`, when an IR is loaded:
    /// copy `buf` to a scratch buffer `wet_in`, convolve `wet_in` into `buf`, then for each
    /// sample `b = cabsim_gain.next(); buf[i] = buf[i] * b + wet_in[i] * ((CABINET_MAX_GAIN - b) / CABINET_MAX_GAIN)`
    /// (smooth CABSIMBYPASS with -12 dB compensation). IRs must be resampled to the host
    /// rate and reloaded on sample-rate change.
    fn apply_cabinet(&mut self, _buf: &mut [f32]) {}
}

impl Default for AidaEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// `MAP(ANTIALIASING, 0, 100, INLPF_MAX_CO, INLPF_MIN_CO)`: normalized LPF cutoff.
fn in_lpf_cutoff(antialiasing: f32) -> f64 {
    map(antialiasing as f64, 0.0, 100.0, INLPF_MAX_CO, INLPF_MIN_CO)
}

fn mid_filter_type(mid_type: MidType) -> BiquadType {
    match mid_type {
        MidType::Peak => BiquadType::Peak,
        MidType::Bandpass => BiquadType::BandPass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;
    const BLOCK: usize = 256;

    /// All coloring stages off and unity gains: the chain must be bit-transparent.
    fn transparent() -> ParamSnapshot {
        ParamSnapshot {
            antialiasing: 0.0,
            eq_bypass: true,
            dc_blocker: false,
            ..ParamSnapshot::default()
        }
    }

    fn engine(p: &ParamSnapshot) -> (AidaEngine, PluginOutputs) {
        let mut e = AidaEngine::new();
        e.prepare(SR, BLOCK, p);
        (e, PluginOutputs::default())
    }

    fn sine(freq: f64, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (0.5 * (2.0 * std::f64::consts::PI * freq * i as f64 / SR).sin()) as f32)
            .collect()
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    #[test]
    fn transparent_settings_pass_signal_unchanged() {
        let (mut e, out) = engine(&transparent());
        let input = sine(440.0, 1000); // not a multiple of BLOCK: exercises chunking
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        assert_eq!(buf, input);
    }

    #[test]
    fn global_bypass_outputs_dry_signal() {
        let p = ParamSnapshot {
            bypass: true,
            bass_db: 8.0,
            input_db: 12.0,
            ..ParamSnapshot::default()
        };
        let (mut e, out) = engine(&p);
        let input = sine(100.0, 4096);
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        assert_eq!(buf, input);
    }

    #[test]
    fn dc_blocker_removes_dc() {
        let p = ParamSnapshot {
            dc_blocker: true,
            ..transparent()
        };
        let (mut e, out) = engine(&p);
        let mut buf = vec![0.5f32; SR as usize];
        e.process(&mut buf, &out);
        assert!(buf.last().unwrap().abs() < 1e-3, "tail = {}", buf.last().unwrap());

        let p = transparent();
        let (mut e, out) = engine(&p);
        let mut buf = vec![0.5f32; 4096];
        e.process(&mut buf, &out);
        assert_eq!(*buf.last().unwrap(), 0.5);
    }

    #[test]
    fn antialiasing_filter_attenuates_highs() {
        let p = ParamSnapshot {
            antialiasing: 100.0,
            ..transparent()
        };
        let (mut e, out) = engine(&p);
        let mut hi = sine(16_000.0, 8192);
        e.process(&mut hi, &out);
        let (mut e, out) = engine(&p);
        let mut lo = sine(200.0, 8192);
        e.process(&mut lo, &out);
        assert!(rms(&hi[4096..]) < 0.5 * rms(&lo[4096..]));
    }

    #[test]
    fn bass_boost_raises_low_frequencies_and_eq_bypass_disables_it() {
        let boosted = ParamSnapshot {
            bass_db: 8.0,
            eq_bypass: false,
            ..transparent()
        };
        let (mut e, out) = engine(&boosted);
        let mut buf = sine(80.0, 16384);
        e.process(&mut buf, &out);
        let gain = rms(&buf[8192..]) / rms(&sine(80.0, 16384)[8192..]);
        assert!(gain > 1.8, "bass gain {gain}");

        let (mut e, out) = engine(&transparent());
        e.set_params(&ParamSnapshot {
            bass_db: 8.0,
            ..transparent()
        });
        let input = sine(80.0, 4096);
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        assert_eq!(buf, input, "EQBYPASS must skip the tone stack");
    }

    #[test]
    fn bandpass_mode_only_runs_mid_filter() {
        let p = ParamSnapshot {
            eq_bypass: false,
            mid_type: MidType::Bandpass,
            bass_db: 8.0,
            treble_db: 8.0,
            ..transparent()
        };
        let (mut e, out) = engine(&p);
        let mut low = sine(60.0, 16384);
        e.process(&mut low, &out);
        // A bandpass around 750 Hz strongly attenuates 60 Hz despite the +8 dB bass.
        assert!(rms(&low[8192..]) < 0.25 * rms(&sine(60.0, 16384)[8192..]));
    }

    #[test]
    fn meters_track_peaks_and_reset() {
        let p = transparent();
        let (mut e, out) = engine(&p);
        let mut buf = vec![0.0f32; 2048];
        buf[100] = 0.75;
        e.process(&mut buf, &out);
        assert_eq!(out.meter_in.load(Ordering::Relaxed), 0.75);
        assert_eq!(out.meter_out.load(Ordering::Relaxed), 0.75);

        out.reset_meters.store(true, Ordering::Relaxed);
        let mut silent = vec![0.0f32; 2048];
        e.process(&mut silent, &out);
        let floor = db_to_coeff(MINIMUM_METER_DB) as f32;
        assert_eq!(out.meter_in.load(Ordering::Relaxed), floor);
    }
}
