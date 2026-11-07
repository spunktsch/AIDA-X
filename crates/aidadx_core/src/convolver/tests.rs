//! Tests for the FFT convolution engine.

use super::fft::{Bin, RealFft};
use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deterministic white noise in [-1, 1) (xorshift32).
fn noise(len: usize, seed: u32) -> Vec<Sample> {
    let mut state = seed.max(1);
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f64 / u32::MAX as f64 * 2.0 - 1.0) as Sample
        })
        .collect()
}

/// Cabinet-like IR: exponentially decaying noise, last sample non-zero.
fn decaying_ir(len: usize, seed: u32) -> Vec<Sample> {
    let mut ir = noise(len, seed);
    for (i, s) in ir.iter_mut().enumerate() {
        *s *= (-4.0 * i as f32 / len as f32).exp();
    }
    if let Some(last) = ir.last_mut() {
        *last = 0.01;
    }
    ir
}

/// Reference: direct time-domain convolution, truncated to `x.len()`.
fn naive_convolve(x: &[Sample], h: &[Sample]) -> Vec<Sample> {
    (0..x.len())
        .map(|n| {
            let kmax = n.min(h.len().saturating_sub(1));
            (0..=kmax)
                .map(|k| h[k] as f64 * x[n - k] as f64)
                .sum::<f64>() as Sample
        })
        .collect()
}

/// Feed `input` through `process` in chunks of `block` samples.
fn run_blocks(
    input: &[Sample],
    block: usize,
    mut process: impl FnMut(&[Sample], &mut [Sample]),
) -> Vec<Sample> {
    let mut out = vec![0.0; input.len()];
    for (i, o) in input.chunks(block).zip(out.chunks_mut(block)) {
        process(i, o);
    }
    out
}

fn assert_close(actual: &[Sample], expected: &[Sample], ctx: &str) {
    assert_eq!(actual.len(), expected.len(), "{ctx}: length");
    let peak = expected.iter().fold(1.0f32, |m, v| m.max(v.abs()));
    let tol = 2e-5 * peak;
    for (n, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (a - e).abs() <= tol,
            "{ctx}: sample {n}: got {a}, expected {e} (tol {tol})"
        );
    }
}

// ---------------------------------------------------------------------------
// FFT wrapper
// ---------------------------------------------------------------------------

#[test]
fn fft_roundtrip() {
    let n = 256;
    let mut fft = RealFft::new(n);
    let signal = noise(n, 7);
    let mut buf = signal.clone();
    let mut spec = vec![Bin::default(); fft.complex_size()];
    fft.forward(&mut buf, &mut spec);
    let mut out = vec![0.0; n];
    fft.inverse(&mut spec, &mut out);
    for (o, s) in out.iter().zip(&signal) {
        assert!((o / n as f32 - s).abs() < 1e-5);
    }
}

#[test]
fn fft_dc_component() {
    let n = 64;
    let mut fft = RealFft::new(n);
    let mut buf = vec![1.0; n];
    let mut spec = vec![Bin::default(); fft.complex_size()];
    fft.forward(&mut buf, &mut spec);
    assert!((spec[0].re - n as f32).abs() < 1e-4);
    assert!(spec[1..].iter().all(|b| b.norm() < 1e-4));
}

// ---------------------------------------------------------------------------
// FftConvolver (single stage)
// ---------------------------------------------------------------------------

#[test]
fn fft_convolver_rejects_zero_block_size() {
    let mut conv = FftConvolver::new();
    assert_eq!(conv.init(0, &[1.0]), Err(ConvolverError::InvalidBlockSize));
}

#[test]
fn fft_convolver_rounds_block_size() {
    let mut conv = FftConvolver::new();
    conv.init(100, &decaying_ir(300, 1)).unwrap();
    assert_eq!(conv.block_size(), 128);
    assert_eq!(conv.segment_count(), 3);
}

