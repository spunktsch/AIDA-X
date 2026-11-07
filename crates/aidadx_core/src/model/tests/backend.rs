//! Inference tests for the `DynamicModel` backend of this build (`BACKEND`:
//! RTNeural by default, candle when only `candle` is enabled).

use super::*;

// ---------------------------------------------------------------------------
// Inference vs. RTNeural reference
// ---------------------------------------------------------------------------

/// Loads all 54 architectures from JSON and checks the backend forward pass
/// against the scalar RTNeural transcription, sample by sample, including
/// recurrent state carried across steps and the param1/param2 inputs. Block
/// processing must give the same result as per-sample stepping.
#[test]
fn all_54_variants_match_rtneural_reference() {
    let n = 256;
    let signal = test_signal(n);
    let (p1, p2) = (0.3f32, -0.7f32);
    let mut count = 0;
    for arch in ModelArch::ALL {
        let json = model_json(arch.layer_type(), arch.hidden_size(), arch.input_size(), json!({}), 0x5eed + count);
        let mut model = custom_model_creator(&json.to_string()).unwrap();
        assert_eq!(model.arch, arch);
        assert_eq!(model.input_size(), arch.input_size());
        let mut reference = RefModel::from_json(&json);

        let inputs = [0.0, p1, p2];
        let mut got = Vec::with_capacity(n);
        let mut want = Vec::with_capacity(n);
        for &s in &signal {
            got.push(model.forward_sample(s, p1, p2).unwrap());
            let mut x = inputs;
            x[0] = s;
            want.push(reference.forward(&x[..arch.input_size()]));
        }
        let err = max_abs_diff(&got, &want);
        assert!(err < 1e-4, "{BACKEND} {arch}: max abs diff {err}");
        assert!(got.iter().all(|v| v.is_finite()));
        // Non-trivial output that actually depends on the input.
        let spread = got.iter().cloned().fold(f32::MIN, f32::max) - got.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread > 1e-3, "{arch}: output is constant");

        // Same thing through `process` (no gains, no skip) in one block.
        model.reset().unwrap();
        let mut block = signal.clone();
        model.process(&mut block, p1, p2).unwrap();
        let err = max_abs_diff(&block, &want);
        assert!(err < 1e-4, "{BACKEND} {arch} (block): max abs diff {err}");
        count += 1;
    }
    assert_eq!(count, 54);
}

/// The candle implementation stays available for offline use; keep it honest
/// even when `DynamicModel` runs on RTNeural.
#[cfg(feature = "candle")]
#[test]
fn candle_all_54_variants_match_rtneural_reference() {
    use crate::model::inference::ModelInference;

    let n = 64;
    let signal = test_signal(n);
    let (p1, p2) = (0.3f32, -0.7f32);
    for (i, arch) in ModelArch::ALL.into_iter().enumerate() {
        let json = model_json(arch.layer_type(), arch.hidden_size(), arch.input_size(), json!({}), 0xca + i as u64);
        let cfg = parse_model(&json.to_string()).unwrap();
        let mut net = ModelInference::from_config(&cfg, arch).unwrap();
        let mut reference = RefModel::from_json(&json);
        for &s in &signal {
            let x = [s, p1, p2];
            let got = net.forward(&x[..arch.input_size()]).unwrap();
            let want = reference.forward(&x[..arch.input_size()]);
            assert!((got - want).abs() < 1e-4, "candle {arch}: {got} vs {want}");
        }
    }
}

#[test]
fn params_affect_multi_input_models() {
    for lt in [LayerType::Gru, LayerType::Lstm] {
        let json = model_json(lt, 16, 3, json!({}), 99).to_string();
        let mut a = custom_model_creator(&json).unwrap();
        let mut b = a.clone();
        let mut buf_a = test_signal(64);
        let mut buf_b = buf_a.clone();
        a.process(&mut buf_a, 0.0, 0.0).unwrap();
        b.process(&mut buf_b, 1.0, 0.5).unwrap();
        assert!(max_abs_diff(&buf_a, &buf_b) > 1e-3, "{lt}: params ignored");
    }
}

