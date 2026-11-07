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
//! The neural model ([`DynamicModel`], RTNeural backend) and the cabinet convolver
//! ([`TwoStageConvolver`]) are built off the audio thread (see [`crate::loader`]) and handed
//! over through lock-free [`Handoff`] slots ([`AidaEngine::model_slot`],
//! [`AidaEngine::cabinet_slot`]). [`AidaEngine::process`] never allocates or frees.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use aidadx_core::biquad::{
    db_to_coeff, map, Biquad, BiquadType, ExponentialValueSmoother, GainRamp, LinearValueSmoother,
    ValueSmoother,
};
use aidadx_core::convolver::TwoStageConvolver;
use aidadx_core::model::DynamicModel;
use nih_plug::prelude::AtomicF32;

use crate::handoff::Handoff;
use crate::parameters::{EqPosition, MidType, ParamSnapshot};

// The C++ constants are `float`s handed to `double` APIs, so they are rounded through
// `f32` here, exactly like `aidadx_core::biquad` does internally (its copies are private).

/// Q used by all fixed-Q tone filters (`COMMON_Q = 0.707f`).
const COMMON_Q: f64 = 0.707_f32 as f64;
/// Fixed centre frequency of the DEPTH peak filter (`DEPTH_FREQ`).
const DEPTH_FREQ: f64 = 75.0;
/// Fixed corner frequency of the PRESENCE high shelf (`PRESENCE_FREQ`).
const PRESENCE_FREQ: f64 = 900.0;
/// DC blocker high-pass corner, in Hz (`35.0f`).
const DC_BLOCKER_FREQ: f64 = 35.0;
/// Anti-aliasing LPF normalized cutoff at ANTIALIASING = 0 % (`0.99f * 0.5f`).
const INLPF_MAX_CO: f64 = (0.99_f32 * 0.5) as f64;
/// Anti-aliasing LPF normalized cutoff at ANTIALIASING = 100 % (`0.25f * 0.5f`).
const INLPF_MIN_CO: f64 = (0.25_f32 * 0.5) as f64;
/// Gain compensation for the cabinet IR, -12 dB (`kCabinetMaxGain = 0.251f`), as used in
/// the per-sample `float` crossfade.
pub const CABINET_MAX_GAIN_F32: f32 = 0.251;
/// [`CABINET_MAX_GAIN_F32`] widened for the `f64` smoother API.
pub const CABINET_MAX_GAIN: f64 = CABINET_MAX_GAIN_F32 as f64;

