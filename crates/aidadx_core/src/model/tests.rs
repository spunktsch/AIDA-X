//! Model tests. The inference backends (RTNeural via FFI by default, candle
//! behind its feature) are checked against a scalar reference that transcribes
//! RTNeural's non-templated `GRULayer`/`LSTMLayer`/`Dense`
//! (`setWVals`/`setUVals`/`setBVals` + `forward`) and reads the JSON directly,
//! independently of `parser.rs`.
//!
//! Parser / detection / validation tests run in every feature configuration;
//! inference tests need `rtneural` or `candle`.

#![cfg_attr(not(any(feature = "rtneural", feature = "candle")), allow(dead_code))]

#[cfg(any(feature = "rtneural", feature = "candle"))]
use crate::model::loader::{custom_model_creator, AtomicModel, DynamicModel, BACKEND};
use crate::model::loader::{detect_arch, detect_arch_from_config, validate_network};
use crate::model::parser::parse_model;
use crate::model::types::{db_to_coef, LayerType, ModelArch, ModelError, ModelResult, SUPPORTED_HIDDEN_SIZES};

use serde_json::{json, Value};
#[cfg(any(feature = "rtneural", feature = "candle"))]
use std::time::Instant;

const REAL_MODEL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../files/tw40_california_clean_deerinkstudios.json"
);

// ---------------------------------------------------------------------------
// Deterministic test data
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40;
        (v as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }

    fn mat(&mut self, rows: usize, cols: usize, scale: f32) -> Value {
        json!((0..rows)
            .map(|_| (0..cols).map(|_| self.next() * scale).collect::<Vec<f32>>())
            .collect::<Vec<_>>())
    }

    fn vec(&mut self, n: usize, scale: f32) -> Value {
        json!((0..n).map(|_| self.next() * scale).collect::<Vec<f32>>())
    }
}

/// Builds a model JSON in the Keras/RTNeural export format.
fn model_json(lt: LayerType, hidden: usize, input: usize, extra: Value, seed: u64) -> Value {
    let mut rng = Rng(seed | 1);
    let gates = lt.gate_count();
    let scale = 1.5 / (hidden as f32).sqrt();
    let rnn_weights = match lt {
        LayerType::Gru => json!([
            rng.mat(input, 3 * hidden, 1.0),
            rng.mat(hidden, 3 * hidden, scale),
            rng.mat(2, 3 * hidden, 0.3),
        ]),
        LayerType::Lstm => json!([
            rng.mat(input, gates * hidden, 1.0),
            rng.mat(hidden, gates * hidden, scale),
            rng.vec(gates * hidden, 0.3),
        ]),
        LayerType::Dense => unreachable!(),
    };
    let mut model = json!({
        "in_shape": [null, null, input],
        "layers": [
            { "type": lt.to_string(), "activation": "", "shape": [null, null, hidden], "weights": rnn_weights },
            { "type": "dense", "activation": "", "shape": [null, null, 1],
              "weights": [rng.mat(hidden, 1, scale), rng.vec(1, 0.1)] }
        ]
    });
    if let Value::Object(extra) = extra {
        for (k, v) in extra {
            model[k] = v;
        }
    }
    model
}

fn test_signal(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32 / 48_000.0;
            0.6 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
                + 0.2 * (2.0 * std::f32::consts::PI * 1375.0 * t).sin()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Scalar RTNeural reference
// ---------------------------------------------------------------------------

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn v_mult(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn mat_of(v: &Value) -> Vec<Vec<f32>> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect())
        .collect()
}

fn vec_of(v: &Value) -> Vec<f32> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect()
}

/// Per-gate weights as RTNeural stores them: `W[k][i]`, `U[k][i]`, bias(es).
struct Gate {
    w: Vec<Vec<f32>>,
    u: Vec<Vec<f32>>,
    b0: Vec<f32>,
    b1: Vec<f32>,
}

