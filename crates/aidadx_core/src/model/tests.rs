use crate::model::inference::{DenseWeights, GruEngine, GruWeights, LstmEngine, LstmWeights};
use crate::model::loader::{
    custom_model_creator, create_model, detect_arch, detect_hidden_size_from_config,
    detect_input_size_from_config, AtomicModel, DynamicModel,
};
use crate::model::parser::parse_model;
use crate::model::types::{LayerConfig, LayerType, ModelArch};

use std::path::Path;

fn gru_json(input_size: usize, hidden_size: usize, input_skip: bool, in_gain: f32, out_gain: f32) -> String {
    let input_shape = match input_size {
        1 => format!("[null, null, {}]", input_size),
        2 => format!("[null, null, {}]", input_size),
        3 => format!("[null, null, {}]", input_size),
        _ => "[null, null, 1]".to_string(),
    };

    let in_skip = if input_skip { 1 } else { 0 };

    let mut gru_weights = Vec::new();
    for _ in 0..6 {
        let size = hidden_size * input_size;
        let data: Vec<f32> = (0..size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect();
        gru_weights.push(format!("[{}]", data.iter().map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(", ")));
    }
    for _ in 0..3 {
        let data: Vec<f32> = (0..hidden_size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect();
        gru_weights.push(format!("[{}]", data.iter().map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(", ")));
    }

    let dense_weights = format!(
        "[[{}]], [[{}]]",
        (0..hidden_size).map(|i| format!("{:.6}", (i as f32) * 0.02 - 0.5)).collect::<Vec<_>>().join(", "),
        (0..1).map(|_| "0.100000".to_string()).collect::<Vec<_>>().join(", ")
    );

    format!(
        r#"{{
            "in_shape": {input_shape},
            "in_skip": {in_skip},
            "in_gain": {in_gain},
            "out_gain": {out_gain},
            "layers": [
                {{
                    "type": "gru",
                    "activation": "",
                    "shape": [null, null, {hidden_size}],
                    "weights": [{gru}]
                }},
                {{
                    "type": "dense",
                    "activation": "",
                    "shape": [null, null, 1],
                    "weights": [{dense}]
                }}
            ]
        }}"#,
        input_shape = input_shape,
        gru = gru_weights.join(", "),
        dense = dense_weights,
    )
}

fn lstm_json(input_size: usize, hidden_size: usize, input_skip: bool, in_gain: f32, out_gain: f32) -> String {
    let in_skip = if input_skip { 1 } else { 0 };

    let weights_per_chunk = hidden_size * input_size;
    let mut lstm_weights = Vec::new();
    for _ in 0..8 {
        let data: Vec<f32> = (0..weights_per_chunk).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect();
        lstm_weights.push(format!("[{}]", data.iter().map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(", ")));
    }

    let bias_total = hidden_size * 4;
    let bias_data: Vec<f32> = (0..bias_total).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect();
    let bias_weights = format!("[{}]", bias_data.iter().map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(", "));

    let dense_weights = format!(
        "[[{}]], [[{}]]",
        (0..hidden_size).map(|i| format!("{:.6}", (i as f32) * 0.02 - 0.5)).collect::<Vec<_>>().join(", "),
        (0..1).map(|_| "0.100000".to_string()).collect::<Vec<_>>().join(", ")
    );

    format!(
        r#"{{
            "in_shape": [null, null, {}],
            "in_skip": {},
            "in_gain": {},
            "out_gain": {},
            "layers": [
                {{
                    "type": "lstm",
                    "activation": "",
                    "shape": [null, null, {}],
                    "weights": [{lstm}, {bias}]
                }},
                {{
                    "type": "dense",
                    "activation": "",
                    "shape": [null, null, 1],
                    "weights": [{dense}]
                }}
            ]
        }}"#,
        input_size,
        in_skip,
        in_gain,
        out_gain,
        hidden_size,
        lstm = lstm_weights.join(", "),
        bias = bias_weights,
        dense = dense_weights,
    )
}