#[test]
fn fft_convolver_empty_and_silent_ir_output_silence() {
    for ir in [vec![], vec![0.0; 500], vec![1e-9; 10]] {
        let mut conv = FftConvolver::new();
        conv.init(64, &ir).unwrap();
        assert!(!conv.is_initialized());
        let mut out = vec![1.0; 100];
        conv.process(&noise(100, 3), &mut out);
        assert!(out.iter().all(|&s| s == 0.0));
    }
}

#[test]
fn fft_convolver_uninitialized_outputs_silence() {
    let mut conv = FftConvolver::new();
    let mut out = vec![1.0; 32];
    conv.process(&[0.5; 32], &mut out);
    assert!(out.iter().all(|&s| s == 0.0));
}

#[test]
fn fft_convolver_trims_trailing_zeros() {
    let mut ir = decaying_ir(130, 2);
    ir.extend(std::iter::repeat(0.0).take(1000));
    let mut conv = FftConvolver::new();
    conv.init(64, &ir).unwrap();
    assert_eq!(conv.segment_count(), 3);
}

#[test]
fn fft_convolver_delta_is_identity() {
    let mut conv = FftConvolver::new();
    conv.init(128, &[1.0]).unwrap();
    let input = noise(1000, 11);
    let out = run_blocks(&input, 37, |i, o| conv.process(i, o));
    assert_close(&out, &input, "delta");
}

#[test]
fn fft_convolver_matches_naive() {
    for &ir_len in &[1usize, 5, 63, 64, 65, 200, 1000] {
        for &call in &[1usize, 7, 32, 64, 100, 257] {
            let ir = decaying_ir(ir_len, ir_len as u32);
            let input = noise(ir_len + 700, 42);
            let mut conv = FftConvolver::new();
            conv.init(64, &ir).unwrap();
            let out = run_blocks(&input, call, |i, o| conv.process(i, o));
            assert_close(
                &out,
                &naive_convolve(&input, &ir),
                &format!("single-stage ir_len={ir_len} call={call}"),
            );
        }
    }
}

#[test]
fn fft_convolver_reinit_discards_state() {
    let mut conv = FftConvolver::new();
    conv.init(32, &decaying_ir(100, 5)).unwrap();
    let mut scratch = vec![0.0; 77];
    conv.process(&noise(77, 9), &mut scratch);

    let ir = decaying_ir(90, 6);
    conv.init(32, &ir).unwrap();
    let input = noise(400, 10);
    let out = run_blocks(&input, 50, |i, o| conv.process(i, o));
    assert_close(&out, &naive_convolve(&input, &ir), "reinit");
}

// ---------------------------------------------------------------------------
// TwoStageConvolver
// ---------------------------------------------------------------------------

#[test]
fn two_stage_rejects_zero_block_size() {
    let mut conv = TwoStageConvolver::new();
    assert_eq!(
        conv.init(0, 1024, &[1.0]),
        Err(ConvolverError::InvalidBlockSize)
    );
    assert_eq!(
        conv.init(128, 0, &[1.0]),
        Err(ConvolverError::InvalidBlockSize)
    );
}

#[test]
fn two_stage_normalizes_block_sizes() {
    let mut conv = TwoStageConvolver::new();
    conv.init(1000, 100, &decaying_ir(5000, 1)).unwrap();
    assert_eq!(conv.head_block_size(), 128);
    assert_eq!(conv.tail_block_size(), 1024);
    assert!(conv.is_initialized());
    assert_eq!(conv.latency(), 0);
}

#[test]
fn two_stage_empty_ir_outputs_silence() {
    let mut conv = TwoStageConvolver::new();
    conv.init(128, 1024, &[0.0; 4096]).unwrap();
    assert!(!conv.is_initialized());
    let mut out = vec![1.0; 300];
    conv.process(&noise(300, 1), &mut out);
    assert!(out.iter().all(|&s| s == 0.0));
}