impl Gate {
    /// `setWVals`/`setUVals`/`setBVals`: gate `g` occupies columns `g*H..(g+1)*H`.
    fn from_keras(w: &[Vec<f32>], u: &[Vec<f32>], b0: &[f32], b1: Option<&[f32]>, g: usize, h: usize) -> Self {
        let in_size = w.len();
        Gate {
            w: (0..h).map(|k| (0..in_size).map(|i| w[i][k + g * h]).collect()).collect(),
            u: (0..h).map(|k| (0..h).map(|i| u[i][k + g * h]).collect()).collect(),
            b0: (0..h).map(|k| b0[k + g * h]).collect(),
            b1: (0..h).map(|k| b1.map_or(0.0, |b| b[k + g * h])).collect(),
        }
    }
}

enum RefRnn {
    /// Gates z, r, c.
    Gru([Gate; 3], Vec<f32>),
    /// Gates i, f, c, o.
    Lstm([Gate; 4], Vec<f32>, Vec<f32>),
}

struct RefModel {
    rnn: RefRnn,
    dense_w: Vec<f32>,
    dense_b: f32,
}

impl RefModel {
    fn from_json(model: &Value) -> Self {
        let layers = model["layers"].as_array().unwrap();
        let rnn = &layers[0];
        let h = rnn["shape"].as_array().unwrap().last().unwrap().as_u64().unwrap() as usize;
        let w = mat_of(&rnn["weights"][0]);
        let u = mat_of(&rnn["weights"][1]);
        let rnn = match rnn["type"].as_str().unwrap() {
            "gru" => {
                let b = mat_of(&rnn["weights"][2]);
                let g = |i| Gate::from_keras(&w, &u, &b[0], Some(&b[1]), i, h);
                RefRnn::Gru([g(0), g(1), g(2)], vec![0.0; h])
            }
            "lstm" => {
                let b = vec_of(&rnn["weights"][2]);
                let g = |i| Gate::from_keras(&w, &u, &b, None, i, h);
                RefRnn::Lstm([g(0), g(1), g(2), g(3)], vec![0.0; h], vec![0.0; h])
            }
            t => panic!("unexpected layer {t}"),
        };
        // loadDense: denseWeights[j][i] = weights[0][i][j]; out_size = 1.
        let dw = mat_of(&layers[1]["weights"][0]);
        RefModel {
            rnn,
            dense_w: dw.iter().map(|r| r[0]).collect(),
            dense_b: vec_of(&layers[1]["weights"][1])[0],
        }
    }

    fn forward(&mut self, x: &[f32]) -> f32 {
        let h_out = match &mut self.rnn {
            RefRnn::Gru([z, r, c], ht1) => {
                let n = ht1.len();
                let mut h = vec![0.0; n];
                for i in 0..n {
                    let zv = sigmoid(v_mult(&z.w[i], x) + v_mult(&z.u[i], ht1) + z.b0[i] + z.b1[i]);
                    let rv = sigmoid(v_mult(&r.w[i], x) + v_mult(&r.u[i], ht1) + r.b0[i] + r.b1[i]);
                    let cv = (v_mult(&c.w[i], x) + rv * (v_mult(&c.u[i], ht1) + c.b1[i]) + c.b0[i]).tanh();
                    h[i] = (1.0 - zv) * cv + zv * ht1[i];
                }
                ht1.copy_from_slice(&h);
                h
            }
            RefRnn::Lstm([ig, fg, cg, og], ht1, ct1) => {
                let n = ht1.len();
                let mut h = vec![0.0; n];
                let mut c = vec![0.0; n];
                for i in 0..n {
                    let f = sigmoid(v_mult(&fg.w[i], x) + v_mult(&fg.u[i], ht1) + fg.b0[i]);
                    let iv = sigmoid(v_mult(&ig.w[i], x) + v_mult(&ig.u[i], ht1) + ig.b0[i]);
                    let o = sigmoid(v_mult(&og.w[i], x) + v_mult(&og.u[i], ht1) + og.b0[i]);
                    let ct = (v_mult(&cg.w[i], x) + v_mult(&cg.u[i], ht1) + cg.b0[i]).tanh();
                    c[i] = f * ct1[i] + iv * ct;
                    h[i] = o * c[i].tanh();
                }
                ct1.copy_from_slice(&c);
                ht1.copy_from_slice(&h);
                h
            }
        };
        v_mult(&self.dense_w, &h_out) + self.dense_b
    }
}

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

