use crate::model::types::{LayerConfig, LayerType, ModelArch};

use candle_core::{D, Device, Tensor};
use candle_nn::VarDict;

#[derive(Clone)]
pub struct GruWeights {
    pub i_r: Tensor,
    pub i_z: Tensor,
    pub i_h: Tensor,
    pub h_r: Tensor,
    pub h_z: Tensor,
    pub h_h: Tensor,
    pub b_r: Tensor,
    pub b_z: Tensor,
    pub b_h: Tensor,
}

impl GruWeights {
    pub fn from_layer_config(config: &LayerConfig, hidden_size: usize) -> Result<Self, String> {
        let weights = &config.weights;
        let total = weights.len();
        if total < 6 || total > 9 {
            return Err(format!(
                "GRU layer expects 6-9 weight matrices, got {}",
                total
            ));
        }

        let dev = Device::Cpu;

        let to_tensor = |flat: Vec<f32>, shape: &[usize]| {
            let shape: Vec<usize> = shape.iter().map(|&x| x as usize).collect();
            Tensor::from_vec(flat, shape, &dev)
                .map_err(|e| format!("Tensor creation error: {}", e))
        };

        let flatten = |mat: &Vec<Vec<f32>>| -> Result<Vec<f32>, String> {
            let mut flat = Vec::with_capacity(mat.len() * mat.first().map(|r| r.len()).unwrap_or(0));
            for row in mat {
                flat.extend_from_slice(row);
            }
            Ok(flat)
        };

        let i_r_flat = flatten(&weights[0])?;
        let i_z_flat = flatten(&weights[1])?;
        let i_h_flat = flatten(&weights[2])?;
        let h_r_flat = flatten(&weights[3])?;
        let h_z_flat = flatten(&weights[4])?;
        let h_h_flat = flatten(&weights[5])?;

        let i_r_t = to_tensor(i_r_flat, &[hidden_size, config.input_size()])?.transpose(0, 1)?;
        let i_z_t = to_tensor(i_z_flat, &[hidden_size, config.input_size()])?.transpose(0, 1)?;
        let i_h_t = to_tensor(i_h_flat, &[hidden_size, config.input_size()])?.transpose(0, 1)?;

        let h_r_t = to_tensor(h_r_flat, &[hidden_size, hidden_size])?.transpose(0, 1)?;
        let h_z_t = to_tensor(h_z_flat, &[hidden_size, hidden_size])?.transpose(0, 1)?;
        let h_h_t = to_tensor(h_h_flat, &[hidden_size, hidden_size])?.transpose(0, 1)?;

        let b_r_t = if total > 6 {
            to_tensor(flatten(&weights[6])?, &[1, hidden_size])?
        } else {
            Tensor::zeros([1, hidden_size], D::F32, &dev)?
        };
        let b_z_t = if total > 7 {
            to_tensor(flatten(&weights[7])?, &[1, hidden_size])?
        } else {
            Tensor::zeros([1, hidden_size], D::F32, &dev)?
        };
        let b_h_t = if total > 8 {
            to_tensor(flatten(&weights[8])?, &[1, hidden_size])?
        } else {
            Tensor::zeros([1, hidden_size], D::F32, &dev)?
        };

        Ok(GruWeights {
            i_r: i_r_t,
            i_z: i_z_t,
            i_h: i_h_t,
            h_r: h_r_t,
            h_z: h_z_t,
            h_h: h_h_t,
            b_r: b_r_t,
            b_z: b_z_t,
            b_h: b_h_t,
        })
    }

    pub fn to_var_dict(&self, vd: &mut VarDict) {
        vd.set_tensor("i_r", self.i_r.clone());
        vd.set_tensor("i_z", self.i_z.clone());
        vd.set_tensor("i_h", self.i_h.clone());
        vd.set_tensor("h_r", self.h_r.clone());
        vd.set_tensor("h_z", self.h_z.clone());
        vd.set_tensor("h_h", self.h_h.clone());
        vd.set_tensor("b_r", self.b_r.clone());
        vd.set_tensor("b_z", self.b_z.clone());
        vd.set_tensor("b_h", self.b_h.clone());
    }
}

