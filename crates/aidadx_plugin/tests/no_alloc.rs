//! `AidaEngine::process` and `reset` must never allocate or free, including while model
//! and cabinet swaps are pending, being taken, or deferred (TICKET-017).
//!
//! A counting global allocator records every (de)allocation made on the test thread while
//! counting is enabled; loading/publishing happens with counting disabled, as it would on
//! the loader thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use aidadx_core::model::DynamicModel;
use aidadx_plugin::loader::{load_cabinet, load_model, FileSource};
use aidadx_plugin::test_models::lstm_json;
use aidadx_plugin::{AidaEngine, ParamSnapshot, PluginOutputs};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

fn counting() -> bool {
    COUNTING.try_with(|c| c.get()).unwrap_or(false)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if counting() {
            FREES.fetch_add(1, Ordering::SeqCst);
        }
        System.dealloc(ptr, layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if counting() {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Runs `f` with counting enabled and returns (allocations, frees) it made.
fn count<R>(f: impl FnOnce() -> R) -> (usize, usize, R) {
    let (a0, f0) = (ALLOCS.load(Ordering::SeqCst), FREES.load(Ordering::SeqCst));
    COUNTING.with(|c| c.set(true));
    let r = f();
    COUNTING.with(|c| c.set(false));
    (
        ALLOCS.load(Ordering::SeqCst) - a0,
        FREES.load(Ordering::SeqCst) - f0,
        r,
    )
}

fn signal(len: usize, seed: u32) -> Vec<f32> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 8) as f32 / (1u32 << 24) as f32 - 0.5
        })
        .collect()
}

fn param_model(input: usize) -> DynamicModel {
    let mut m = DynamicModel::from_json(&lstm_json(input, 8, 11)).unwrap();
    m.prewarm(0.0, 0.0).unwrap();
    m
}

#[test]
fn process_and_swaps_do_not_allocate() {
    const SR: f64 = 48_000.0;
    const BLOCK: usize = 128;

    // The counter itself works.
    let (a, f, v) = count(|| vec![0u8; 16]);
    drop(v);
    assert_eq!((a, f), (1, 0));

    // Every stage on: LPF, EQ pre, model, DC blocker, cabinet.
    let params = ParamSnapshot {
        antialiasing: 50.0,
        eq_bypass: false,
        bass_db: 3.0,
        ..ParamSnapshot::default()
    };
    let mut engine = AidaEngine::new();
    engine.prepare(SR, BLOCK, &params);
    let outputs = PluginOutputs::default();
    let model_slot = engine.model_slot();
    let cabinet_slot = engine.cabinet_slot();
    let mut buf = signal(BLOCK, 1);

    let mut run_blocks = |engine: &mut AidaEngine, n: usize| {
        let (a, f, _) = count(|| {
            for _ in 0..n {
                engine.process(&mut buf, &outputs);
            }
        });
        assert_eq!((a, f), (0, 0), "process allocated {a} / freed {f}");
    };

    // 1. First model + cabinet pending -> taken by process (nothing to retire).
    model_slot.publish(Box::new(load_model(&FileSource::Default).unwrap()));
    cabinet_slot.publish(Box::new(load_cabinet(&FileSource::Default, SR).unwrap()));
    run_blocks(&mut engine, 50);
    assert!(engine.model().is_some() && engine.cabinet().is_some());
    assert_eq!(outputs.model_input_size.load(Ordering::Relaxed), 1);

    // 2. Swap to a 3-input model and a new cabinet: the old ones are retired, not freed.
    model_slot.publish(Box::new(param_model(3)));
    cabinet_slot.publish(Box::new(load_cabinet(&FileSource::Default, SR).unwrap()));
    run_blocks(&mut engine, 50);
    assert_eq!(outputs.model_input_size.load(Ordering::Relaxed), 3);
    assert!(model_slot.has_retired() && cabinet_slot.has_retired());

    // 3. Another swap while the retired slots are still full (loader has not collected):
    //    the swap is deferred, nothing is freed on this thread.
    publish_without_gc(&model_slot, Box::new(param_model(2)));
    publish_without_gc(
        &cabinet_slot,
        Box::new(load_cabinet(&FileSource::Default, SR).unwrap()),
    );
    run_blocks(&mut engine, 10);
    assert_eq!(
        outputs.model_input_size.load(Ordering::Relaxed),
        3,
        "swap deferred"
    );
    assert!(model_slot.has_pending());

    // Loader collects -> the deferred swap goes through on the next block.
    model_slot.collect_garbage();
    cabinet_slot.collect_garbage();
    run_blocks(&mut engine, 10);
    assert_eq!(outputs.model_input_size.load(Ordering::Relaxed), 2);
    assert!(!model_slot.has_pending() && !cabinet_slot.has_pending());

    // 4. Parameter ramps (per-sample conditioning path), CABSIMBYPASS, NETBYPASS.
    engine.set_params(&ParamSnapshot {
        param1: 1.0,
        cabsim_bypass: true,
        ..params
    });
    run_blocks(&mut engine, 100);
    engine.set_params(&ParamSnapshot {
        net_bypass: true,
        ..params
    });
    run_blocks(&mut engine, 10);

    // 5. reset() (activate) pre-rolls the model without allocating.
    let (a, f, _) = count(|| engine.reset());
    assert_eq!((a, f), (0, 0), "reset allocated {a} / freed {f}");

    // 6. Oversized host block (split internally).
    let mut big = signal(BLOCK * 3 + 17, 5);
    let (a, f, _) = count(|| engine.process(&mut big, &outputs));
    assert_eq!((a, f), (0, 0));
    assert!(big.iter().all(|s| s.is_finite()));
}

/// Fills the pending slot while the retired slot is still occupied, which is what the
/// audio thread sees when a `publish` races with its previous swap.
fn publish_without_gc<T>(slot: &aidadx_plugin::handoff::Handoff<T>, value: Box<T>) {
    slot.publish_without_gc(value);
}