#[test]
fn parse_real_model_file() {
    let json = std::fs::read_to_string(REAL_MODEL).expect("example model in files/");
    let cfg = parse_model(&json).unwrap();
    assert_eq!(cfg.input_size(), 1);
    assert_eq!(cfg.hidden_size(), 12);
    assert_eq!(cfg.in_skip, 0);
    assert_eq!(cfg.in_gain, 0.0);
    assert_eq!(cfg.out_gain, 0.0);
    assert_eq!(cfg.layers.len(), 2);
    assert_eq!(cfg.layers[0].layer_type, LayerType::Lstm);
    assert_eq!(cfg.layers[0].weights[0].shape, vec![1, 48]);
    assert_eq!(cfg.layers[0].weights[1].shape, vec![12, 48]);
    assert_eq!(cfg.layers[0].weights[2].shape, vec![48]);
    assert_eq!(cfg.layers[1].layer_type, LayerType::Dense);
    assert_eq!(cfg.layers[1].weights[0].shape, vec![12, 1]);
    assert_eq!(cfg.layers[1].weights[1].shape, vec![1]);
    assert_eq!(detect_arch_from_config(&cfg).unwrap(), ModelArch::Lstm12_1);
}

#[test]
fn parse_optional_fields() {
    let m = model_json(LayerType::Gru, 8, 1, json!({"in_skip": 1, "in_gain": -6.0, "out_gain": 3.5}), 1);
    let cfg = parse_model(&m.to_string()).unwrap();
    assert_eq!(cfg.in_skip, 1);
    assert_eq!(cfg.in_gain, -6.0);
    assert_eq!(cfg.out_gain, 3.5);

    // Non-numeric values fall back to defaults like the C++ loader.
    let m = model_json(LayerType::Gru, 8, 1, json!({"in_skip": "yes", "in_gain": null}), 1);
    let cfg = parse_model(&m.to_string()).unwrap();
    assert_eq!(cfg.in_skip, 0);
    assert_eq!(cfg.in_gain, 0.0);
    assert_eq!(cfg.out_gain, 0.0);

    let m = model_json(LayerType::Gru, 8, 1, json!({"in_skip": 2}), 1);
    assert!(matches!(parse_model(&m.to_string()), Err(ModelError::Unsupported(_))));
}