#[derive(Clone)]
pub struct LstmWeights {
    pub i_i: Tensor,
    pub i_f: Tensor,
    pub i_o: Tensor,
    pub i_c: Tensor,
    pub h_i: Tensor,
    pub h_f: Tensor,
    pub h_o: Tensor,
    pub h_c: Tensor,
    pub b_i: Tensor,
    pub b_f: Tensor,
    pub b_o: Tensor,
    pub b_c: Tensor,
}

impl LstmWeights {
    pub fn from_layer_config(config: &LayerConfig, hidden_size: usize) -> Result<Self, String> {
        let weights = &config.weights;
        let dev = Device::Cpu;

        let to_tensor = |flat: Vec<f32>, shape: &[usize]| {
            let shape: Vec<usize> = shape.iter().map(|&x| x as usize).collect();
            Tensor::from_vec(flat, shape, &dev)
                .map_err(|e| format!("Tensor creation error: {}", e))
        };

        let flatten = |mat: &Vec<Vec<f32>>| -> Result<Vec<f32>, String> {
            let mut flat = Vec::with_capacity(mat.len() * mat.first().map(|r| r.len()).unwrap_or(0));
            for row in mat {
                flat.extend_from_slice(row);
            }
            Ok(flat)
        };

        let input_size = config.input_size();
        let i_i_t = to_tensor(flatten(&weights[0])?, &[hidden_size, input_size])?.transpose(0, 1)?;
        let i_f_t = to_tensor(flatten(&weights[1])?, &[hidden_size, input_size])?.transpose(0, 1)?;
        let i_o_t = to_tensor(flatten(&weights[2])?, &[hidden_size, input_size])?.transpose(0, 1)?;
        let i_c_t = to_tensor(flatten(&weights[3])?, &[hidden_size, input_size])?.transpose(0, 1)?;

        let h_i_t = to_tensor(flatten(&weights[4])?, &[hidden_size, hidden_size])?.transpose(0, 1)?;
        let h_f_t = to_tensor(flatten(&weights[5])?, &[hidden_size, hidden_size])?.transpose(0, 1)?;
        let h_o_t = to_tensor(flatten(&weights[6])?, &[hidden_size, hidden_size])?.transpose(0, 1)?;
        let h_c_t = to_tensor(flatten(&weights[7])?, &[hidden_size, hidden_size])?.transpose(0, 1)?;

        let b_flat = flatten(&weights[8])?;
        let b_i_t = to_tensor(b_flat.clone(), &[1, hidden_size])?;
        let b_f_t = to_tensor(b_flat.clone(), &[1, hidden_size])?;
        let b_o_t = to_tensor(b_flat.clone(), &[1, hidden_size])?;
        let b_c_t = to_tensor(b_flat, &[1, hidden_size])?;

        Ok(LstmWeights {
            i_i: i_i_t,
            i_f: i_f_t,
            i_o: i_o_t,
            i_c: i_c_t,
            h_i: h_i_t,
            h_f: h_f_t,
            h_o: h_o_t,
            h_c: h_c_t,
            b_i: b_i_t,
            b_f: b_f_t,
            b_o: b_o_t,
            b_c: b_c_t,
        })
    }

    pub fn to_var_dict(&self, vd: &mut VarDict) {
        vd.set_tensor("i_i", self.i_i.clone());
        vd.set_tensor("i_f", self.i_f.clone());
        vd.set_tensor("i_o", self.i_o.clone());
        vd.set_tensor("i_c", self.i_c.clone());
        vd.set_tensor("h_i", self.h_i.clone());
        vd.set_tensor("h_f", self.h_f.clone());
        vd.set_tensor("h_o", self.h_o.clone());
        vd.set_tensor("h_c", self.h_c.clone());
        vd.set_tensor("b_i", self.b_i.clone());
        vd.set_tensor("b_f", self.b_f.clone());
        vd.set_tensor("b_o", self.b_o.clone());
        vd.set_tensor("b_c", self.b_c.clone());
    }
}

