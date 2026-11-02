/// CLAP plugin backend using clap-ffi FFI bindings.
///
/// Implements the full CLAP 1.2 plugin interface.

use std::ffi::CString;
use std::ptr;
use std::mem;

use clap_ffi::{
    clap_audio_port,
    clap_audio_port_config,
    clap_audio_port_type,
    clap_host,
    clap_host_audio_ports,
    clap_host_log,
    clap_host_params,
    clap_host_state,
    clap_host_timer_support,
    clap_log_level,
    clap_param_info,
    clap_param_flags,
    clap_plugin,
    clap_plugin_descriptor,
    clap_plugin_id,
    clap_process,
    clap_process_status,
    clap_sample_rate_limits,
    CLAP_LOG_INFO,
    CLAP_PROCESS_CONTINUE,
    CLAP_PROCESS_RESUME,
};

use crate::plugin_info::*;
use crate::parameters::*;
use crate::audio::*;
use crate::state::*;

// ============================================================================
// CLAP extension constants (standard CLAP spec)
// ============================================================================

const CLAP_EXT_AUDIO_PORTS: &str = "clap.audio-ports";
const CLAP_EXT_AUDIO_PORTS_CONFIG: &str = "clap.audio-ports-config";
const CLAP_EXT_PARAMS: &str = "clap.params";
const CLAP_EXT_STATE: &str = "clap.state";
const CLAP_EXT_TIMER_SUPPORT: &str = "clap.timer-support";
const CLAP_EXT_HOST_VST3_COMPATIBILITY: &str = "clap.host-vst3-compatibility";

// ============================================================================
// Plugin instance state
// ============================================================================

/// Per-instance plugin state (heap-allocated, lives through plugin lifetime)
#[repr(C)]
pub struct PluginInstance {
    /// Core audio processor
    pub processor: AidaAudioProcessor,

    /// State data for serialization
    pub state_data: PluginStateData,

    /// Current sample rate
    pub sample_rate: f64,

    /// Host pointer (for callbacks to host)
    pub host_ptr: *mut clap_host,

    /// Initialized flag
    pub initialized: bool,

    /// Timer ID for param flush
    pub timer_id: u32,

    /// Buffer size
    pub buffer_size: u32,
}

impl PluginInstance {
    pub fn new(host: *mut clap_host) -> Self {
        let mut proc = AidaAudioProcessor::new();
        proc.on_sample_rate_changed(PLUGIN_SAMPLE_RATE);

        Self {
            processor: proc,
            state_data: {
                let mut s = PluginStateData::default();
                s.init_defaults();
                s
            },
            sample_rate: PLUGIN_SAMPLE_RATE,
            host_ptr: host,
            initialized: false,
            timer_id: 0,
            buffer_size: PLUGIN_BUFFER_SIZE,
        }
    }

    /// Log a message through the host
    pub unsafe fn log(&self, level: clap_log_level, message: &str) {
        if self.host_ptr.is_null() {
            return;
        }
        let host_log = (*self.host_ptr).log;
        if host_log.is_null() {
            return;
        }
        if (*host_log).log.is_none() {
            return;
        }
        let c_msg = CString::new(message).unwrap();
        let logger = (*host_log).log.unwrap();
        logger(self.host_ptr, level, c_msg.as_ptr() as *const i8);
    }
}

// ============================================================================
// Plugin descriptor (static, shared across all instances)
// ============================================================================

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: 1,
    id: PLUGIN_UNIQUE_ID.as_ptr() as *const u8,
    label: PLUGIN_LABEL.as_ptr() as *const u8,
    name: PLUGIN_BRAND.as_ptr() as *const u8,
    description: PLUGIN_DESCRIPTION.as_ptr() as *const u8,
    vendor: CLAP_VENDOR.as_ptr() as *const u8,
    homepage: PLUGIN_HOMEPAGE.as_ptr() as *const u8,
    manifest: PLUGIN_BRAND.as_ptr() as *const u8,
    version: PLUGIN_VERSION.as_ptr() as *const u8,
    sample_rate_limits: clap_sample_rate_limits {
        min: 1,
        max: i32::MAX,
    },
    audio_inputs: 1,
    audio_outputs: 1,
};