#[test]
fn state_is_carried_and_reset() {
    let json = model_json(LayerType::Lstm, 12, 1, json!({}), 7).to_string();
    let mut model = custom_model_creator(&json).unwrap();
    let first: Vec<f32> = (0..4).map(|_| model.forward_sample(0.5, 0.0, 0.0).unwrap()).collect();
    assert!((first[0] - first[3]).abs() > 1e-6, "recurrent state not updated");
    model.reset().unwrap();
    let again: Vec<f32> = (0..4).map(|_| model.forward_sample(0.5, 0.0, 0.0).unwrap()).collect();
    assert_eq!(first, again);
}

/// A clone is an independent model with the same weights; models are only
/// cloned before they run, so a fresh (reset) state is what callers get.
#[test]
fn clone_is_independent() {
    let json = model_json(LayerType::Gru, 24, 2, json!({"in_gain": 2.0}), 11).to_string();
    let mut a = custom_model_creator(&json).unwrap();
    let mut b = a.clone();
    assert_eq!((a.arch, a.input_gain, a.output_gain), (b.arch, b.input_gain, b.output_gain));

    let sig = test_signal(300);
    let mut ya = sig.clone();
    a.process(&mut ya, 0.1, 0.0).unwrap();
    let mut yb = sig.clone();
    b.process(&mut yb, 0.1, 0.0).unwrap();
    assert_eq!(ya, yb);

    // Advancing `a` does not affect `b`.
    let mut junk = sig.clone();
    a.process(&mut junk, 0.9, 0.0).unwrap();
    a.reset().unwrap();
    b.reset().unwrap();
    let (mut za, mut zb) = (sig.clone(), sig.clone());
    a.process(&mut za, 0.1, 0.0).unwrap();
    b.process(&mut zb, 0.1, 0.0).unwrap();
    assert_eq!(za, zb);
}