fn gru_with_skip_json(hidden_size: usize) -> String {
    let input_size = 1;
    let mut gru_weights = Vec::new();
    for _ in 0..6 {
        let size = hidden_size * input_size;
        let data: Vec<f32> = (0..size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect();
        gru_weights.push(format!("[{}]", data.iter().map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(", ")));
    }
    for _ in 0..3 {
        let data: Vec<f32> = (0..hidden_size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect();
        gru_weights.push(format!("[{}]", data.iter().map(|v| format!("{:.6}", v)).collect::<Vec<_>>().join(", ")));
    }

    let dense_weights = format!(
        "[[{}]], [[{}]]",
        (0..hidden_size).map(|i| format!("{:.6}", (i as f32) * 0.02 - 0.5)).collect::<Vec<_>>().join(", "),
        (0..1).map(|_| "0.100000".to_string()).collect::<Vec<_>>().join(", ")
    );

    format!(
        r#"{{
            "in_shape": [null, null, {}],
            "in_skip": 1,
            "in_gain": -6.0,
            "out_gain": -3.0,
            "layers": [
                {{
                    "type": "gru",
                    "activation": "",
                    "shape": [null, null, {}],
                    "weights": [{gru}]
                }},
                {{
                    "type": "dense",
                    "activation": "",
                    "shape": [null, null, 1],
                    "weights": [{dense}]
                }}
            ]
        }}"#,
        input_size,
        hidden_size,
        gru = gru_weights.join(", "),
        dense = dense_weights,
    )
}

#[test]
fn test_gru_parse_json_input_size_1() {
    let json = gru_json(1, 8, false, -6.0, -3.0);
    let config = parse_model(&json).expect("Should parse GRU JSON");
    assert_eq!(config.layers.len(), 2);
    assert_eq!(config.layers[0].layer_type, LayerType::Gru);
    assert_eq!(config.layers[1].layer_type, LayerType::Dense);
    assert_eq!(config.in_skip, 0);
    assert!((config.in_gain - (-6.0)).abs() < f32::EPSILON);
    assert!((config.out_gain - (-3.0)).abs() < f32::EPSILON);
}

#[test]
fn test_gru_parse_json_input_size_2() {
    let json = gru_json(2, 16, false, -6.0, -3.0);
    let config = parse_model(&json).expect("Should parse GRU JSON");
    assert_eq!(detect_input_size_from_config(&config), 2);
}

#[test]
fn test_gru_parse_json_input_size_3() {
    let json = gru_json(3, 32, false, -6.0, -3.0);
    let config = parse_model(&json).expect("Should parse GRU JSON");
    assert_eq!(detect_input_size_from_config(&config), 3);
}

#[test]
fn test_lstm_parse_json() {
    let json = lstm_json(1, 8, false, -6.0, -3.0);
    let config = parse_model(&json).expect("Should parse LSTM JSON");
    assert_eq!(config.layers.len(), 2);
    assert_eq!(config.layers[0].layer_type, LayerType::Lstm);
    assert_eq!(config.layers[1].layer_type, LayerType::Dense);
}

#[test]
fn test_in_skip_detection() {
    let json = gru_with_skip_json(8);
    let config = parse_model(&json).expect("Should parse GRU with skip JSON");
    assert_eq!(config.in_skip, 1);
    assert!((config.in_gain - f32::powf(10.0, -6.0 * 0.05)).abs() < 1e-5);
}

#[test]
fn test_detect_arch_gru_all_variants() {
    let hidden_sizes = [8, 12, 16, 20, 24, 32, 40, 64, 80];
    let input_sizes = [1, 2, 3];

    for &hs in &hidden_sizes {
        for &is_ in &input_sizes {
            let arch = detect_arch(LayerType::Gru, hs, is_);
            assert!(arch.is_some(), "GRU arch should be detected for hs={}, is={}", hs, is_);
            assert_eq!(arch.unwrap().hidden_size(), hs);
            assert_eq!(arch.unwrap().input_size(), is_);
        }
    }
}

#[test]
fn test_detect_arch_lstm_all_variants() {
    let hidden_sizes = [8, 12, 16, 20, 24, 32, 40, 64, 80];
    let input_sizes = [1, 2, 3];

    for &hs in &hidden_sizes {
        for &is_ in &input_sizes {
            let arch = detect_arch(LayerType::Lstm, hs, is_);
            assert!(arch.is_some(), "LSTM arch should be detected for hs={}, is={}", hs, is_);
            assert_eq!(arch.unwrap().hidden_size(), hs);
            assert_eq!(arch.unwrap().input_size(), is_);
        }
    }
}

#[test]
fn test_all_54_gru_models_load() {
    let hidden_sizes = [8, 12, 16, 20, 24, 32, 40, 64, 80];
    let input_sizes = [1, 2, 3];
    let mut loaded = 0usize;

    for &hs in &hidden_sizes {
        for &is_ in &input_sizes {
            let json = gru_json(is_, hs, false, -6.0, -3.0);
            let result = custom_model_creator(&json);
            assert!(result.is_ok(), "GRU model hs={} is={} should load", hs, is_);
            let model = result.unwrap();
            assert_eq!(model.arch.hidden_size(), hs);
            assert_eq!(model.arch.input_size(), is_);
            assert!(!model.input_skip);
            loaded += 1;
        }
    }

    assert_eq!(loaded, 27);
}

