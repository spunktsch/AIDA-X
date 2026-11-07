//! Safe Rust wrapper around the vendored C++ [RTNeural] engine.
//!
//! `cpp/shim.cpp` includes the C++ plugin's `src/model_variant.hpp`, i.e. the
//! same 54 compile-time `RTNeural::ModelT` types (GRU/LSTM × hidden
//! 8..80 × input 1..3, followed by Dense(H → 1)) and the same
//! `custom_model_creator` the C++ AIDA-X plugin uses, compiled with the Eigen
//! backend exactly like `CMakeLists.txt`.
//!
//! This crate only runs the raw network: input/output gain and the `in_skip`
//! residual connection are applied by `aidadx_core::model::DynamicModel`.
//!
//! [RTNeural]: https://github.com/jatinchowdhury18/RTNeural

use std::ffi::{c_char, c_int, CStr};
use std::fmt;
use std::ptr::NonNull;

mod ffi {
    use std::ffi::{c_char, c_int};

    #[repr(C)]
    pub struct AidadxRtModel {
        _private: [u8; 0],
    }

    extern "C" {
        pub fn aidadx_rt_create(
            json: *const c_char,
            json_len: usize,
            err: *mut c_char,
            err_len: usize,
        ) -> *mut AidadxRtModel;
        pub fn aidadx_rt_variant_index(model: *const AidadxRtModel) -> c_int;
        pub fn aidadx_rt_input_size(model: *const AidadxRtModel) -> c_int;
        pub fn aidadx_rt_reset(model: *mut AidadxRtModel);
        pub fn aidadx_rt_process(
            model: *mut AidadxRtModel,
            input: *const f32,
            output: *mut f32,
            n: usize,
            param1: f32,
            param2: f32,
        );
        pub fn aidadx_rt_destroy(model: *mut AidadxRtModel);
    }
}

/// Model construction failed (invalid JSON, unsupported architecture, bad weight shapes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RTNeural: {}", self.0)
    }
}

impl std::error::Error for Error {}

/// One of the 54 compile-time RTNeural models, owned on the C++ heap.
///
/// Construction and [`Drop`] allocate/free and must happen off the audio
/// thread; [`Model::process`], [`Model::process_in_place`] and
/// [`Model::reset`] are real-time safe.
pub struct Model {
    raw: NonNull<ffi::AidadxRtModel>,
    input_size: usize,
    variant_index: usize,
}

// SAFETY: the C++ object has no thread affinity and no shared/global state; all
// mutation goes through `&mut self`. It is not `Sync` (process/reset mutate).
unsafe impl Send for Model {}

impl Model {
    /// Parses an AIDA-X / RTNeural model JSON and builds the matching
    /// compile-time network (equivalent to `loadModelFromStream` in the C++ plugin).
    pub fn from_json(json: &str) -> Result<Self, Error> {
        let mut err = [0 as c_char; 512];
        // SAFETY: pointers/lengths describe live buffers; the shim never throws.
        let raw = unsafe {
            ffi::aidadx_rt_create(
                json.as_ptr().cast(),
                json.len(),
                err.as_mut_ptr(),
                err.len(),
            )
        };
        let Some(raw) = NonNull::new(raw) else {
            // SAFETY: the shim always NUL-terminates within `err.len()`.
            let msg = unsafe { CStr::from_ptr(err.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            return Err(Error(if msg.is_empty() {
                "unknown error".into()
            } else {
                msg
            }));
        };
        // SAFETY: `raw` is a live model.
        let (input_size, variant_index) = unsafe {
            (
                ffi::aidadx_rt_input_size(raw.as_ptr()),
                ffi::aidadx_rt_variant_index(raw.as_ptr()),
            )
        };
        let model = Model {
            raw,
            input_size: to_usize(input_size),
            variant_index: to_usize(variant_index),
        };
        if !(1..=3).contains(&model.input_size) || model.variant_index == 0 {
            return Err(Error("shim returned an empty model".into()));
        }
        Ok(model)
    }

    /// Network input size: 1 = audio only, 2/3 = audio + param1 (+ param2).
    pub fn input_size(&self) -> usize {
        self.input_size
    }

    /// 1-based index into `ModelVariantType` (`model_variant.hpp`), i.e. the
    /// position in `aidadx_core::model::ModelArch::ALL` plus one.
    pub fn variant_index(&self) -> usize {
        self.variant_index
    }

    /// Clears the recurrent state. Real-time safe.
    pub fn reset(&mut self) {
        // SAFETY: `self.raw` is live and uniquely borrowed.
        unsafe { ffi::aidadx_rt_reset(self.raw.as_ptr()) }
    }

    /// `output[i] = net(input[i], param1, param2)`. Real-time safe.
    ///
    /// # Panics
    /// If `input` and `output` differ in length.
    pub fn process(&mut self, input: &[f32], output: &mut [f32], param1: f32, param2: f32) {
        assert_eq!(input.len(), output.len(), "input/output length mismatch");
        // SAFETY: both slices are valid for `len` elements and do not overlap
        // (guaranteed by the borrow rules); the shim never throws.
        unsafe {
            ffi::aidadx_rt_process(
                self.raw.as_ptr(),
                input.as_ptr(),
                output.as_mut_ptr(),
                input.len(),
                param1,
                param2,
            )
        }
    }

    /// In-place variant of [`Model::process`]: `buf[i] = net(buf[i], ..)`.
    pub fn process_in_place(&mut self, buf: &mut [f32], param1: f32, param2: f32) {
        let p = buf.as_mut_ptr();
        // SAFETY: the shim reads `in[i]` before writing `out[i]`, so aliasing
        // the same buffer is allowed by its contract.
        unsafe { ffi::aidadx_rt_process(self.raw.as_ptr(), p, p, buf.len(), param1, param2) }
    }

    /// Single-sample convenience wrapper (slower per sample than block processing).
    pub fn forward(&mut self, sample: f32, param1: f32, param2: f32) -> f32 {
        let mut x = [sample];
        self.process_in_place(&mut x, param1, param2);
        x[0]
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `aidadx_rt_create` and is freed exactly once.
        unsafe { ffi::aidadx_rt_destroy(self.raw.as_ptr()) }
    }
}

impl fmt::Debug for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Model")
            .field("variant_index", &self.variant_index)
            .field("input_size", &self.input_size)
            .finish()
    }
}

