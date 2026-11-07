//! CPU cost of the full engine (default model + default cabinet, every stage on) per
//! 128-sample block at 48 kHz. Ignored by default; run with
//! `cargo test --release -p aidadx_plugin --test realtime_bench -- --ignored --nocapture`.

use std::time::Instant;

use aidadx_plugin::loader::{load_cabinet, load_model, FileSource};
use aidadx_plugin::{AidaEngine, ParamSnapshot, PluginOutputs};

#[test]
#[ignore]
fn engine_cpu_per_128_sample_block() {
    const SR: f64 = 48_000.0;
    const BLOCK: usize = 128;
    const BLOCKS: usize = 20_000; // ~53 s of audio

    let params = ParamSnapshot {
        antialiasing: 50.0,
        eq_bypass: false,
        ..ParamSnapshot::default()
    };
    let mut engine = AidaEngine::new();
    engine.prepare(SR, BLOCK, &params);
    engine
        .model_slot()
        .publish(Box::new(load_model(&FileSource::Default).unwrap()));
    engine
        .cabinet_slot()
        .publish(Box::new(load_cabinet(&FileSource::Default, SR).unwrap()));
    let outputs = PluginOutputs::default();

    let mut x = 1u32;
    let input: Vec<f32> = (0..BLOCK * 64)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((x >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.5
        })
        .collect();
    let mut buf = [0.0f32; BLOCK];
    let mut run = |n: usize| {
        for i in 0..n {
            let off = (i % 64) * BLOCK;
            buf.copy_from_slice(&input[off..off + BLOCK]);
            engine.process(&mut buf, &outputs);
        }
    };
    run(1000); // warm-up

    let start = Instant::now();
    run(BLOCKS);
    let elapsed = start.elapsed().as_secs_f64();
    let per_block_us = elapsed / BLOCKS as f64 * 1e6;
    let budget_us = BLOCK as f64 / SR * 1e6;
    println!(
        "engine: {per_block_us:.2} us per {BLOCK}-sample block ({:.2} % of the {budget_us:.0} us real-time budget, {:.0}x real time)",
        100.0 * per_block_us / budget_us,
        budget_us / per_block_us
    );
}
