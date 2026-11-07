//! Drives the exported `clap_entry` through the raw CLAP ABI, the way hosts and
//! clap-validator do (TICKET-021): factory/descriptor, state save/load (including empty and
//! garbage state), parameter value <-> text round-trips, processing with 0-length and
//! varying block sizes across sample rates, parameter fuzzing, stereo config, and multiple
//! concurrent instances.
//!
//! This goes through nih-plug's real CLAP wrapper, so it also covers the glue in `lib.rs`.

use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::sync::Once;

use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_param_value, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::audio_ports_config::{
    clap_plugin_audio_ports_config, CLAP_EXT_AUDIO_PORTS_CONFIG,
};
use clap_sys::ext::params::{
    clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_STEPPED,
};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::id::clap_id;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::{clap_process, CLAP_PROCESS_ERROR};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::version::CLAP_VERSION;

use aidadx_plugin::parameters::PARAMETER_SYMBOLS;
use aidadx_plugin::plugin_info::CLAP_ID;
use aidadx_plugin::{AidaEngine, ParamSnapshot, PluginOutputs};

/// DPF CLAP parameter IDs (= DPF index, see `parameters.rs`).
const ID_ANTIALIASING: clap_id = 0;
const ID_PREGAIN: clap_id = 1;
const ID_MFREQ: clap_id = 8;
const ID_BYPASS: clap_id = 17;

// ---------------------------------------------------------------------------------------
// Minimal host
// ---------------------------------------------------------------------------------------

unsafe extern "C" fn host_get_extension(_: *const clap_host, _: *const c_char) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn host_noop(_: *const clap_host) {}

struct StaticHost(clap_host);
// SAFETY: only immutable pointers to 'static data.
unsafe impl Sync for StaticHost {}

static HOST: StaticHost = StaticHost(clap_host {
    clap_version: CLAP_VERSION,
    host_data: ptr::null_mut(),
    name: b"aidadx host_behaviour test\0".as_ptr() as *const c_char,
    vendor: b"AIDA-X\0".as_ptr() as *const c_char,
    url: b"\0".as_ptr() as *const c_char,
    version: b"0\0".as_ptr() as *const c_char,
    get_extension: Some(host_get_extension),
    request_restart: Some(host_noop),
    request_process: Some(host_noop),
    request_callback: Some(host_noop),
});

fn entry() -> &'static clap_plugin_entry {
    static INIT: Once = Once::new();
    let entry = &aidadx_plugin::clap_entry;
    INIT.call_once(|| unsafe {
        assert!((entry.init.unwrap())(b"\0".as_ptr() as *const c_char));
    });
    entry
}

fn factory() -> &'static clap_plugin_factory {
    let entry = entry();
    let factory = unsafe { (entry.get_factory.unwrap())(CLAP_PLUGIN_FACTORY_ID.as_ptr()) };
    assert!(!factory.is_null());
    unsafe { &*(factory as *const clap_plugin_factory) }
}

/// Input events: a list of parameter value events.
struct Events(Vec<clap_event_param_value>);

unsafe extern "C" fn events_size(list: *const clap_input_events) -> u32 {
    (*((*list).ctx as *const Events)).0.len() as u32
}
unsafe extern "C" fn events_get(
    list: *const clap_input_events,
    i: u32,
) -> *const clap_event_header {
    let events = &*((*list).ctx as *const Events);
    events
        .0
        .get(i as usize)
        .map_or(ptr::null(), |e| &e.header as *const _)
}
unsafe extern "C" fn out_try_push(
    _: *const clap_output_events,
    _: *const clap_event_header,
) -> bool {
    true
}

impl Events {
    fn param(values: &[(clap_id, f64)]) -> Self {
        Self(
            values
                .iter()
                .map(|&(param_id, value)| clap_event_param_value {
                    header: clap_event_header {
                        size: std::mem::size_of::<clap_event_param_value>() as u32,
                        time: 0,
                        space_id: CLAP_CORE_EVENT_SPACE_ID,
                        type_: CLAP_EVENT_PARAM_VALUE,
                        flags: 0,
                    },
                    param_id,
                    cookie: ptr::null_mut(),
                    note_id: -1,
                    port_index: -1,
                    channel: -1,
                    key: -1,
                    value,
                })
                .collect(),
        )
    }

    fn as_input(&self) -> clap_input_events {
        clap_input_events {
            ctx: self as *const Self as *mut c_void,
            size: Some(events_size),
            get: Some(events_get),
        }
    }
}