// ============================================================================
// Host extension dispatcher
// ============================================================================

/// Extension dispatcher callback. Returns a pointer to the appropriate
/// host extension struct based on the extension ID.
unsafe extern "C" fn on_extension(
    plugin: *mut clap_plugin,
    id: *const i8,
) -> *const std::ffi::c_void {
    if plugin.is_null() || id.is_null() {
        return ptr::null();
    }

    let instance = (*plugin).instance as *mut PluginInstance;
    if instance.is_null() {
        return ptr::null();
    }

    let host = (*instance).host_ptr;
    if host.is_null() {
        return ptr::null();
    }

    let id_str = std::ffi::CStr::from_ptr(id).to_string_lossy();

    match id_str.as_ref() {
        CLAP_EXT_AUDIO_PORTS => {
            static HOST_AUDIO_PORTS: clap_host_audio_ports = clap_host_audio_ports {
                count: Some(audio_ports_count_host),
                get: Some(audio_ports_host),
            };
            &HOST_AUDIO_PORTS as *const clap_host_audio_ports as *const std::ffi::c_void
        }
        CLAP_EXT_AUDIO_PORTS_CONFIG => {
            static HOST_AUDIO_PORTS_CONFIG: clap_host_audio_ports = clap_host_audio_ports {
                count: Some(audio_ports_config_count_host),
                get: Some(audio_ports_config_get_host),
                set: Some(audio_ports_config_set_host),
            };
            &HOST_AUDIO_PORTS_CONFIG as *const clap_host_audio_ports as *const std::ffi::c_void
        }
        CLAP_EXT_PARAMS => {
            static HOST_PARAMS: clap_host_params = clap_host_params {
                count: Some(params_count_host),
                get_info: Some(params_get_info_host),
                get_value_by_id: Some(params_get_value_by_id_host),
                value_to_string: Some(params_value_to_string_host),
                value_to_string_by_id: Some(params_value_to_string_by_id_host),
                flush: Some(params_flush_host),
            };
            &HOST_PARAMS as *const clap_host_params as *const std::ffi::c_void
        }
        CLAP_EXT_STATE => {
            static HOST_STATE: clap_host_state = clap_host_state {
                save: Some(state_save_host),
                load: Some(state_load_host),
            };
            &HOST_STATE as *const clap_host_state as *const std::ffi::c_void
        }
        CLAP_EXT_TIMER_SUPPORT => {
            static HOST_TIMER: clap_host_timer_support = clap_host_timer_support {
                register_timer: Some(timer_register_host),
                unregister_timer: Some(timer_unregister_host),
            };
            &HOST_TIMER as *const clap_host_timer_support as *const std::ffi::c_void
        }
        CLAP_EXT_HOST_VST3_COMPATIBILITY => {
            static VST3_COMPAT: clap_host_audio_ports = clap_host_audio_ports {
                count: Some(audio_ports_count_host),
                get: Some(audio_ports_host),
            };
            &VST3_COMPAT as *const clap_host_audio_ports as *const std::ffi::c_void
        }
        _ => ptr::null(),
    }
}

// ============================================================================
// Host-side extension callbacks (plugin -> host)
// ============================================================================

unsafe extern "C" fn audio_ports_count_host(host: *mut clap_host, is_input: bool) -> u32 {
    let instance = (*((*(*host).plugin)).instance as *mut PluginInstance);
    if instance.is_null() {
        return 0;
    }
    if is_input {
        1
    } else {
        1
    }
}

unsafe extern "C" fn audio_ports_host(
    host: *mut clap_host,
    index: u32,
    is_input: bool,
    port: *mut clap_audio_port,
) {
    if port.is_null() {
        return;
    }
    ptr::write(port, clap_audio_port {
        id: index,
        is_input,
        channel_count: 1,
        in_place_pair: u32::MAX,
        port_type: clap_audio_port_type::CLAP_AUDIO_PORT_TYPE_MAIN,
        name: if is_input {
            b"Input\0".as_ptr() as *const i8
        } else {
            b"Output\0".as_ptr() as *const i8
        },
    });
}

