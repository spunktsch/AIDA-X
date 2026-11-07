//! JSON parser for AIDA-X model files (RTNeural / Keras-style export).
//!
//! Format (as read by `loadModelFromStream` in `aidadsp-plugin.cpp` and
//! `RTNeural::json_parser`):
//!
//! ```json
//! {
//!   "in_shape": [null, null, 1],
//!   "in_skip": 0,          // optional number, 0 or 1
//!   "in_gain": 0.0,        // optional number, dB
//!   "out_gain": 0.0,       // optional number, dB
//!   "layers": [
//!     { "type": "lstm", "activation": "", "shape": [null, null, 12],
//!       "weights": [ W[in][4H], U[H][4H], b[4H] ] },
//!     { "type": "dense", "activation": "", "shape": [null, null, 1],
//!       "weights": [ W[H][1], b[1] ] }
//!   ]
//! }
//! ```
//!
//! Other top-level keys (`metadata`, `input_batch`, `output_batch`, ...) are ignored.

use crate::model::types::{LayerConfig, LayerType, ModelConfig, ModelError, ModelResult, WeightArray};

use serde_json::Value;

fn parse_err(msg: impl Into<String>) -> ModelError {
    ModelError::Parse(msg.into())
}

fn parse_shape(v: &Value, what: &str) -> ModelResult<Vec<Option<i64>>> {
    let arr = v
        .as_array()
        .ok_or_else(|| parse_err(format!("`{what}` must be an array")))?;
    arr.iter()
        .map(|d| match d {
            Value::Null => Ok(None),
            Value::Number(n) => n
                .as_i64()
                .map(Some)
                .ok_or_else(|| parse_err(format!("`{what}` contains a non-integer dimension"))),
            _ => Err(parse_err(format!("`{what}` contains an invalid dimension"))),
        })
        .collect()
}

/// Flattens a (possibly nested) rectangular JSON array of numbers.
fn parse_weight_array(v: &Value) -> ModelResult<WeightArray> {
    fn walk(v: &Value, depth: usize, shape: &mut Vec<usize>, data: &mut Vec<f32>) -> ModelResult<()> {
        match v {
            Value::Number(n) => {
                if depth != shape.len() {
                    return Err(parse_err("ragged weight array"));
                }
                data.push(n.as_f64().ok_or_else(|| parse_err("invalid weight value"))? as f32);
                Ok(())
            }
            Value::Array(items) => {
                if depth == shape.len() {
                    // First time we reach this depth: it defines the extent.
                    if !data.is_empty() {
                        return Err(parse_err("ragged weight array"));
                    }
                    shape.push(items.len());
                } else if depth > shape.len() || shape[depth] != items.len() {
                    return Err(parse_err("ragged weight array"));
                }
                for item in items {
                    walk(item, depth + 1, shape, data)?;
                }
                Ok(())
            }
            _ => Err(parse_err("weights must be numbers")),
        }
    }

    let mut shape = Vec::new();
    let mut data = Vec::new();
    walk(v, 0, &mut shape, &mut data)?;
    if shape.iter().product::<usize>() != data.len() {
        return Err(parse_err("ragged weight array"));
    }
    Ok(WeightArray { shape, data })
}

fn parse_layer(v: &Value) -> ModelResult<LayerConfig> {
    let type_str = v
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| parse_err("layer is missing `type`"))?;
    let layer_type = match type_str {
        "gru" => LayerType::Gru,
        "lstm" => LayerType::Lstm,
        "dense" | "time-distributed-dense" => LayerType::Dense,
        other => return Err(ModelError::Unsupported(format!("layer type `{other}`"))),
    };
    let activation = v
        .get("activation")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let shape = parse_shape(v.get("shape").ok_or_else(|| parse_err("layer is missing `shape`"))?, "shape")?;
    let weights = v
        .get("weights")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_err("layer is missing `weights` array"))?
        .iter()
        .map(parse_weight_array)
        .collect::<ModelResult<Vec<_>>>()?;

    Ok(LayerConfig {
        layer_type,
        activation,
        shape,
        weights,
    })
}

/// Parses a model JSON document. Optional fields follow the C++ loader:
/// a non-numeric or missing `in_skip`/`in_gain`/`out_gain` falls back to 0 / 0 dB / 0 dB.
pub fn parse_model(json_str: &str) -> ModelResult<ModelConfig> {
    let root: Value = serde_json::from_str(json_str).map_err(|e| parse_err(format!("invalid JSON: {e}")))?;
    parse_model_value(&root)
}

pub fn parse_model_value(root: &Value) -> ModelResult<ModelConfig> {
    let in_shape = parse_shape(
        root.get("in_shape").ok_or_else(|| parse_err("missing `in_shape`"))?,
        "in_shape",
    )?;
    if in_shape.last().copied().flatten().is_none() {
        return Err(parse_err("last dimension of `in_shape` must be an integer"));
    }

    let in_skip = match root.get("in_skip") {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) as i64,
        _ => 0,
    };
    if !(0..=1).contains(&in_skip) {
        return Err(ModelError::Unsupported(format!("in_skip = {in_skip} (only 0 or 1 supported)")));
    }

    let gain_db = |key: &str| match root.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) as f32,
        _ => 0.0,
    };

    let layers = root
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_err("missing `layers` array"))?
        .iter()
        .map(parse_layer)
        .collect::<ModelResult<Vec<_>>>()?;

    Ok(ModelConfig {
        in_shape,
        in_skip,
        in_gain: gain_db("in_gain"),
        out_gain: gain_db("out_gain"),
        layers,
    })
}