fn output_events() -> clap_output_events {
    clap_output_events {
        ctx: ptr::null_mut(),
        try_push: Some(out_try_push),
    }
}

unsafe extern "C" fn ostream_write(s: *const clap_ostream, buf: *const c_void, size: u64) -> i64 {
    let out = &mut *((*s).ctx as *mut Vec<u8>);
    out.extend_from_slice(std::slice::from_raw_parts(buf as *const u8, size as usize));
    size as i64
}

/// Reads from a byte slice, at most `chunk` bytes per call (hosts may use small reads).
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    chunk: usize,
}

unsafe extern "C" fn istream_read(s: *const clap_istream, buf: *mut c_void, size: u64) -> i64 {
    let r = &mut *((*s).ctx as *mut Reader);
    let n = (r.data.len() - r.pos).min(size as usize).min(r.chunk);
    ptr::copy_nonoverlapping(r.data.as_ptr().add(r.pos), buf as *mut u8, n);
    r.pos += n;
    n as i64
}

/// One plugin instance, created through the factory and destroyed on drop.
struct Instance {
    plugin: *const clap_plugin,
    params: &'static clap_plugin_params,
    state: &'static clap_plugin_state,
    active: bool,
    processing: bool,
}

impl Instance {
    fn new() -> Self {
        let factory = factory();
        let id = CString::new(CLAP_ID).unwrap();
        let plugin = unsafe { (factory.create_plugin.unwrap())(factory, &HOST.0, id.as_ptr()) };
        assert!(!plugin.is_null(), "create_plugin");
        unsafe {
            assert!(((*plugin).init.unwrap())(plugin), "init");
        }
        let params = unsafe { Self::ext::<clap_plugin_params>(plugin, CLAP_EXT_PARAMS) };
        let state = unsafe { Self::ext::<clap_plugin_state>(plugin, CLAP_EXT_STATE) };
        Self {
            plugin,
            params,
            state,
            active: false,
            processing: false,
        }
    }