#[test]
fn parse_rejects_bad_input() {
    assert!(matches!(parse_model("not json"), Err(ModelError::Parse(_))));
    assert!(matches!(parse_model(r#"{"in_shape":[null,1]}"#), Err(ModelError::Parse(_))));
    assert!(matches!(parse_model(r#"{"layers":[]}"#), Err(ModelError::Parse(_))));

    let mut m = model_json(LayerType::Gru, 8, 1, json!({}), 1);
    m["layers"][0]["type"] = json!("conv1d");
    assert!(matches!(parse_model(&m.to_string()), Err(ModelError::Unsupported(_))));

    let mut m = model_json(LayerType::Gru, 8, 1, json!({}), 1);
    m["layers"][0]["weights"][0] = json!([[1.0, 2.0], [3.0]]);
    assert!(matches!(parse_model(&m.to_string()), Err(ModelError::Parse(_))));
}

#[test]
fn db_to_coef_matches_cpp() {
    assert_eq!(db_to_coef(0.0), 1.0);
    assert!((db_to_coef(-6.0) - 0.501_187).abs() < 1e-5);
    assert!((db_to_coef(6.0) - 1.995_262).abs() < 1e-5);
    assert_eq!(db_to_coef(-90.0), 0.0);
}

// ---------------------------------------------------------------------------
// Architecture detection
// ---------------------------------------------------------------------------

#[test]
fn detect_all_54_architectures() {
    let mut seen = std::collections::HashSet::new();
    for lt in [LayerType::Gru, LayerType::Lstm] {
        for h in SUPPORTED_HIDDEN_SIZES {
            for i in 1..=3 {
                let arch = detect_arch(lt, h, i).unwrap();
                assert_eq!((arch.layer_type(), arch.hidden_size(), arch.input_size()), (lt, h, i));
                assert_eq!(arch.arch_name(), format!("{}_{}_{}", lt.to_string().to_uppercase(), h, i));
                seen.insert(arch);
            }
        }
    }
    assert_eq!(seen.len(), 54);
    assert_eq!(ModelArch::ALL.len(), 54);
    assert!(detect_arch(LayerType::Gru, 10, 1).is_none());
    assert!(detect_arch(LayerType::Lstm, 8, 4).is_none());
    assert!(detect_arch(LayerType::Dense, 8, 1).is_none());
}

/// Builds a model with the backend if there is one, otherwise runs the
/// backend-independent parse + detect + validate steps.
fn try_create(json: &str) -> ModelResult<()> {
    #[cfg(any(feature = "rtneural", feature = "candle"))]
    {
        custom_model_creator(json).map(|_| ())
    }
    #[cfg(not(any(feature = "rtneural", feature = "candle")))]
    {
        let cfg = parse_model(json)?;
        let arch = detect_arch_from_config(&cfg)?;
        validate_network(&cfg, arch)
    }
}

#[test]
fn create_rejects_unsupported_models() {
    let m = model_json(LayerType::Gru, 10, 1, json!({}), 1);
    assert!(matches!(try_create(&m.to_string()), Err(ModelError::Unsupported(_))));

    let m = model_json(LayerType::Lstm, 8, 1, json!({"in_shape": [null, null, 4]}), 1);
    assert!(matches!(try_create(&m.to_string()), Err(ModelError::Unsupported(_))));

    // Dense first.
    let mut m = model_json(LayerType::Gru, 8, 1, json!({}), 1);
    let layers = m["layers"].as_array_mut().unwrap();
    layers.swap(0, 1);
    assert!(try_create(&m.to_string()).is_err());

    // GRU with a 1-D bias (Keras reset_after=False) is not what RTNeural reads.
    let mut m = model_json(LayerType::Gru, 8, 1, json!({}), 1);
    m["layers"][0]["weights"][2] = json!(vec![0.0f32; 24]);
    assert!(matches!(try_create(&m.to_string()), Err(ModelError::Unsupported(_))));

    // Wrong kernel width.
    let mut m = model_json(LayerType::Lstm, 8, 1, json!({}), 1);
    m["layers"][0]["weights"][0] = json!([vec![0.0f32; 24]]);
    assert!(matches!(try_create(&m.to_string()), Err(ModelError::Unsupported(_))));

    // Wrong dense shape / extra layer.
    let mut m = model_json(LayerType::Lstm, 8, 1, json!({}), 1);
    m["layers"][1]["weights"][0] = json!(vec![vec![0.0f32; 2]; 8]);
    assert!(matches!(try_create(&m.to_string()), Err(ModelError::Unsupported(_))));
    let mut m = model_json(LayerType::Lstm, 8, 1, json!({}), 1);
    let extra = m["layers"][1].clone();
    m["layers"].as_array_mut().unwrap().push(extra);
    assert!(matches!(try_create(&m.to_string()), Err(ModelError::Unsupported(_))));

    // Every generated architecture passes validation.
    for arch in ModelArch::ALL {
        let m = model_json(arch.layer_type(), arch.hidden_size(), arch.input_size(), json!({}), 5);
        let cfg = parse_model(&m.to_string()).unwrap();
        assert_eq!(detect_arch_from_config(&cfg).unwrap(), arch);
        validate_network(&cfg, arch).unwrap();
    }
}


#[cfg(any(feature = "rtneural", feature = "candle"))]
mod backend;