fn to_usize(v: c_int) -> usize {
    usize::try_from(v).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LSTM_8_{input} with constant weights (the numeric checks against the
    /// RTNeural reference live in aidadx_core's model tests).
    fn tiny_lstm_json(input: usize) -> String {
        let h = 8;
        let row = |v: f32| format!("[{}]", vec![v.to_string(); 4 * h].join(","));
        let kernel = format!(
            "[{}]",
            (0..input).map(|_| row(0.1)).collect::<Vec<_>>().join(",")
        );
        let rec = format!(
            "[{}]",
            (0..h).map(|_| row(0.05)).collect::<Vec<_>>().join(",")
        );
        let bias = row(0.0);
        let dense_w = format!(
            "[{}]",
            (0..h)
                .map(|_| "[0.5]".to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        format!(
            r#"{{"in_shape":[null,null,{input}],"layers":[
                {{"type":"lstm","activation":"","shape":[null,null,{h}],"weights":[{kernel},{rec},{bias}]}},
                {{"type":"dense","activation":"","shape":[null,null,1],"weights":[{dense_w},[0.0]]}}]}}"#
        )
    }

    #[test]
    fn create_process_reset() {
        for input in 1..=3 {
            let mut m = Model::from_json(&tiny_lstm_json(input)).unwrap();
            assert_eq!(m.input_size(), input);
            // LSTM_8_{input}: GRU variants occupy 1..=27, LSTM_8_1 is 28.
            assert_eq!(m.variant_index(), 27 + input);

            let x = [0.5f32; 16];
            let mut a = [0.0f32; 16];
            m.process(&x, &mut a, 0.2, 0.3);
            assert!(a.iter().all(|v| v.is_finite()));
            assert!(a[0] != 0.0 && a[0] != a[15], "state must evolve: {a:?}");

            m.reset();
            let mut b = x;
            m.process_in_place(&mut b, 0.2, 0.3);
            assert_eq!(a, b, "reset + in-place must reproduce the first run");
        }
    }

    #[test]
    fn errors_do_not_cross_ffi() {
        assert!(Model::from_json("not json").is_err());
        assert!(Model::from_json("{}").is_err());
        assert!(Model::from_json("").is_err());
        // Unsupported hidden size.
        let bad = tiny_lstm_json(1).replace("[null,null,8]", "[null,null,9]");
        let e = Model::from_json(&bad).unwrap_err();
        assert!(!e.0.is_empty());
        // Wrong kernel shape.
        let bad =
            tiny_lstm_json(2).replace("\"in_shape\":[null,null,2]", "\"in_shape\":[null,null,1]");
        assert!(Model::from_json(&bad).is_err());
        // Too many inputs.
        let bad =
            tiny_lstm_json(1).replace("\"in_shape\":[null,null,1]", "\"in_shape\":[null,null,4]");
        assert!(Model::from_json(&bad).is_err());
    }

    #[test]
    fn model_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Model>();
        let m = Model::from_json(&tiny_lstm_json(1)).unwrap();
        std::thread::spawn(move || drop(m)).join().unwrap();
    }
}