    unsafe fn ext<T>(plugin: *const clap_plugin, id: &CStr) -> &'static T {
        let ext = ((*plugin).get_extension.unwrap())(plugin, id.as_ptr());
        assert!(!ext.is_null(), "extension {id:?}");
        &*(ext as *const T)
    }

    fn activate(&mut self, sample_rate: f64, min: u32, max: u32) {
        self.deactivate();
        unsafe {
            assert!(((*self.plugin).activate.unwrap())(
                self.plugin,
                sample_rate,
                min,
                max
            ));
            self.active = true;
            assert!(((*self.plugin).start_processing.unwrap())(self.plugin));
            self.processing = true;
        }
    }

    fn deactivate(&mut self) {
        unsafe {
            if self.processing {
                ((*self.plugin).stop_processing.unwrap())(self.plugin);
                self.processing = false;
            }
            if self.active {
                ((*self.plugin).deactivate.unwrap())(self.plugin);
                self.active = false;
            }
        }
    }

    fn reset(&self) {
        unsafe { ((*self.plugin).reset.unwrap())(self.plugin) }
    }

    /// Process `channels` x `frames` of `input` out of place; returns the output channels.
    fn process(&self, input: &[Vec<f32>], frames: usize, events: &Events) -> Vec<Vec<f32>> {
        let mut inputs: Vec<Vec<f32>> = input.iter().map(|c| c[..frames].to_vec()).collect();
        // Fill outputs with garbage: the plugin must overwrite every sample.
        let mut outputs: Vec<Vec<f32>> = vec![vec![f32::NAN; frames.max(1)]; input.len()];
        let mut in_ptrs: Vec<*mut f32> = inputs.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let mut out_ptrs: Vec<*mut f32> = outputs.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let in_buf = clap_audio_buffer {
            data32: in_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: input.len() as u32,
            latency: 0,
            constant_mask: 0,
        };
        let mut out_buf = clap_audio_buffer {
            data32: out_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: input.len() as u32,
            latency: 0,
            constant_mask: 0,
        };
        let in_events = events.as_input();
        let out_events = output_events();
        let process = clap_process {
            steady_time: -1,
            frames_count: frames as u32,
            transport: ptr::null(),
            audio_inputs: &in_buf,
            audio_outputs: &mut out_buf,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: &in_events,
            out_events: &out_events,
        };
        let status = unsafe { ((*self.plugin).process.unwrap())(self.plugin, &process) };
        assert_ne!(status, CLAP_PROCESS_ERROR, "process({frames})");
        for c in &mut outputs {
            c.truncate(frames);
        }
        outputs
    }

    fn flush(&self, events: &Events) {
        let in_events = events.as_input();
        let out_events = output_events();
        unsafe { (self.params.flush.unwrap())(self.plugin, &in_events, &out_events) }
    }

    fn param_count(&self) -> u32 {
        unsafe { (self.params.count.unwrap())(self.plugin) }
    }

    fn param_info(&self, index: u32) -> clap_param_info {
        let mut info: clap_param_info = unsafe { std::mem::zeroed() };
        assert!(unsafe { (self.params.get_info.unwrap())(self.plugin, index, &mut info) });
        info
    }

    fn value(&self, id: clap_id) -> f64 {
        let mut v = f64::NAN;
        assert!(unsafe { (self.params.get_value.unwrap())(self.plugin, id, &mut v) });
        v
    }

    fn value_to_text(&self, id: clap_id, value: f64) -> Option<String> {
        let mut buf = [0 as c_char; 256];
        let ok = unsafe {
            (self.params.value_to_text.unwrap())(
                self.plugin,
                id,
                value,
                buf.as_mut_ptr(),
                buf.len() as u32,
            )
        };
        ok.then(|| {
            unsafe { CStr::from_ptr(buf.as_ptr()) }
                .to_str()
                .unwrap()
                .to_owned()
        })
    }

    fn text_to_value(&self, id: clap_id, text: &str) -> Option<f64> {
        let text = CString::new(text).unwrap();
        let mut v = f64::NAN;
        let ok =
            unsafe { (self.params.text_to_value.unwrap())(self.plugin, id, text.as_ptr(), &mut v) };
        ok.then_some(v)
    }

    fn save(&self) -> Vec<u8> {
        let mut data = Vec::new();
        let stream = clap_ostream {
            ctx: &mut data as *mut Vec<u8> as *mut c_void,
            write: Some(ostream_write),
        };
        assert!(unsafe { (self.state.save.unwrap())(self.plugin, &stream) });
        data
    }

    fn load(&self, data: &[u8], chunk: usize) -> bool {
        let mut reader = Reader {
            data,
            pos: 0,
            chunk,
        };
        let stream = clap_istream {
            ctx: &mut reader as *mut Reader as *mut c_void,
            read: Some(istream_read),
        };
        unsafe { (self.state.load.unwrap())(self.plugin, &stream) }
    }

    fn select_audio_ports_config(&self, id: clap_id) -> bool {
        unsafe {
            let ext = Self::ext::<clap_plugin_audio_ports_config>(
                self.plugin,
                CLAP_EXT_AUDIO_PORTS_CONFIG,
            );
            (ext.select.unwrap())(self.plugin, id)
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.deactivate();
        unsafe { ((*self.plugin).destroy.unwrap())(self.plugin) }
    }
}

/// Deterministic noise in [-1, 1).
struct Lcg(u64);
impl Lcg {
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn noise(&mut self, n: usize) -> Vec<f32> {
        (0..n)
            .map(|_| (self.next_f64() * 2.0 - 1.0) as f32)
            .collect()
    }
}