/// Zero latency: an impulse in yields the IR out starting at sample 0, and a
/// pure delay IR (delay beyond the head and first tail part) is exact.
#[test]
fn two_stage_impulse_response_and_latency() {
    let ir = decaying_ir(5000, 3);
    let mut conv = TwoStageConvolver::new();
    conv.init(DEFAULT_HEAD_BLOCK_SIZE, DEFAULT_TAIL_BLOCK_SIZE, &ir)
        .unwrap();
    let mut impulse = vec![0.0; 6000];
    impulse[0] = 1.0;
    let out = run_blocks(&impulse, 64, |i, o| conv.process(i, o));
    let mut expected = ir.clone();
    expected.resize(6000, 0.0);
    assert_close(&out, &expected, "impulse");

    for delay in [0usize, 1, 127, 128, 1023, 1024, 2047, 2048, 3000] {
        let mut ir = vec![0.0; delay + 1];
        ir[delay] = 1.0;
        let mut conv = TwoStageConvolver::new();
        conv.init(128, 1024, &ir).unwrap();
        assert!(conv.is_initialized());
        let input = noise(delay + 2000, 4);
        let out = run_blocks(&input, 96, |i, o| conv.process(i, o));
        let measured = out
            .iter()
            .position(|s| s.abs() > 1e-3)
            .expect("output must not be silent");
        assert_eq!(measured, delay, "latency for delay IR {delay}");
        assert_close(&out[delay..], &input[..input.len() - delay], "delay");
    }
}

/// FFT two-stage output vs. naive time-domain convolution across IR lengths
/// (head only, head+tail0, head+tail0+tail1) and host block sizes that are
/// smaller than, equal to, larger than, and not multiples of the partitions.
#[test]
fn two_stage_matches_naive_convolution() {
    let configs: &[(usize, usize)] = &[(128, 1024), (16, 64), (32, 32)];
    let ir_lens: &[usize] = &[1, 50, 128, 129, 1000, 1024, 1025, 2048, 2049, 3000, 6000];
    let calls: &[usize] = &[1, 13, 64, 128, 200, 1024, 1500];
    for &(head, tail) in configs {
        for &ir_len in ir_lens {
            let ir = decaying_ir(ir_len, (ir_len * 31 + head) as u32);
            let input = noise(ir_len + 3 * tail + 500, 1234);
            let expected = naive_convolve(&input, &ir);
            for &call in calls {
                let mut conv = TwoStageConvolver::new();
                conv.init(head, tail, &ir).unwrap();
                let out = run_blocks(&input, call, |i, o| conv.process(i, o));
                assert_close(
                    &out,
                    &expected,
                    &format!("two-stage head={head} tail={tail} ir_len={ir_len} call={call}"),
                );
            }
        }
    }
}

/// Varying host buffer sizes within one stream.
#[test]
fn two_stage_matches_naive_with_variable_call_sizes() {
    let ir = decaying_ir(4500, 77);
    let input = noise(12000, 78);
    let mut conv = TwoStageConvolver::new();
    conv.init(128, 1024, &ir).unwrap();
    let mut out = vec![0.0; input.len()];
    let sizes = [1usize, 511, 64, 3, 1024, 2000, 17, 128, 333];
    let mut pos = 0;
    let mut k = 0;
    while pos < input.len() {
        let n = sizes[k % sizes.len()].min(input.len() - pos);
        conv.process(&input[pos..pos + n], &mut out[pos..pos + n]);
        pos += n;
        k += 1;
    }
    assert_close(&out, &naive_convolve(&input, &ir), "variable calls");
}

/// IR whose head part is silent (only the tail stages are active).
#[test]
fn two_stage_silent_head_part() {
    let mut ir = vec![0.0; 2500];
    ir[1500..].copy_from_slice(&decaying_ir(1000, 8));
    let input = noise(5000, 9);
    let mut conv = TwoStageConvolver::new();
    conv.init(128, 1024, &ir).unwrap();
    let out = run_blocks(&input, 100, |i, o| conv.process(i, o));
    assert_close(&out, &naive_convolve(&input, &ir), "silent head");
}