#[derive(Clone)]
pub struct DenseWeights {
    pub weight: Tensor,
    pub bias: Tensor,
}

impl DenseWeights {
    pub fn from_layer_config(config: &LayerConfig) -> Result<Self, String> {
        let weights = &config.weights;
        if weights.len() != 2 {
            return Err(format!(
                "Dense layer expects 2 weight arrays, got {}",
                weights.len()
            ));
        }

        let dev = Device::Cpu;

        let flatten = |mat: &Vec<Vec<f32>>| -> Vec<f32> {
            let mut flat = Vec::with_capacity(mat.len() * mat.first().map(|r| r.len()).unwrap_or(0));
            for row in mat {
                flat.extend_from_slice(row);
            }
            flat
        };

        let hidden_size = config.hidden_size();
        let output_size = 1;

        let weight = Tensor::from_vec(flatten(&weights[0]), [hidden_size, output_size], &dev)?;
        let bias = Tensor::from_vec(flatten(&weights[1]), [1, output_size], &dev)?;

        Ok(DenseWeights { weight, bias })
    }

    pub fn to_var_dict(&self, vd: &mut VarDict) {
        vd.set_tensor("weight", self.weight.clone());
        vd.set_tensor("bias", self.bias.clone());
    }
}

pub struct GruEngine {
    pub weights: GruWeights,
    pub hidden_size: usize,
    pub(crate) device: Device,
}

impl GruEngine {
    pub fn new(weights: GruWeights, hidden_size: usize) -> Self {
        Self {
            weights,
            hidden_size,
            device: Device::Cpu,
        }
    }

    pub fn weights_from_layer_config(config: &LayerConfig, hidden_size: usize) -> Result<GruWeights, String> {
        GruWeights::from_layer_config(config, hidden_size)
    }

    pub fn forward(&self, input: &Tensor, hx: &Tensor) -> Result<(Tensor, Tensor), String> {
        let ir = self.weights.i_r.matmul(input)?;
        let iz = self.weights.i_z.matmul(input)?;
        let ih = self.weights.i_h.matmul(input)?;

        let hr = self.weights.h_r.matmul(hx)?;
        let hz = self.weights.h_z.matmul(hx)?;
        let hh = self.weights.h_h.matmul(hx)?;

        let br = &self.weights.b_r;
        let bz = &self.weights.b_z;
        let bh = &self.weights.b_h;

        let r = ((&ir + &hr + br).sigmoid()?)?;
        let z = ((&iz + &hz + bz).sigmoid()?)?;
        let h_candidate = ((&ih + (&r * &hh)? + bh).tanh()?)?;

        let hx_new = (&z * hx)? + ((1.0 - &z)? * &h_candidate)?;

        Ok((hx_new.clone(), hx_new))
    }
}

pub struct LstmEngine {
    pub weights: LstmWeights,
    pub hidden_size: usize,
    pub(crate) device: Device,
}

impl LstmEngine {
    pub fn new(weights: LstmWeights, hidden_size: usize) -> Self {
        Self {
            weights,
            hidden_size,
            device: Device::Cpu,
        }
    }

    pub fn weights_from_layer_config(config: &LayerConfig, hidden_size: usize) -> Result<LstmWeights, String> {
        LstmWeights::from_layer_config(config, hidden_size)
    }

    pub fn forward(
        &self,
        input: &Tensor,
        hx: &Tensor,
        cx: &Tensor,
    ) -> Result<(Tensor, Tensor, Tensor), String> {
        let ii = self.weights.i_i.matmul(input)?;
        let if_ = self.weights.i_f.matmul(input)?;
        let io = self.weights.i_o.matmul(input)?;
        let ic = self.weights.i_c.matmul(input)?;

        let hi = self.weights.h_i.matmul(hx)?;
        let hf = self.weights.h_f.matmul(hx)?;
        let ho = self.weights.h_o.matmul(hx)?;
        let hc = self.weights.h_c.matmul(hx)?;

        let bi = &self.weights.b_i;
        let bf = &self.weights.b_f;
        let bo = &self.weights.b_o;
        let bc = &self.weights.b_c;

        let i_gate = ((&ii + &hi + bi).sigmoid()?)?;
        let f_gate = ((&if_ + &hf + bf).sigmoid()?)?;
        let o_gate = ((&io + &ho + bo).sigmoid()?)?;
        let c_candidate = ((&ic + &hc + bc).tanh()?)?;

        let cx_new = (&f_gate * cx)? + (&i_gate * &c_candidate)?;
        let h_new = (&o_gate * (&cx_new).tanh()?)?;

        Ok((h_new, cx_new, h_new))
    }
}