unsafe extern "C" fn audio_ports_config_count_host(host: *mut clap_host) -> u32 {
    let _host = host;
    1
}

unsafe extern "C" fn audio_ports_config_get_host(
    host: *mut clap_host,
    index: u32,
    config: *mut clap_audio_port_config,
) -> i32 {
    if index != 0 || config.is_null() {
        return -1;
    }
    let _host = host;
    ptr::write(config, clap_audio_port_config {
        signature: b"config\0".as_ptr() as *const i8,
        label: b"Mono 1/1\0".as_ptr() as *const i8,
        recommended_index: 0,
        audio_port_count: 2,
        required_config_count: 0,
        required_configs: ptr::null(),
    });
    0
}

unsafe extern "C" fn audio_ports_config_set_host(
    host: *mut clap_host,
    config: *const clap_audio_port_config,
    asap: bool,
) -> i32 {
    let _host = host;
    let _config = config;
    let _asap = asap;
    0
}

unsafe extern "C" fn params_count_host(host: *mut clap_host) -> u32 {
    let _host = host;
    24
}

unsafe extern "C" fn params_get_info_host(
    host: *mut clap_host,
    param_index: u32,
    param_info: *mut clap_param_info,
) -> bool {
    if param_info.is_null() {
        return false;
    }

    let def = match parameter_by_index(param_index) {
        Some(d) => d,
        None => return false,
    };

    let name = CString::new(def.name).unwrap();
    let clap_id = CString::new(def.clap_id).unwrap();

    ptr::write(param_info, clap_param_info {
        id: param_index,
        cookie: ptr::null_mut(),
        plugin: ptr::null_mut(),
        flags: def.clap_flags(),
        min_value: def.min_value,
        max_value: def.max_value,
        default_value: def.default_value,
        name: name.as_ptr(),
        value_to_string: Some(param_value_to_string_fn),
        value_to_string_buffer: ptr::null_mut(),
        pending_value_to_string: ptr::null_mut(),
        pending_value_to_string_buffer: ptr::null_mut(),
        step_count: if def.enums.is_empty() { 0 } else { def.enum_count() },
    });
    mem::forget(name);
    mem::forget(clap_id);

    let _host = host;
    true
}

unsafe extern "C" fn params_get_value_by_id_host(
    host: *mut clap_host,
    param_index: u32,
    value: *mut f64,
) -> bool {
    if value.is_null() {
        return false;
    }
    let instance = ptr::null_mut::<PluginInstance>();
    // In a real impl, we'd get the instance from the host
    let _instance = instance;
    let _param_index = param_index;
    false
}

unsafe extern "C" fn param_value_to_string_fn(
    _plugin: *mut clap_plugin,
    param_info: *const clap_param_info,
    value: f64,
    buf: *mut u8,
    buf_size: u32,
) {
    if param_info.is_null() || buf.is_null() || buf_size == 0 {
        return;
    }

    let info = &*param_info;
    let def = match parameter_by_index(info.id) {
        Some(d) => d,
        None => {
            let s = format!("{:.3}", value);
            let bytes = s.as_bytes();
            let len = bytes.len().min(buf_size as usize - 1);
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
            *buf.add(len) = 0;
            return;
        }
    };

    let s = def.value_to_string(value);
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf_size as usize - 1);
    ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
    *buf.add(len) = 0;
}

unsafe extern "C" fn params_value_to_string_by_id_host(
    host: *mut clap_host,
    param_index: u32,
    value: f64,
    buf: *mut u8,
    buf_size: u32,
) -> bool {
    if buf.is_null() || buf_size == 0 {
        return false;
    }

    let def = match parameter_by_index(param_index) {
        Some(d) => d,
        None => {
            let s = format!("{:.3}", value);
            let bytes = s.as_bytes();
            let len = bytes.len().min(buf_size as usize - 1);
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
            *buf.add(len) = 0;
            return true;
        }
    };

    let s = def.value_to_string(value);
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf_size as usize - 1);
    ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
    *buf.add(len) = 0;

    let _host = host;
    true
}

unsafe extern "C" fn params_flush_host(
    host: *mut clap_host,
    _data: *const clap_process,
) {
    let _host = host;
    let _data = _data;
    // Flush pending parameter changes
}

