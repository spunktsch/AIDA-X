//! Deterministic synthetic models for tests (unit and integration). Not part of the API.

/// An LSTM(`hidden`) + Dense(1) model with `input` inputs (1 = snapshot, 2/3 = PARAM1/
/// PARAM2 conditioned) in the Keras/AIDA-X JSON format, with pseudo-random weights.
pub fn lstm_json(input: usize, hidden: usize, seed: u64) -> String {
    let mut state = seed | 1;
    let mut next = move |scale: f32| {
        // xorshift64*
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let v = (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32;
        (v * 2.0 - 1.0) * scale
    };
    let mut mat = |rows: usize, cols: usize, scale: f32| {
        let rows: Vec<String> = (0..rows)
            .map(|_| {
                let r: Vec<String> = (0..cols).map(|_| next(scale).to_string()).collect();
                format!("[{}]", r.join(","))
            })
            .collect();
        format!("[{}]", rows.join(","))
    };
    let g = 4 * hidden;
    let scale = 1.5 / (hidden as f32).sqrt();
    let kernel = mat(input, g, 1.0);
    let recurrent = mat(hidden, g, scale);
    let bias = mat(1, g, 0.3);
    let bias = &bias[1..bias.len() - 1]; // 1-D
    let dense = mat(hidden, 1, scale);
    let dense_bias = mat(1, 1, 0.1);
    let dense_bias = &dense_bias[1..dense_bias.len() - 1];
    format!(
        r#"{{"in_shape":[null,null,{input}],"layers":[
{{"type":"lstm","activation":"","shape":[null,null,{hidden}],"weights":[{kernel},{recurrent},{bias}]}},
{{"type":"dense","activation":"","shape":[null,null,1],"weights":[{dense},{dense_bias}]}}]}}"#
    )
}