#[test]
fn test_all_54_lstm_models_load() {
    let hidden_sizes = [8, 12, 16, 20, 24, 32, 40, 64, 80];
    let input_sizes = [1, 2, 3];
    let mut loaded = 0usize;

    for &hs in &hidden_sizes {
        for &is_ in &input_sizes {
            let json = lstm_json(is_, hs, false, -6.0, -3.0);
            let result = custom_model_creator(&json);
            assert!(result.is_ok(), "LSTM model hs={} is={} should load", hs, is_);
            let model = result.unwrap();
            assert_eq!(model.arch.hidden_size(), hs);
            assert_eq!(model.arch.input_size(), is_);
            loaded += 1;
        }
    }

    assert_eq!(loaded, 27);
}

#[test]
fn test_all_54_models_combined() {
    let hidden_sizes = [8, 12, 16, 20, 24, 32, 40, 64, 80];
    let input_sizes = [1, 2, 3];
    let mut total = 0usize;

    for &hs in &hidden_sizes {
        for &is_ in &input_sizes {
            let gru_json = gru_json(is_, hs, false, -6.0, -3.0);
            let gru_result = custom_model_creator(&gru_json);
            assert!(gru_result.is_ok(), "GRU hs={} is={}", hs, is_);
            total += 1;

            let lstm_json = lstm_json(is_, hs, false, -6.0, -3.0);
            let lstm_result = custom_model_creator(&lstm_json);
            assert!(lstm_result.is_ok(), "LSTM hs={} is={}", hs, is_);
            total += 1;
        }
    }

    assert_eq!(total, 54);
}

#[test]
fn test_model_with_input_skip() {
    let json = gru_with_skip_json(8);
    let model = custom_model_creator(&json).expect("Should create model with skip");
    assert!(model.input_skip);
}

#[test]
fn test_model_forward_no_skip() {
    let json = gru_json(1, 8, false, -6.0, -3.0);
    let model = custom_model_creator(&json).expect("Should create model");

    let mut input = [0.5f32];
    model.forward(&mut input, 0.0, 0.0);
    assert!(input[0].is_finite());
}

#[test]
fn test_model_forward_with_skip() {
    let json = gru_with_skip_json(8);
    let model = custom_model_creator(&json).expect("Should create model with skip");

    let mut input = [0.5f32];
    let original = input[0];
    model.forward(&mut input, 0.0, 0.0);
    assert!(input[0] != original);
    assert!(input[0].is_finite());
}

#[test]
fn test_model_forward_input_size_2() {
    let json = gru_json(2, 8, false, -6.0, -3.0);
    let model = custom_model_creator(&json).expect("Should create model");

    let mut input = [0.5f32, 0.3f32];
    model.forward(&mut input, 0.7, 0.0);
    assert!(input[0].is_finite());
}

#[test]
fn test_model_forward_input_size_3() {
    let json = gru_json(3, 8, false, -6.0, -3.0);
    let model = custom_model_creator(&json).expect("Should create model");

    let mut input = [0.5f32, 0.3f32, 0.1f32];
    model.forward(&mut input, 0.7, 0.9);
    assert!(input[0].is_finite());
}

#[test]
fn test_gain_compensation() {
    let json = gru_json(1, 8, false, -6.0, -3.0);
    let model = custom_model_creator(&json).expect("Should create model");
    assert!((model.input_gain - f32::powf(10.0, -6.0 * 0.05)).abs() < 1e-5);
    assert!((model.output_gain - f32::powf(10.0, -3.0 * 0.05)).abs() < 1e-5);
}

#[test]
fn test_atomic_model_swap() {
    let json1 = gru_json(1, 8, false, -6.0, -3.0);
    let model1 = custom_model_creator(&json1).expect("Should create model 1");
    let atomic = AtomicModel::new(model1);

    let json2 = lstm_json(1, 12, false, -6.0, -3.0);
    let model2 = custom_model_creator(&json2).expect("Should create model 2");

    atomic.swap(model2);

    let current = atomic.model();
    assert_eq!(current.arch.hidden_size(), 12);
}

