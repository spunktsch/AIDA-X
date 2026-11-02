use crate::model::types::{LayerConfig, LayerType, ModelConfig};

use serde::Deserialize;

#[derive(Deserialize)]
struct RawModel {
    in_shape: Vec<Option<i64>>,
    #[serde(default)]
    in_skip: ValueOrInt,
    #[serde(default)]
    in_gain: ValueOrFloat,
    #[serde(default)]
    out_gain: ValueOrFloat,
    layers: Vec<RawLayer>,
}

#[derive(Deserialize)]
struct ValueOrInt {
    int: Option<i64>,
}

impl Default for ValueOrInt {
    fn default() -> Self {
        Self { int: None }
    }
}

#[derive(Deserialize)]
struct ValueOrFloat {
    float: Option<f32>,
}

impl Default for ValueOrFloat {
    fn default() -> Self {
        Self { float: None }
    }
}

#[derive(Deserialize)]
struct RawLayer {
    #[serde(alias = "type")]
    layer_type: String,
    #[serde(default)]
    activation: String,
    shape: Vec<Option<i64>>,
    weights: Vec<Vec<Vec<f32>>>,
}

pub fn parse_model(json_str: &str) -> Result<ModelConfig, String> {
    let raw: RawModel =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;

    let in_skip = raw.in_skip.int.unwrap_or(0);
    let in_gain = raw.in_gain.float.unwrap_or(1.0);
    let out_gain = raw.out_gain.float.unwrap_or(1.0);

    let layers = raw
        .layers
        .into_iter()
        .map(|rl| {
            let layer_type = match rl.layer_type.as_str() {
                "gru" => LayerType::Gru,
                "lstm" => LayerType::Lstm,
                "dense" => LayerType::Dense,
                other => {
                    return Err(format!("Unknown layer type: {}", other));
                }
            };
            Ok(LayerConfig {
                layer_type,
                shape: rl.shape,
                weights: rl.weights,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ModelConfig {
        in_shape: raw.in_shape,
        in_skip,
        in_gain,
        out_gain,
        layers,
    })
}