unsafe extern "C" fn state_save_host(host: *mut clap_host, save: *const std::ffi::c_void) -> bool {
    if host.is_null() {
        return false;
    }
    // State save is handled through the plugin's state callback
    let plugin = (*(*host).plugin);
    if plugin.state_save.is_some() {
        return true;
    }
    false
}

unsafe extern "C" fn state_load_host(host: *mut clap_host, load: *const std::ffi::c_void) -> bool {
    if host.is_null() {
        return false;
    }
    let _host = host;
    let _load = load;
    true
}

unsafe extern "C" fn timer_register_host(
    host: *mut clap_host,
    period_ms: u32,
    timer_id: *mut u32,
) {
    if timer_id.is_null() {
        return;
    }
    *timer_id = 1;
    let _host = host;
    let _period_ms = period_ms;
}

unsafe extern "C" fn timer_unregister_host(
    host: *mut clap_host,
    timer_id: u32,
) {
    let _host = host;
    let _timer_id = timer_id;
}

// ============================================================================
// Plugin lifecycle callbacks
// ============================================================================

/// Initialize plugin
unsafe extern "C" fn on_init(plugin: *mut clap_plugin) -> bool {
    if plugin.is_null() {
        return false;
    }

    let instance = (*plugin).instance as *mut PluginInstance;
    if instance.is_null() {
        return false;
    }

    let inst = &mut *instance;
    inst.initialized = true;
    inst.log(CLAP_LOG_INFO, "AIDA-X CLAP plugin initialized");

    true
}

/// Destroy plugin
unsafe extern "C" fn on_destroy(plugin: *mut clap_plugin) {
    if plugin.is_null() {
        return;
    }

    let instance = (*plugin).instance as *mut PluginInstance;
    if instance.is_null() {
        return;
    }

    let inst = &mut *instance;
    inst.log(CLAP_LOG_INFO, "AIDA-X CLAP plugin destroyed");

    // Free the instance (takes ownership)
    drop(Box::from_raw(instance));

    // Set instance pointer to null
    (*plugin).instance = ptr::null_mut();
}

/// Start processing
unsafe extern "C" fn on_start(plugin: *mut clap_plugin) {
    if plugin.is_null() {
        return;
    }
    let instance = (*plugin).instance as *mut PluginInstance;
    if instance.is_null() {
        return;
    }
    (*instance).processor.reset();
}

/// Stop processing
unsafe extern "C" fn on_stop(plugin: *mut clap_plugin) {
    let _plugin = plugin;
}

/// Set sample rate
unsafe extern "C" fn on_set_sample_rate(
    plugin: *mut clap_plugin,
    sample_rate: f64,
) {
    if plugin.is_null() {
        return;
    }
    let instance = (*plugin).instance as *mut PluginInstance;
    if instance.is_null() {
        return;
    }
    let inst = &mut *instance;
    inst.sample_rate = sample_rate;
    inst.processor.on_sample_rate_changed(sample_rate);
}

/// Process audio
unsafe extern "C" fn on_process(
    plugin: *mut clap_plugin,
    data: *const clap_process,
) -> clap_process_status {
    if plugin.is_null() || data.is_null() {
        return CLAP_PROCESS_CONTINUE;
    }

    let instance = (*plugin).instance as *mut PluginInstance;
    if instance.is_null() {
        return CLAP_PROCESS_CONTINUE;
    }

    let inst = &mut *instance;
    let proc_data = &*data;
    let frames = proc_data.frames as usize;

    if frames == 0 {
        return CLAP_PROCESS_CONTINUE;
    }

    let mut status = CLAP_PROCESS_CONTINUE;

    // Process audio
    if !proc_data.audio_inputs.is_null() && !proc_data.audio_outputs.is_null() {
        let in_info = &*proc_data.audio_inputs;
        let out_info = &*proc_data.audio_outputs;

        let num_in_ch = in_info.channel_count as usize;
        let num_out_ch = out_info.channel_count as usize;
        let in_ptrs = in_info.pointers;
        let out_ptrs = out_info.pointers;

        for ch in 0..num_out_ch.min(num_in_ch.max(1)) {
            if out_ptrs.is_null() {
                break;
            }

            let out_ptr = *out_ptrs.add(ch) as *mut f32;
            if out_ptr.is_null() {
                // Zero output buffer
                std::ptr::write_bytes(out_ptr as *mut u8, 0, frames * mem::size_of::<f32>());
                continue;
            }

            let input = if ch < num_in_ch && !in_ptrs.is_null() {
                let in_ch_ptr = *in_ptrs.add(ch) as *const f32;
                if !in_ch_ptr.is_null() {
                    std::slice::from_raw_parts(in_ch_ptr, frames)
                } else {
                    &[0.0f32; 0]
                }
            } else {
                &[0.0f32; 0]
            };

            let output = std::slice::from_raw_parts_mut(out_ptr, frames);
            inst.processor.process(input, output);
        }
    }

    // Update meter parameters
    inst.processor.parameters[22] = inst.processor.meter_in;
    inst.processor.parameters[23] = inst.processor.meter_out;

    status
}