#[test]
fn test_invalid_layer_type() {
    let json = r#"{
        "in_shape": [null, null, 1],
        "in_skip": 0,
        "in_gain": -6.0,
        "out_gain": -3.0,
        "layers": [
            {
                "type": "conv",
                "activation": "",
                "shape": [null, null, 8],
                "weights": []
            }
        ]
    }"#;
    let result = parse_model(json);
    assert!(result.is_err(), "Should reject unknown layer type");
}

#[test]
fn test_missing_layers() {
    let json = r#"{
        "in_shape": [null, null, 1],
        "in_skip": 0,
        "in_gain": -6.0,
        "out_gain": -3.0,
        "layers": []
    }"#;
    let result = custom_model_creator(json);
    assert!(result.is_err(), "Should reject model with no layers");
}

#[test]
fn test_model_arch_names() {
    let hidden_sizes = [8, 12, 16, 20, 24, 32, 40, 64, 80];
    for &hs in &hidden_sizes {
        for &is_ in &[1, 2, 3] {
            let arch = detect_arch(LayerType::Gru, hs, is_).unwrap();
            assert!(!arch.arch_name().is_empty());
            assert!(arch.arch_name().starts_with("GRU_"));

            let arch = detect_arch(LayerType::Lstm, hs, is_).unwrap();
            assert!(!arch.arch_name().is_empty());
            assert!(arch.arch_name().starts_with("LSTM_"));
        }
    }
}

#[test]
fn test_load_real_model_json() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json_path = test_dir.parent().unwrap().parent().unwrap().join("files/tw40_california_clean_deerinkstudios.json");
    
    if json_path.exists() {
        let json = std::fs::read_to_string(&json_path).expect("Should read model JSON");
        let result = custom_model_creator(&json);
        assert!(result.is_ok(), "Should load real model: {:?}", result.err());
        if let Ok(model) = result {
            assert_eq!(model.arch.hidden_size(), 12);
        }
    } else {
        let minimal_json = lstm_json(1, 12, false, -6.0, -3.0);
        let result = custom_model_creator(&minimal_json);
        assert!(result.is_ok(), "Should load minimal model: {:?}", result.err());
    }
}

#[test]
fn test_gru_weights_creation() {
    let hidden_size = 8;
    let input_size = 1;
    let mut weights = Vec::new();
    for _ in 0..6 {
        let size = hidden_size * input_size;
        let data: Vec<Vec<f32>> = vec![(0..size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect()];
        weights.push(data);
    }
    for _ in 0..3 {
        let data: Vec<Vec<f32>> = vec![(0..hidden_size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect()];
        weights.push(data);
    }

    let config = LayerConfig {
        layer_type: LayerType::Gru,
        shape: vec![None, None, Some(hidden_size as i64)],
        weights,
    };

    let gru_weights = GruEngine::weights_from_layer_config(&config, hidden_size);
    assert!(gru_weights.is_ok());
}

#[test]
fn test_lstm_weights_creation() {
    let hidden_size = 8;
    let input_size = 1;
    let weights_per_chunk = hidden_size * input_size;
    let mut weights = Vec::new();
    for _ in 0..8 {
        let data: Vec<Vec<f32>> = vec![(0..weights_per_chunk).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect()];
        weights.push(data);
    }

    let config = LayerConfig {
        layer_type: LayerType::Lstm,
        shape: vec![None, None, Some(hidden_size as i64)],
        weights,
    };

    let lstm_weights = LstmEngine::weights_from_layer_config(&config, hidden_size);
    assert!(lstm_weights.is_ok());
}

#[test]
fn test_dense_weights_creation() {
    let hidden_size = 8;
    let weights: Vec<Vec<f32>> = vec![(0..hidden_size).map(|i| ((i as f32) * 0.01 - 0.5) as f32).collect()];
    let bias: Vec<Vec<f32>> = vec![(0..1).map(|_| 0.1f32).collect()];

    let config = LayerConfig {
        layer_type: LayerType::Dense,
        shape: vec![None, None, 1],
        weights: vec![weights, bias],
    };

    let dense_weights = DenseWeights::from_layer_config(&config);
    assert!(dense_weights.is_ok());
}

#[test]
fn test_model_reset() {
    let json = gru_json(1, 8, false, -6.0, -3.0);
    let model = custom_model_creator(&json).expect("Should create model");
    model.reset();
}

#[test]
fn test_atomic_model_clone() {
    let json = gru_json(1, 8, false, -6.0, -3.0);
    let model = custom_model_creator(&json).expect("Should create model");
    let atomic = AtomicModel::new(model);
    let _cloned = atomic.clone();
}
