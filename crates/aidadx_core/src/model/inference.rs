//! Candle-backed, stateful inference for the RNN + Dense networks used by AIDA-X.
//!
//! Weight layout and math follow RTNeural (`gru/gru.h`, `lstm/lstm.h`,
//! `model_loader.h`), which reads the Keras-style export used by AIDA-X:
//!
//! * GRU:  `W[in][3H]`, `U[H][3H]`, `b[2][3H]`, gate order **z, r, h**;
//!   `b[0]` is the input bias, `b[1]` the recurrent bias ("reset_after" GRU):
//!   ```text
//!   z  = σ(W_z·x + b0_z + U_z·h + b1_z)
//!   r  = σ(W_r·x + b0_r + U_r·h + b1_r)
//!   h~ = tanh(W_h·x + b0_h + r ⊙ (U_h·h + b1_h))
//!   h' = (1 − z) ⊙ h~ + z ⊙ h
//!   ```
//! * LSTM: `W[in][4H]`, `U[H][4H]`, `b[4H]`, gate order **i, f, c, o**:
//!   ```text
//!   i = σ(..), f = σ(..), c~ = tanh(..), o = σ(..)
//!   c' = f ⊙ c + i ⊙ c~,   h' = o ⊙ tanh(c')
//!   ```
//! * Dense: `W[H][out]`, `b[out]`, `y = h·W + b`.
//!
//! Because the JSON stores `W` as `[in][gates*H]`, we keep the row-vector
//! convention `x(1×in) · W(in×gH)` and never transpose.

use crate::model::loader::validate_network;
use crate::model::types::{LayerConfig, LayerType, ModelArch, ModelConfig, ModelError, ModelResult, WeightArray};

use candle_core::{DType, Device, Tensor};

fn tensor(w: &WeightArray, rows: usize, cols: usize, dev: &Device) -> ModelResult<Tensor> {
    Ok(Tensor::from_slice(&w.data, (rows, cols), dev)?)
}

#[derive(Clone, Debug)]
enum RnnState {
    Gru {
        h: Tensor,
    },
    Lstm {
        h: Tensor,
        c: Tensor,
    },
}

/// Weights + state of one recurrent layer followed by a Dense(H → 1) layer.
#[derive(Clone, Debug)]
pub struct ModelInference {
    pub arch: ModelArch,
    input_size: usize,
    hidden_size: usize,
    /// Kernel weights, `in × gates·H`.
    w: Tensor,
    /// Recurrent weights, `H × gates·H`.
    u: Tensor,
    /// Input bias (`1 × gates·H`). For LSTM this is the only bias.
    b_in: Tensor,
    /// GRU recurrent bias (`1 × 3H`); unused for LSTM.
    b_rec: Option<Tensor>,
    dense_w: Tensor,
    dense_b: Tensor,
    state: RnnState,
    device: Device,
}

impl ModelInference {
    /// Builds the network from a parsed config. `arch` must have been derived from the same config.
    pub fn from_config(config: &ModelConfig, arch: ModelArch) -> ModelResult<Self> {
        let dev = Device::Cpu;
        let input = arch.input_size();
        let hidden = arch.hidden_size();
        let gates = arch.layer_type().gate_count();

        validate_network(config, arch)?;
        let rnn: &LayerConfig = &config.layers[0];
        let dense: &LayerConfig = &config.layers[1];

        let gh = gates * hidden;
        let w = tensor(&rnn.weights[0], input, gh, &dev)?;
        let u = tensor(&rnn.weights[1], hidden, gh, &dev)?;

        let (b_in, b_rec, state) = match arch.layer_type() {
            LayerType::Gru => {
                let b = &rnn.weights[2];
                let b_in = Tensor::from_slice(b.row(0), (1, gh), &dev)?;
                let b_rec = Tensor::from_slice(b.row(1), (1, gh), &dev)?;
                let h = Tensor::zeros((1, hidden), DType::F32, &dev)?;
                (b_in, Some(b_rec), RnnState::Gru { h })
            }
            LayerType::Lstm => {
                let b = &rnn.weights[2];
                let b_in = Tensor::from_slice(&b.data, (1, gh), &dev)?;
                let h = Tensor::zeros((1, hidden), DType::F32, &dev)?;
                let c = Tensor::zeros((1, hidden), DType::F32, &dev)?;
                (b_in, None, RnnState::Lstm { h, c })
            }
            LayerType::Dense => unreachable!("ModelArch is always recurrent"),
        };

        let dense_w = tensor(&dense.weights[0], hidden, 1, &dev)?;
        let dense_b = Tensor::from_slice(&dense.weights[1].data, (1, 1), &dev)?;

        Ok(Self {
            arch,
            input_size: input,
            hidden_size: hidden,
            w,
            u,
            b_in,
            b_rec,
            dense_w,
            dense_b,
            state,
            device: dev,
        })
    }