fn assert_finite(outputs: &[Vec<f32>], what: &str) {
    for (c, channel) in outputs.iter().enumerate() {
        for (i, s) in channel.iter().enumerate() {
            assert!(s.is_finite(), "{what}: channel {c} sample {i} = {s}");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------

#[test]
fn factory_exposes_one_plugin_with_expected_id() {
    let factory = factory();
    assert_eq!(unsafe { (factory.get_plugin_count.unwrap())(factory) }, 1);
    let desc = unsafe { (factory.get_plugin_descriptor.unwrap())(factory, 0) };
    assert!(!desc.is_null());
    let id = unsafe { CStr::from_ptr((*desc).id) };
    assert_eq!(id.to_str().unwrap(), CLAP_ID);
    // Unknown IDs must be rejected, not crash.
    let bogus = unsafe {
        (factory.create_plugin.unwrap())(factory, &HOST.0, b"not.a.plugin\0".as_ptr() as _)
    };
    assert!(bogus.is_null());
}

#[test]
fn param_ids_are_dpf_indices() {
    let plugin = Instance::new();
    assert_eq!(plugin.param_count() as usize, PARAMETER_SYMBOLS.len());
    for index in 0..plugin.param_count() {
        let info = plugin.param_info(index);
        assert_eq!(info.id, index, "CLAP id of parameter {index}");
        assert!(info.min_value <= info.default_value && info.default_value <= info.max_value);
    }
}

/// clap-validator `param-conversions`: value -> text -> value -> text must be stable for
/// every parameter, and text -> value must succeed for every text the plugin produced.
#[test]
fn param_value_text_round_trips() {
    let plugin = Instance::new();
    for index in 0..plugin.param_count() {
        let info = plugin.param_info(index);
        let stepped = info.flags & CLAP_PARAM_IS_STEPPED != 0;
        let (min, max) = (info.min_value, info.max_value);
        let mut values = vec![min, max, info.default_value];
        if stepped {
            values.extend((min as i64..=max as i64).map(|v| v as f64));
        } else {
            values.extend(
                [1e-7, 1e-4, 0.001, 0.25, 0.3333, 0.5, 0.75, 0.9999].map(|t| min + t * (max - min)),
            );
        }
        for value in values {
            let text = plugin
                .value_to_text(info.id, value)
                .unwrap_or_else(|| panic!("param {index}: value_to_text({value})"));
            let parsed = plugin
                .text_to_value(info.id, &text)
                .unwrap_or_else(|| panic!("param {index}: text_to_value({text:?}) for {value}"));
            assert!(
                (min..=max).contains(&parsed),
                "param {index}: {text:?} -> {parsed} out of range"
            );
            let again = plugin.value_to_text(info.id, parsed).unwrap();
            assert_eq!(
                text, again,
                "param {index}: {value} -> {text:?} -> {parsed}"
            );
        }
        // Garbage text is rejected rather than mapped to some value.
        if !stepped {
            assert_eq!(plugin.text_to_value(info.id, "not a number"), None);
        }
    }
    // Unknown IDs.
    assert_eq!(plugin.value_to_text(9999, 0.0), None);
    assert_eq!(plugin.text_to_value(9999, "0"), None);
}

#[test]
fn antialiasing_off_label_round_trips() {
    let plugin = Instance::new();
    let off = plugin.value_to_text(ID_ANTIALIASING, 0.0).unwrap();
    assert!(off.starts_with("Off"), "{off}");
    for text in [off.as_str(), "Off", "off", "OFF %", " Off "] {
        assert_eq!(
            plugin.text_to_value(ID_ANTIALIASING, text),
            Some(0.0),
            "{text:?}"
        );
    }
    // A tiny non-zero value keeps the filter active, so it must not read as "Off"/"0.0".
    let tiny = plugin.value_to_text(ID_ANTIALIASING, 1e-6).unwrap();
    assert!(
        !tiny.starts_with("Off") && !tiny.starts_with("0.0"),
        "{tiny}"
    );
    assert_eq!(
        plugin.value_to_text(ID_ANTIALIASING, 1.0).unwrap(),
        "100.0 %"
    );
    assert_eq!(
        plugin
            .text_to_value(ID_ANTIALIASING, "50 %")
            .map(|v| (v * 1e6).round()),
        Some(0.5e6)
    );
}

#[test]
fn bypass_labels_round_trip() {
    let plugin = Instance::new();
    let info = plugin.param_info(ID_BYPASS);
    assert_eq!(
        plugin.value_to_text(ID_BYPASS, info.min_value).unwrap(),
        "PROCESSING"
    );
    assert_eq!(
        plugin.value_to_text(ID_BYPASS, info.max_value).unwrap(),
        "BYPASSED"
    );
    assert_eq!(
        plugin.text_to_value(ID_BYPASS, "bypassed"),
        Some(info.max_value)
    );
}

/// clap-validator `state-reproducibility-*`: save, load into a fresh instance, save again
/// -> identical bytes and parameter values, also with small buffered stream reads.
#[test]
fn state_round_trip_is_reproducible() {
    let a = Instance::new();
    a.flush(&Events::param(&[
        (ID_PREGAIN, 0.8),
        (ID_MFREQ, 0.1),
        (ID_BYPASS, 1.0),
    ]));
    let saved = a.save();
    assert!(!saved.is_empty());

    for chunk in [usize::MAX, 7, 1] {
        let b = Instance::new();
        assert!(b.load(&saved, chunk), "load (chunk {chunk})");
        for index in 0..a.param_count() {
            let id = a.param_info(index).id;
            assert!(
                (a.value(id) - b.value(id)).abs() < 1e-6,
                "param {index} after load"
            );
        }
        assert_eq!(b.save(), saved, "re-saved state differs (chunk {chunk})");
    }

    // Loading the default state of a fresh instance restores the defaults.
    let fresh = Instance::new().save();
    assert!(a.load(&fresh, usize::MAX));
    assert_eq!(a.save(), fresh);
}

/// clap-validator `state-invalid` and fuzzed state: empty, truncated and garbage state must
/// be rejected (or ignored) without crashing, and the instance must stay usable.
#[test]
fn empty_and_garbage_state_is_rejected() {
    let mut plugin = Instance::new();
    plugin.flush(&Events::param(&[(ID_PREGAIN, 0.25)]));
    let before = plugin.save();

    let with_len = |body: &[u8], len: u64| {
        let mut v = len.to_le_bytes().to_vec();
        v.extend_from_slice(body);
        v
    };
    let truncated = &before[..before.len() / 2];
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", Vec::new()),
        ("short length prefix", vec![1, 2, 3]),
        ("length only", with_len(b"", 0)),
        ("truncated", truncated.to_vec()),
        ("not json", with_len(b"\xff\xfegarbage", 9)),
        ("json array", with_len(b"[1,2,3]", 7)),
        ("json number", with_len(b"42", 2)),
        // NOTE: a length prefix far beyond the data (e.g. u64::MAX) makes nih-plug's CLAP
        // wrapper call `Vec::with_capacity(len)` and abort; that is an upstream issue
        // (src/wrapper/clap/wrapper.rs `ext_state_load`) and not exercised here.
        ("length beyond data", with_len(b"{}", 64)),
    ];
    for (what, data) in &cases {
        assert!(!plugin.load(data, usize::MAX), "{what} accepted");
        assert_eq!(plugin.save(), before, "{what} changed the state");
    }

    // Well-formed JSON with unknown/mistyped content: may be accepted, must not crash, and
    // unknown keys must not change known parameters.
    let odd: [&[u8]; 4] = [
        br#"{"params":{},"fields":{}}"#,
        br#"{"params":{"nope":1.0,"PREGAIN#08NL4:N":"x"},"fields":{"json":"not json","cabinet":"\"\u0000\""}}"#,
        br#"{"params":{"PREGAIN#08NL4:N":1e30},"fields":{"json":"\"/nonexistent/m.json\"","cabinet":"\"\""}}"#,
        br#"{"version":"999","params":{"dpf_bypass":true}}"#,
    ];
    for body in odd {
        let _ = plugin.load(&with_len(body, body.len() as u64), usize::MAX);
        for index in 0..plugin.param_count() {
            let info = plugin.param_info(index);
            let v = plugin.value(info.id);
            assert!(
                v.is_finite() && (info.min_value..=info.max_value).contains(&v),
                "param {index} = {v} after {}",
                String::from_utf8_lossy(body)
            );
        }
    }

    // Still works afterwards.
    plugin.activate(48_000.0, 1, 256);
    let out = plugin.process(&[Lcg(1).noise(256)], 256, &Events(Vec::new()));
    assert_finite(&out, "after garbage state");
    assert!(plugin.load(&before, usize::MAX));
}

/// Activation with different sample rates and max block sizes; 0-length, 1-sample and
/// max-length blocks; reset in between; outputs fully written and finite.
#[test]
fn process_with_varying_block_sizes_and_sample_rates() {
    let mut plugin = Instance::new();
    let mut rng = Lcg(7);
    for (sr, max) in [
        (44_100.0, 32),
        (48_000.0, 1),
        (96_000.0, 4096),
        (22_050.0, 512),
        (192_000.0, 8192),
        (44_100.0, 64),
    ] {
        plugin.activate(sr, 1, max);
        for frames in [
            0usize,
            1,
            max as usize / 2,
            max as usize,
            0,
            3,
            max as usize,
        ] {
            let input = rng.noise(frames.max(1));
            let out = plugin.process(&[input], frames, &Events(Vec::new()));
            assert_eq!(out[0].len(), frames);
            assert_finite(&out, &format!("sr {sr} max {max} frames {frames}"));
        }
        plugin.reset();
        let out = plugin.process(
            &[rng.noise(max as usize)],
            max as usize,
            &Events(Vec::new()),
        );
        assert_finite(&out, &format!("after reset, sr {sr}"));
    }
    // Deactivate + reactivate at the same settings, then destroy while active.
    plugin.deactivate();
    plugin.activate(44_100.0, 64, 64);
    let out = plugin.process(&[rng.noise(64)], 64, &Events(Vec::new()));
    assert_finite(&out, "reactivated");
}

/// clap-validator `param-fuzz-basic`: random values for all parameters every block, with
/// noise input; outputs must stay finite.
#[test]
fn param_fuzz_keeps_output_finite() {
    let mut plugin = Instance::new();
    let infos: Vec<clap_param_info> = (0..plugin.param_count())
        .map(|i| plugin.param_info(i))
        .collect();
    let mut rng = Lcg(0x5eed);
    for sr in [22_050.0, 44_100.0, 48_000.0, 96_000.0] {
        plugin.activate(sr, 1, 512);
        for block in 0..200 {
            let values: Vec<(clap_id, f64)> = infos
                .iter()
                .map(|info| {
                    let t = rng.next_f64();
                    let mut v = info.min_value + t * (info.max_value - info.min_value);
                    if info.flags & CLAP_PARAM_IS_STEPPED != 0 {
                        v = v.round();
                    }
                    // Also hit the exact range ends regularly.
                    match block % 10 {
                        0 => (info.id, info.min_value),
                        1 => (info.id, info.max_value),
                        _ => (info.id, v),
                    }
                })
                .collect();
            let input = rng.noise(512);
            let out = plugin.process(&[input], 512, &Events::param(&values));
            assert_finite(&out, &format!("sr {sr} block {block} params {values:?}"));
        }
    }
}

#[test]
fn stereo_config_processes_both_channels() {
    let mut plugin = Instance::new();
    assert!(plugin.select_audio_ports_config(1), "2-in/2-out config");
    plugin.activate(48_000.0, 1, 256);
    let mut rng = Lcg(3);
    for frames in [0, 256, 17] {
        let input = vec![rng.noise(256), rng.noise(256)];
        let out = plugin.process(&input, frames, &Events(Vec::new()));
        assert_eq!(out.len(), 2);
        assert_finite(&out, "stereo");
        assert_eq!(out[0], out[1], "second channel mirrors the first");
    }
    // Out-of-range config IDs are rejected.
    plugin.deactivate();
    assert!(!plugin.select_audio_ports_config(99));
}

/// Several instances at once (some hosts and clap-validator do this), created and
/// destroyed in different orders and on different threads.
#[test]
fn multiple_instances_coexist() {
    let mut instances: Vec<Instance> = (0..4).map(|_| Instance::new()).collect();
    for (i, plugin) in instances.iter_mut().enumerate() {
        plugin.activate(44_100.0 * (i + 1) as f64, 1, 128);
    }
    let mut rng = Lcg(11);
    for _ in 0..4 {
        for plugin in &instances {
            let out = plugin.process(&[rng.noise(128)], 128, &Events(Vec::new()));
            assert_finite(&out, "multi-instance");
        }
    }
    instances.remove(1);
    instances.remove(2);
    drop(instances);

    let handles: Vec<_> = (0..4)
        .map(|seed| {
            std::thread::spawn(move || {
                let mut plugin = Instance::new();
                plugin.activate(48_000.0, 1, 64);
                let mut rng = Lcg(seed);
                for _ in 0..16 {
                    let out = plugin.process(&[rng.noise(64)], 64, &Events(Vec::new()));
                    assert_finite(&out, "threaded instance");
                }
                let saved = plugin.save();
                let other = Instance::new();
                assert!(other.load(&saved, usize::MAX));
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("instance thread panicked");
    }
}

/// nih-plug's CLAP wrapper skips `process()` for 0-frame blocks (debug-logging
/// `buffer_is_valid`), so check the engine directly: empty blocks and blocks longer than
/// the prepared maximum (hosts that ignore `max_frames_count`) must be handled.
#[test]
fn engine_handles_empty_and_oversized_blocks() {
    let outputs = PluginOutputs::default();
    let mut engine = AidaEngine::new();
    let params = ParamSnapshot::default();
    let mut rng = Lcg(5);
    for (sr, max) in [(44_100.0, 0usize), (48_000.0, 1), (96_000.0, 64)] {
        engine.prepare(sr, max, &params);
        engine.process(&mut [], &outputs);
        let mut block = rng.noise(1000);
        engine.process(&mut block, &outputs);
        assert_finite(&[block], &format!("oversized block, sr {sr} max {max}"));
        engine.process(&mut [], &outputs);
    }
}