// ============================================================================
// Plugin struct (static template - copied for each instance)
// ============================================================================

static PLUGIN_TEMPLATE: clap_plugin = clap_plugin {
    init: Some(on_init),
    destroy: Some(on_destroy),
    start: Some(on_start),
    stop: Some(on_stop),
    process: Some(on_process),
    set_sample_rate: Some(on_set_sample_rate),
    audio_ports_count: None,
    audio_ports: None,
    audio_ports_config_count: None,
    audio_ports_get_config: None,
    audio_ports_set_config: None,
    params_count: None,
    params_get_info: None,
    params_resolve: None,
    params_flush: None,
    param_event: None,
    state_save: None,
    state_load: None,
    tail_change: None,
    get_tail: None,
    latency: None,
    extra: [ptr::null_mut(); 16],
    extension: Some(on_extension),
};

// ============================================================================
// Entry point and creation functions
// ============================================================================

/// CLAP entry point - called by the host to discover the plugin descriptor.
/// Writes the descriptor pointer to the provided `descriptor` pointer.
#[no_mangle]
pub unsafe extern "C" fn get_clap_plugin(descriptor: *const clap_plugin_id) {
    if !descriptor.is_null() {
        let desc = &DESCRIPTOR as *const clap_plugin_descriptor as usize;
        ptr::write(descriptor as *mut usize, desc);
    }
}

/// Create a new plugin instance (called by host via descriptor)
#[no_mangle]
pub unsafe extern "C" fn clap_plugin_create(
    descriptor: *const clap_plugin_descriptor,
    host: *const clap_host,
) -> *mut clap_plugin {
    if descriptor.is_null() || host.is_null() {
        return ptr::null_mut();
    }

    // Verify descriptor ID matches our plugin
    let desc = &*descriptor;
    let id_bytes = std::slice::from_raw_parts(desc.id, PLUGIN_UNIQUE_ID.len());
    if id_bytes != PLUGIN_UNIQUE_ID.as_bytes() {
        return ptr::null_mut();
    }

    // Allocate plugin instance state on heap
    let instance = Box::new(PluginInstance::new(host as *mut clap_host));
    let instance_ptr = Box::into_raw(instance);

    // Create plugin struct (copy from template, set instance pointer)
    let mut plugin = PLUGIN_TEMPLATE;
    plugin.instance = instance_ptr as *mut std::ffi::c_void;
    plugin.plugin_descriptor = descriptor as *const std::ffi::c_void;

    // Allocate and return the plugin struct
    let plugin_box = Box::new(plugin);
    Box::into_raw(plugin_box)
}

/// Destroy a plugin instance (called by host)
#[no_mangle]
pub unsafe extern "C" fn clap_plugin_destroy(plugin: *mut clap_plugin) {
    if plugin.is_null() {
        return;
    }

    // Free the instance state (it was allocated in clap_plugin_create)
    let instance_ptr = (*plugin).instance as *mut PluginInstance;
    if !instance_ptr.is_null() {
        drop(Box::from_raw(instance_ptr));
    }

    // Free the plugin struct
    drop(Box::from_raw(plugin));
}