/// Lock-free slot the loader publishes new (reset + pre-warmed) models into.
pub type ModelSlot = Handoff<DynamicModel>;
/// Lock-free slot the loader publishes new cabinet convolvers into.
pub type CabinetSlot = Handoff<TwoStageConvolver>;
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
    /// Convolver input copy (`cabsimInplaceBuffer`).
    cab_in: Vec<f32>,

    /// Active neural model (`model`), owned by the audio thread.
    model: Option<Box<DynamicModel>>,
    model_slot: Arc<ModelSlot>,
    /// `paramFirstRun`: jump PARAM1/PARAM2 smoothers to their targets on the next block.
    param_first_run: bool,
    /// Active cabinet convolver (`cabsim`), owned by the audio thread.
    cabinet: Option<Box<TwoStageConvolver>>,
    cabinet_slot: Arc<CabinetSlot>,

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
            cab_in: Vec::new(),
            model: None,
            model_slot: Arc::new(ModelSlot::new()),
            param_first_run: true,
            cabinet: None,
            cabinet_slot: Arc::new(CabinetSlot::new()),
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
        self.cab_in.resize(max_block_size.max(1), 0.0);
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

    /// Slot through which a loader hands new models to this engine.
    pub fn model_slot(&self) -> Arc<ModelSlot> {
        self.model_slot.clone()
    }

    /// Slot through which a loader hands new cabinet convolvers to this engine.
    pub fn cabinet_slot(&self) -> Arc<CabinetSlot> {
        self.cabinet_slot.clone()
    }

    /// The active model, if any.
    pub fn model(&self) -> Option<&DynamicModel> {
        self.model.as_deref()
    }

    /// The active cabinet convolver, if any.
    pub fn cabinet(&self) -> Option<&TwoStageConvolver> {
        self.cabinet.as_deref()
    }

    /// Install models/convolvers published by the loader. Real-time safe: the replaced
    /// objects are parked for the loader thread to free (see [`Handoff`]).
    fn take_pending(&mut self, outputs: &PluginOutputs) {
        if self.model_slot.take(&mut self.model) {
            // The loader already reset and pre-warmed the model with the PARAM1/PARAM2
            // targets (`loadModelFromStream`); jump the smoothers like C++ does.
            self.param_first_run = true;
        }
        self.cabinet_slot.take(&mut self.cabinet);
        // `kParameterModelInputSize`: 0 ("ERROR") until a model is loaded.
        let size = self.model.as_ref().map_or(0, |m| m.input_size() as u32);
        outputs.model_input_size.store(size, Ordering::Relaxed);
    }

    /// `activate()`: jump all smoothers to their targets, clear filter state and meters,
    /// reset the model and pre-roll 2048 samples of silence through it.
    /// Real-time safe (nih-plug may call it from the audio thread).
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
        if let Some(model) = self.model.as_deref_mut() {
            // `prewarm` = reset + 2048 zeros (stack buffer). It only fails on the candle
            // backend, where a skipped pre-roll is harmless.
            let _ = model.prewarm(
                self.param1.target_value() as f32,
                self.param2.target_value() as f32,
            );
        }
        self.param_first_run = false;
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
        self.take_pending(outputs);
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
            self.apply_model(out);
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

    /// Neural model stage (`applyModel()`). Input gain, `input_skip` and output gain are
    /// handled by [`DynamicModel::process`]. PARAM1/PARAM2 are fed per sample to 2- and
    /// 3-input models, and each smoother only advances if the model consumes it, as in
    /// C++. While the consumed smoothers are settled the whole block runs with constant
    /// conditioning, which is identical to the per-sample loop but avoids per-sample FFI.
    fn apply_model(&mut self, buf: &mut [f32]) {
        let Some(model) = self.model.as_deref_mut() else {
            return;
        };
        if self.param_first_run {
            self.param_first_run = false;
            self.param1.clear_to_target_value();
            self.param2.clear_to_target_value();
        }
        let input_size = model.input_size();
        let settled = |s: &LinearValueSmoother| s.current_value() == s.target_value();
        let ramping = (input_size >= 2 && !settled(&self.param1))
            || (input_size >= 3 && !settled(&self.param2));
        // `process` only fails on the candle backend (never with RTNeural).
        if !ramping {
            let p1 = self.param1.target_value() as f32;
            let p2 = self.param2.target_value() as f32;
            let _ = model.process(buf, p1, p2);
        } else {
            for i in 0..buf.len() {
                let p1 = self.param1.next_f32();
                let p2 = if input_size >= 3 {
                    self.param2.next_f32()
                } else {
                    0.0
                };
                let _ = model.process(&mut buf[i..i + 1], p1, p2);
            }
        }
    }

    /// Cabinet IR stage: convolve, then the smooth CABSIMBYPASS crossfade with -12 dB
    /// compensation, `out = wet * b + dry * ((kCabinetMaxGain - b) / kCabinetMaxGain)`
    /// (`b` = 0.251 when enabled, 0 when bypassed). Skipped entirely (smoother not
    /// advanced) while no IR is loaded, like C++.
    fn apply_cabinet(&mut self, buf: &mut [f32]) {
        let Some(cabinet) = self.cabinet.as_deref_mut() else {
            return;
        };
        let dry = &mut self.cab_in[..buf.len()];
        dry.copy_from_slice(buf);
        cabinet.process(dry, buf);
        for (s, d) in buf.iter_mut().zip(dry.iter()) {
            let b = self.cabsim_gain.next_f32();
            *s = *s * b + *d * ((CABINET_MAX_GAIN_F32 - b) / CABINET_MAX_GAIN_F32);
        }
    }
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

    // ---- model and cabinet stages (TICKET-017) ----

    use crate::loader::{load_cabinet, load_model, FileSource};
    use crate::test_models::lstm_json;

    fn default_model() -> DynamicModel {
        let mut m = load_model(&FileSource::Default).unwrap();
        m.prewarm(0.0, 0.0).unwrap();
        m
    }

    fn noise(len: usize) -> Vec<f32> {
        let mut x = 1u32;
        (0..len)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect()
    }

    #[test]
    fn no_model_reports_error_input_size() {
        let (mut e, out) = engine(&transparent());
        out.model_input_size.store(99, Ordering::Relaxed);
        e.process(&mut [0.0; 64], &out);
        assert_eq!(out.model_input_size.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn model_stage_matches_dynamic_model_and_net_bypass_skips_it() {
        let input = sine(220.0, 3000);

        let (mut e, out) = engine(&transparent());
        e.model_slot().publish(Box::new(default_model()));
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        assert_eq!(out.model_input_size.load(Ordering::Relaxed), 1);
        assert!(e.model().is_some());

        let mut expected = input.clone();
        let mut reference = default_model();
        for chunk in expected.chunks_mut(BLOCK) {
            reference.process(chunk, 0.0, 0.0).unwrap();
        }
        assert_eq!(buf, expected, "engine must run the model exactly like applyModel");
        assert!(rms(&buf) > 0.01 && buf != input, "model changes the signal");

        let p = ParamSnapshot {
            net_bypass: true,
            ..transparent()
        };
        e.set_params(&p);
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        assert_eq!(buf, input, "NETBYPASS skips the model");
        assert_eq!(out.model_input_size.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn conditioned_model_follows_param1_and_param2() {
        let json = lstm_json(3, 8, 7);
        let run = |p1: f32, p2: f32| {
            let p = ParamSnapshot {
                param1: p1,
                param2: p2,
                ..transparent()
            };
            let (mut e, out) = engine(&p);
            let mut m = DynamicModel::from_json(&json).unwrap();
            m.prewarm(p1, p2).unwrap();
            e.model_slot().publish(Box::new(m));
            let mut buf = sine(330.0, 2048);
            e.process(&mut buf, &out);
            assert_eq!(out.model_input_size.load(Ordering::Relaxed), 3);
            buf
        };
        let base = run(0.0, 0.0);
        assert_ne!(base, run(1.0, 0.0), "PARAM1 conditions the model");
        assert_ne!(base, run(0.0, 1.0), "PARAM2 conditions the model");

        // Settled smoothers: whole-block processing equals a direct call.
        let mut m = DynamicModel::from_json(&json).unwrap();
        m.prewarm(0.5, 0.25).unwrap();
        let mut expected = sine(330.0, 2048);
        for chunk in expected.chunks_mut(BLOCK) {
            m.process(chunk, 0.5, 0.25).unwrap();
        }
        assert_eq!(run(0.5, 0.25), expected);
    }

    #[test]
    fn param_change_ramps_per_sample() {
        let json = lstm_json(2, 8, 3);
        let (mut e, out) = engine(&transparent());
        let mut m = DynamicModel::from_json(&json).unwrap();
        m.prewarm(0.0, 0.0).unwrap();
        e.model_slot().publish(Box::new(m));
        let mut warm = sine(100.0, 4096);
        e.process(&mut warm, &out);

        // Jump PARAM1 0 -> 1: the 100 ms linear ramp must reach the model sample by sample.
        e.set_params(&ParamSnapshot {
            param1: 1.0,
            ..transparent()
        });
        let mut ramp = vec![0.0f32; 512];
        e.process(&mut ramp, &out);
        assert!(e.param1.current_value() > 0.0 && e.param1.current_value() < 1.0);

        // Reference: per-sample conditioning with the same smoother.
        let mut m = DynamicModel::from_json(&json).unwrap();
        m.prewarm(0.0, 0.0).unwrap();
        let mut w = sine(100.0, 4096);
        m.process(&mut w, 0.0, 0.0).unwrap();
        let mut s = LinearValueSmoother::with_time_constant(0.1);
        s.set_sample_rate(SR);
        s.set_target_value(1.0);
        let mut expected = vec![0.0f32; 512];
        for x in expected.iter_mut() {
            m.process(std::slice::from_mut(x), s.next_f32(), 0.0).unwrap();
        }
        assert_eq!(ramp, expected);
    }

    #[test]
    fn reset_prewarms_the_model() {
        let (mut e, out) = engine(&transparent());
        e.model_slot().publish(Box::new(default_model()));
        let mut buf = noise(4096);
        e.process(&mut buf, &out);
        e.reset();
        let mut silence = vec![0.0f32; 256];
        e.process(&mut silence, &out);

        let mut expected = vec![0.0f32; 256];
        default_model().process(&mut expected, 0.0, 0.0).unwrap();
        assert_eq!(silence, expected, "reset + 2048-sample pre-roll like activate()");
    }

    #[test]
    fn cabinet_stage_applies_ir_with_compensated_crossfade() {
        let input = noise(SR as usize);

        // CABSIMBYPASS on from the start: b = 0 -> exactly the dry signal.
        let bypassed = ParamSnapshot {
            cabsim_bypass: true,
            ..transparent()
        };
        let (mut e, out) = engine(&bypassed);
        e.cabinet_slot()
            .publish(Box::new(load_cabinet(&FileSource::Default, SR).unwrap()));
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        assert!(e.cabinet().is_some());
        assert_eq!(buf, input, "bypassed cabinet is bit-transparent");

        // Enabled: out = conv(x) * 0.251.
        let (mut e, out) = engine(&transparent());
        e.cabinet_slot()
            .publish(Box::new(load_cabinet(&FileSource::Default, SR).unwrap()));
        let mut buf = input.clone();
        e.process(&mut buf, &out);
        let mut conv = load_cabinet(&FileSource::Default, SR).unwrap();
        let mut wet = vec![0.0f32; input.len()];
        for (i, o) in input.chunks(BLOCK).zip(wet.chunks_mut(BLOCK)) {
            conv.process(i, o);
        }
        for (a, w) in buf.iter().zip(&wet) {
            assert!((a - w * CABINET_MAX_GAIN_F32).abs() < 1e-6, "{a} vs {w}");
        }

        // Toggle CABSIMBYPASS on a slow sine: smooth crossfade, no step.
        let low = sine(50.0, SR as usize);
        let (mut e, out) = engine(&transparent());
        e.cabinet_slot()
            .publish(Box::new(load_cabinet(&FileSource::Default, SR).unwrap()));
        let mut a = low[..24_000].to_vec();
        e.process(&mut a, &out);
        e.set_params(&bypassed);
        let mut b = low[24_000..].to_vec();
        e.process(&mut b, &out);
        let joined: Vec<f32> = a.iter().chain(&b).copied().collect();
        let max_step = joined
            .windows(2)
            .skip(4096)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_step < 0.01, "crossfade step {max_step}");
        // ...and it ends up at the dry signal (exponential, T60 = 100 ms).
        let tail = &b[b.len() - 100..];
        let dry = &low[low.len() - 100..];
        for (x, d) in tail.iter().zip(dry) {
            assert!((x - d).abs() < 1e-4);
        }
    }
}