/// Real AIDA-X model from `files/`: compare against the reference and against
/// the `output_batch` the exporter stored for `input_batch`.
#[test]
fn real_model_inference() {
    let text = std::fs::read_to_string(REAL_MODEL).expect("example model in files/");
    let root: Value = serde_json::from_str(&text).unwrap();
    let mut model = DynamicModel::from_json(&text).unwrap();
    assert_eq!(model.arch, ModelArch::Lstm12_1);
    assert!(!model.input_skip);
    assert_eq!((model.input_gain, model.output_gain), (1.0, 1.0));

    let input = vec_of(&root["input_batch"]);
    let expected = vec_of(&root["output_batch"]);
    assert!(input.len() > 100 && input.len() == expected.len());
    let mut reference = RefModel::from_json(&root);

    let mut out = input.clone();
    model.process(&mut out, 0.0, 0.0).unwrap();
    let want: Vec<f32> = input.iter().map(|&x| reference.forward(&[x])).collect();

    assert!(out.iter().all(|v| v.is_finite()));
    let rms = (out.iter().map(|v| v * v).sum::<f32>() / out.len() as f32).sqrt();
    assert!(rms > 1e-3, "output is (near) silent: rms {rms}");
    let err_ref = max_abs_diff(&out, &want);
    assert!(err_ref < 1e-4, "{BACKEND} vs RTNeural reference: {err_ref}");
    let err_batch = max_abs_diff(&out, &expected);
    println!("{BACKEND}: max abs diff vs reference {err_ref:e}, vs output_batch {err_batch:e}");
    assert!(err_batch < 1e-3, "{BACKEND} vs exported output_batch: {err_batch}");

    // A musical signal through the same model is non-trivial too.
    model.reset().unwrap();
    let mut sig = test_signal(4800);
    model.process(&mut sig, 0.0, 0.0).unwrap();
    assert!(sig.iter().all(|v| v.is_finite()));
    let rms = (sig.iter().map(|v| v * v).sum::<f32>() / sig.len() as f32).sqrt();
    assert!(rms > 1e-3, "rms {rms}");

    // prewarm == reset + 2048 zeros, then the same output as above.
    model.prewarm(0.0, 0.0).unwrap();
    let mut a = test_signal(256);
    model.process(&mut a, 0.0, 0.0).unwrap();
    model.reset().unwrap();
    let mut zeros = [0.0f32; 2048];
    model.process(&mut zeros, 0.0, 0.0).unwrap();
    let mut b = test_signal(256);
    model.process(&mut b, 0.0, 0.0).unwrap();
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Gains / skip (applyModel semantics)
// ---------------------------------------------------------------------------

#[test]
fn process_applies_gain_and_skip_like_apply_model() {
    for skip in [0, 1] {
        let json = model_json(LayerType::Gru, 8, 2, json!({"in_skip": skip, "in_gain": -6.0, "out_gain": 4.0}), 3);
        let mut model = custom_model_creator(&json.to_string()).unwrap();
        assert_eq!(model.input_skip, skip == 1);
        let (ig, og) = (db_to_coef(-6.0), db_to_coef(4.0));
        assert!((model.input_gain - ig).abs() < 1e-6);
        assert!((model.output_gain - og).abs() < 1e-6);

        let mut reference = RefModel::from_json(&json);
        // Longer than the internal skip scratch block, and not a multiple of it.
        let input = test_signal(1000);
        let mut out = input.clone();
        model.process(&mut out, 0.25, 0.0).unwrap();

        let want: Vec<f32> = input
            .iter()
            .map(|&x| {
                let x = x * ig;
                let y = reference.forward(&[x, 0.25]);
                if skip == 1 {
                    (x + y) * og
                } else {
                    y * og
                }
            })
            .collect();
        let err = max_abs_diff(&out, &want);
        assert!(err < 1e-4, "skip={skip}: {err}");
    }
}

// ---------------------------------------------------------------------------
// Thread-safe swap
// ---------------------------------------------------------------------------

#[test]
fn atomic_model_swap() {
    let slot = AtomicModel::new();
    let mut active: Option<Box<DynamicModel>> = None;
    assert!(!slot.take_pending(&mut active));

    let m8 = custom_model_creator(&model_json(LayerType::Gru, 8, 1, json!({}), 1).to_string()).unwrap();
    let m12 = custom_model_creator(&model_json(LayerType::Lstm, 12, 1, json!({}), 2).to_string()).unwrap();
    let m16 = custom_model_creator(&model_json(LayerType::Gru, 16, 1, json!({}), 3).to_string()).unwrap();

    slot.publish(m8);
    assert!(slot.has_pending());
    assert!(slot.take_pending(&mut active));
    assert_eq!(active.as_ref().unwrap().arch, ModelArch::Gru8_1);

    // Latest publish wins if the audio thread has not picked up the previous one.
    slot.publish(m12);
    slot.publish(m16);
    assert!(slot.take_pending(&mut active));
    assert_eq!(active.as_ref().unwrap().arch, ModelArch::Gru16_1);
    assert!(!slot.take_pending(&mut active));
    slot.collect_garbage();
}

#[test]
fn atomic_model_swap_across_threads() {
    use std::sync::Arc;

    let slot = Arc::new(AtomicModel::new());
    let archs = [(LayerType::Gru, 8), (LayerType::Lstm, 8), (LayerType::Gru, 12), (LayerType::Lstm, 16)];
    let models: Vec<DynamicModel> = archs
        .iter()
        .enumerate()
        .map(|(i, &(lt, h))| custom_model_creator(&model_json(lt, h, 1, json!({}), i as u64 + 10).to_string()).unwrap())
        .collect();
    let last_arch = models.last().unwrap().arch;

    let producer = {
        let slot = Arc::clone(&slot);
        std::thread::spawn(move || {
            for m in models {
                slot.publish(m);
                std::thread::yield_now();
            }
        })
    };

    // "Audio thread": keeps processing while models are swapped in.
    let mut active: Option<Box<DynamicModel>> = None;
    let mut buf = [0.0f32; 32];
    let mut swaps = 0;
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if slot.take_pending(&mut active) {
            swaps += 1;
        }
        if let Some(m) = active.as_mut() {
            buf.iter_mut().enumerate().for_each(|(i, s)| *s = (i as f32 * 0.1).sin());
            m.process(&mut buf, 0.0, 0.0).unwrap();
            assert!(buf.iter().all(|v| v.is_finite()));
        }
        // Stands in for the loader thread's periodic GC: without it a swap deferred
        // behind a full retired slot would never go through (TICKET-024).
        slot.collect_garbage();
        if producer.is_finished() && !slot.has_pending() {
            break;
        }
        assert!(Instant::now() < deadline, "swap test timed out");
    }
    producer.join().unwrap();
    assert!(swaps >= 1);
    assert_eq!(active.unwrap().arch, last_arch);
}