    pub fn input_size(&self) -> usize {
        self.input_size
    }

    pub fn hidden_size(&self) -> usize {
        self.hidden_size
    }

    /// Clears the recurrent state (RTNeural `reset()`).
    pub fn reset(&mut self) -> ModelResult<()> {
        let zeros = Tensor::zeros((1, self.hidden_size), DType::F32, &self.device)?;
        self.state = match self.state {
            RnnState::Gru { .. } => RnnState::Gru { h: zeros },
            RnnState::Lstm { .. } => RnnState::Lstm {
                h: zeros.clone(),
                c: zeros,
            },
        };
        Ok(())
    }

    /// Runs one time step. `input` must hold exactly `input_size()` values
    /// (`[audio]`, `[audio, param1]` or `[audio, param1, param2]`).
    /// Returns the Dense output and advances the recurrent state.
    pub fn forward(&mut self, input: &[f32]) -> ModelResult<f32> {
        if input.len() != self.input_size {
            return Err(ModelError::Unsupported(format!(
                "forward expects {} inputs, got {}",
                self.input_size,
                input.len()
            )));
        }
        let hs = self.hidden_size;
        let x = Tensor::from_slice(input, (1, self.input_size), &self.device)?;
        let xw = x.matmul(&self.w)?.broadcast_add(&self.b_in)?;

        let h_new = match &self.state {
            RnnState::Gru { h } => {
                let b_rec = self.b_rec.as_ref().expect("GRU always has a recurrent bias");
                let hu = h.matmul(&self.u)?.broadcast_add(b_rec)?;
                let zr = candle_nn::ops::sigmoid(&(xw.narrow(1, 0, 2 * hs)? + hu.narrow(1, 0, 2 * hs)?)?)?;
                let z = zr.narrow(1, 0, hs)?;
                let r = zr.narrow(1, hs, hs)?;
                let cand = (xw.narrow(1, 2 * hs, hs)? + (r * hu.narrow(1, 2 * hs, hs)?)?)?.tanh()?;
                // h' = (1 - z) * h~ + z * h  ==  h~ + z * (h - h~)
                let h_new = (&cand + (z * (h - &cand)?)?)?;
                self.state = RnnState::Gru { h: h_new.clone() };
                h_new
            }
            RnnState::Lstm { h, c } => {
                let g = (xw + h.matmul(&self.u)?)?;
                let i = candle_nn::ops::sigmoid(&g.narrow(1, 0, hs)?)?;
                let f = candle_nn::ops::sigmoid(&g.narrow(1, hs, hs)?)?;
                let cand = g.narrow(1, 2 * hs, hs)?.tanh()?;
                let o = candle_nn::ops::sigmoid(&g.narrow(1, 3 * hs, hs)?)?;
                let c_new = ((f * c)? + (i * cand)?)?;
                let h_new = (o * c_new.tanh()?)?;
                self.state = RnnState::Lstm {
                    h: h_new.clone(),
                    c: c_new,
                };
                h_new
            }
        };

        let y = h_new.matmul(&self.dense_w)?.add(&self.dense_b)?;
        Ok(y.flatten_all()?.to_vec1::<f32>()?[0])
    }
}