pub struct DenseEngine {
    pub weights: DenseWeights,
    device: Device,
}

impl DenseEngine {
    pub fn new(weights: DenseWeights) -> Self {
        Self {
            weights,
            device: Device::Cpu,
        }
    }

    pub fn forward(&self, input: &Tensor) -> Result<Tensor, String> {
        let out = self.weights.weight.matmul(input)?;
        Ok((&out + &self.weights.bias)?)
    }
}

pub struct ModelInference {
    pub arch: ModelArch,
    pub gru_engine: Option<GruEngine>,
    pub lstm_engine: Option<LstmEngine>,
    pub dense_engine: Option<DenseEngine>,
}

impl ModelInference {
    pub fn from_config(config: &ModelConfig, arch: ModelArch) -> Result<Self, String> {
        if config.layers.len() < 2 {
            return Err("Model must have at least 2 layers (RNN + Dense)".to_string());
        }

        let mut gru_engine = None;
        let mut lstm_engine = None;
        let mut dense_engine = None;

        for layer in &config.layers {
            match layer.layer_type {
                LayerType::Gru => {
                    let hs = layer.hidden_size();
                    let weights = GruWeights::from_layer_config(layer, hs)?;
                    gru_engine = Some(GruEngine::new(weights, hs));
                }
                LayerType::Lstm => {
                    let hs = layer.hidden_size();
                    let weights = LstmWeights::from_layer_config(layer, hs)?;
                    lstm_engine = Some(LstmEngine::new(weights, hs));
                }
                LayerType::Dense => {
                    let weights = DenseWeights::from_layer_config(layer)?;
                    dense_engine = Some(DenseEngine::new(weights));
                }
            }
        }

        if gru_engine.is_none() && lstm_engine.is_none() {
            return Err("Model must have a GRU or LSTM layer".to_string());
        }

        if dense_engine.is_none() {
            return Err("Model must have a Dense output layer".to_string());
        }

        Ok(ModelInference {
            arch,
            gru_engine,
            lstm_engine,
            dense_engine,
        })
    }

    pub fn forward(
        &self,
        input: &[f32],
        param1: f32,
        param2: f32,
    ) -> Result<[f32; 1], String> {
        let dev = Device::Cpu;
        let input_size = self.arch.input_size();

        let mut in_data = vec![0f32; input_size];
        for (i, val) in input.iter().take(input_size).enumerate() {
            in_data[i] = *val;
        }

        if input_size == 2 {
            in_data[1] = param1;
        } else if input_size == 3 {
            in_data[1] = param1;
            in_data[2] = param2;
        }

        let x = Tensor::from_vec(in_data, [1, input_size], &dev)?;

        let (h_out, _) = if let Some(ref gru) = self.gru_engine {
            let hx = Tensor::zeros([1, gru.hidden_size], candle_core::DType::F32, &dev)?;
            gru.forward(&x, &hx)?
        } else if let Some(ref lstm) = self.lstm_engine {
            let hx = Tensor::zeros([1, lstm.hidden_size], candle_core::DType::F32, &dev)?;
            let cx = Tensor::zeros([1, lstm.hidden_size], candle_core::DType::F32, &dev)?;
            lstm.forward(&x, &hx, &cx)?
        } else {
            return Err("No RNN engine available".to_string());
        };

        let output = if let Some(ref dense) = self.dense_engine {
            dense.forward(&h_out)?
        } else {
            return Err("No dense engine available".to_string());
        };

        let flat = output.to_vec1::<f32>()?;
        Ok([flat[0]])
    }
}