// ---------------------------------------------------------------------------
// Throughput (run with:
//   cargo test --release -p aidadx_core model::tests::backend::throughput -- --ignored --nocapture)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn throughput() {
    const SR: usize = 48_000;
    const BLOCK: usize = 128;
    let seconds = 5;
    let signal = test_signal(SR * seconds);

    let mut cases: Vec<(String, Value)> = Vec::new();
    let real: Value = serde_json::from_str(&std::fs::read_to_string(REAL_MODEL).unwrap()).unwrap();
    cases.push(("LSTM_12_1 (tw40 real)".into(), real));
    for (lt, h) in [
        (LayerType::Lstm, 12),
        (LayerType::Gru, 8),
        (LayerType::Gru, 40),
        (LayerType::Lstm, 40),
        (LayerType::Gru, 80),
        (LayerType::Lstm, 80),
    ] {
        cases.push((format!("{}_{}_1", lt.to_string().to_uppercase(), h), model_json(lt, h, 1, json!({}), 42)));
    }

    println!(
        "\nbackend = {BACKEND}, {}, block = {BLOCK}, {seconds} s of audio at {SR} Hz",
        if cfg!(debug_assertions) { "DEBUG build (numbers not meaningful)" } else { "release" }
    );
    println!("{:<24} {:>14} {:>10} {:>16} {:>10}", "model", "backend smp/s", "x RT", "scalar ref smp/s", "x RT");
    let mut lstm12_rt = f64::INFINITY;
    for (name, json) in cases {
        let mut model = custom_model_creator(&json.to_string()).unwrap();
        model.prewarm(0.0, 0.0).unwrap();
        let mut buf = signal.clone();
        let t = Instant::now();
        for block in buf.chunks_mut(BLOCK) {
            model.process(block, 0.0, 0.0).unwrap();
        }
        let backend_sps = signal.len() as f64 / t.elapsed().as_secs_f64();
        assert!(buf.iter().all(|v| v.is_finite()));

        let mut reference = RefModel::from_json(&json);
        let ref_n = SR / 4;
        let t = Instant::now();
        let mut acc = 0.0f32;
        for &s in &signal[..ref_n] {
            acc += reference.forward(&[s]);
        }
        let scalar_sps = ref_n as f64 / t.elapsed().as_secs_f64();
        assert!(acc.is_finite());

        let rt = backend_sps / SR as f64;
        if name.starts_with("LSTM_12_1") {
            lstm12_rt = lstm12_rt.min(rt);
        }
        println!(
            "{:<24} {:>14.0} {:>10.1} {:>16.0} {:>10.1}",
            name,
            backend_sps,
            rt,
            scalar_sps,
            scalar_sps / SR as f64
        );
    }
    if BACKEND == "rtneural" && !cfg!(debug_assertions) {
        assert!(lstm12_rt >= 20.0, "LSTM_12_1 only {lstm12_rt:.1}x realtime (target >= 20x)");
    }
}