#[test]
fn two_stage_reset_and_reinit() {
    let mut conv = TwoStageConvolver::new();
    conv.init(128, 1024, &decaying_ir(4000, 1)).unwrap();
    let mut scratch = vec![0.0; 1500];
    conv.process(&noise(1500, 2), &mut scratch);

    conv.reset();
    assert!(!conv.is_initialized());
    conv.process(&noise(1500, 2), &mut scratch);
    assert!(scratch.iter().all(|&s| s == 0.0));

    let ir = decaying_ir(3000, 3);
    conv.init(128, 1024, &ir).unwrap();
    let input = noise(5000, 4);
    let out = run_blocks(&input, 256, |i, o| conv.process(i, o));
    assert_close(&out, &naive_convolve(&input, &ir), "reinit");
}

// ---------------------------------------------------------------------------
// Real-time safety: no heap allocation in `process`
// ---------------------------------------------------------------------------

mod alloc_counter {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    /// Counts allocations made by the current thread while armed.
    pub struct CountingAllocator;

    thread_local! {
        static ARMED: Cell<bool> = const { Cell::new(false) };
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    fn record() {
        let _ = ARMED.try_with(|armed| {
            if armed.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
    }

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            System.alloc(layout)
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            record();
            System.alloc_zeroed(layout)
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            record();
            System.realloc(ptr, layout, new_size)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout)
        }
    }

    /// Run `f` and return how many allocations it made on this thread.
    pub fn count_allocations(f: impl FnOnce()) -> usize {
        COUNT.with(|c| c.set(0));
        ARMED.with(|a| a.set(true));
        f();
        ARMED.with(|a| a.set(false));
        COUNT.with(|c| c.get())
    }
}

#[global_allocator]
static ALLOCATOR: alloc_counter::CountingAllocator = alloc_counter::CountingAllocator;

#[test]
fn process_does_not_allocate() {
    let input = noise(8192, 5);
    let mut output = vec![0.0; input.len()];

    let mut two_stage = TwoStageConvolver::new();
    two_stage.init(128, 1024, &decaying_ir(6000, 6)).unwrap();
    let mut single = FftConvolver::new();
    single.init(128, &decaying_ir(3000, 7)).unwrap();
    let mut empty = TwoStageConvolver::new();

    // Sanity check: the counter does see allocations on this thread.
    let probe = alloc_counter::count_allocations(|| {
        std::hint::black_box(vec![1u8; 16]);
    });
    assert!(probe > 0, "allocation counter is not active");

    let allocations = alloc_counter::count_allocations(|| {
        for call in [1usize, 64, 100, 128, 1024, 3000] {
            for (i, o) in input.chunks(call).zip(output.chunks_mut(call)) {
                two_stage.process(i, o);
                single.process(i, o);
                empty.process(i, o);
            }
        }
    });
    assert_eq!(allocations, 0, "process() allocated on the audio path");
}

// ---------------------------------------------------------------------------
// Throughput (run with `cargo test --release -- --ignored --nocapture`)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "timing benchmark; no C++ baseline comparison"]
fn bench_two_stage_throughput() {
    let sample_rate = 48_000.0;
    let ir = decaying_ir(48_000 / 2, 1);
    let input = noise(48_000 * 10, 2);
    let mut output = vec![0.0; input.len()];
    for call in [64usize, 128, 256, 512] {
        let mut conv = TwoStageConvolver::new();
        conv.init(DEFAULT_HEAD_BLOCK_SIZE, DEFAULT_TAIL_BLOCK_SIZE, &ir)
            .unwrap();
        let start = std::time::Instant::now();
        for (i, o) in input.chunks(call).zip(output.chunks_mut(call)) {
            conv.process(i, o);
        }
        let secs = start.elapsed().as_secs_f64();
        println!(
            "two-stage, 500 ms IR, block {call}: {:.1}x realtime",
            input.len() as f64 / sample_rate / secs
        );
    }
}
