//! `Handoff::take` (the audio-thread side) must never allocate or free, in any
//! interleaving with publishers and the garbage collector (TICKET-024).
//!
//! A counting global allocator records every (de)allocation made on a thread while that
//! thread has counting enabled. Only the consumer enables it, so concurrent publishers and
//! the collector (which do allocate and free) are not counted. This lives in its own test
//! binary because it installs a `#[global_allocator]`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aidadx_core::handoff::Handoff;

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

/// Runs `f` with counting enabled on this thread; returns (allocations, frees, result).
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

/// Heap-heavy payload, like a model: the `Box` plus an inner allocation.
struct Payload {
    id: usize,
    _data: Vec<f32>,
}

fn payload(id: usize) -> Box<Payload> {
    Box::new(Payload {
        id,
        _data: vec![0.0; 256],
    })
}

#[test]
fn counter_works() {
    let (a, f, v) = count(|| vec![0u8; 16]);
    assert_eq!((a, f), (1, 0));
    let (a, f, ()) = count(|| drop(v));
    assert_eq!((a, f), (0, 1));
}

#[test]
fn take_never_allocates_or_frees_single_thread() {
    let h: Handoff<Payload> = Handoff::new();
    let mut cur: Option<Box<Payload>> = None;

    // Empty slot.
    let (a, f, changed) = count(|| h.take(&mut cur));
    assert_eq!((a, f, changed), (0, 0, false));

    // First install (nothing to retire).
    h.publish(payload(1));
    let (a, f, changed) = count(|| h.take(&mut cur));
    assert_eq!((a, f, changed), (0, 0, true));

    // Swap: old value retired, not freed.
    h.publish(payload(2));
    let (a, f, changed) = count(|| h.take(&mut cur));
    assert_eq!((a, f, changed), (0, 0, true));
    assert!(h.has_retired());

    // Pending while retired is full (publish raced with the previous swap): deferred.
    h.publish_without_gc(payload(3));
    let (a, f, changed) = count(|| (0..10).any(|_| h.take(&mut cur)));
    assert_eq!(
        (a, f, changed),
        (0, 0, false),
        "swap must be deferred, not freed"
    );
    assert_eq!(cur.as_ref().unwrap().id, 2);

    // Producer collects; the deferred swap goes through, still without freeing.
    h.collect_garbage();
    let (a, f, changed) = count(|| h.take(&mut cur));
    assert_eq!((a, f, changed), (0, 0, true));
    assert_eq!(cur.as_ref().unwrap().id, 3);
}

#[cfg(feature = "rtneural")]
#[test]
fn atomic_model_take_pending_never_frees() {
    // The exact TICKET-024 scenario on the real model type: two publishes before GC.
    use aidadx_core::model::{AtomicModel, DynamicModel};
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../files/tw40_california_clean_deerinkstudios.json"
    ))
    .unwrap();
    let model = || DynamicModel::from_json(&json).unwrap();

    let slot = AtomicModel::new();
    let mut active: Option<Box<DynamicModel>> = None;
    slot.publish(model());
    assert!(count(|| slot.take_pending(&mut active)).2);
    slot.publish(model());
    let (a, f, changed) = count(|| slot.take_pending(&mut active));
    assert_eq!((a, f, changed), (0, 0, true));
    slot.publish_without_gc(model());
    let (a, f, changed) = count(|| slot.take_pending(&mut active));
    assert_eq!((a, f, changed), (0, 0, false), "old AtomicModel freed here");
    slot.collect_garbage();
    let (a, f, changed) = count(|| slot.take_pending(&mut active));
    assert_eq!((a, f, changed), (0, 0, true));
}

#[test]
fn take_never_allocates_or_frees_under_concurrency() {
    const PRODUCERS: usize = 3;
    const PER_PRODUCER: usize = 2000;

    let h: Arc<Handoff<Payload>> = Arc::new(Handoff::new());
    let done = Arc::new(AtomicUsize::new(0));
    let stop_gc = Arc::new(AtomicBool::new(false));

    let producers: Vec<_> = (0..PRODUCERS)
        .map(|p| {
            let (h, done) = (h.clone(), done.clone());
            std::thread::spawn(move || {
                for i in 0..PER_PRODUCER {
                    let v = payload(p * PER_PRODUCER + i);
                    if i % 2 == 0 {
                        h.publish_without_gc(v);
                    } else {
                        h.publish(v);
                    }
                }
                done.fetch_add(1, Ordering::Release);
            })
        })
        .collect();
    let collector = {
        let (h, stop) = (h.clone(), stop_gc.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                h.collect_garbage();
                std::thread::yield_now();
            }
        })
    };

    // Consumer = this thread, counting the whole take loop.
    let mut cur: Option<Box<Payload>> = None;
    let deadline = Instant::now() + Duration::from_secs(60);
    let (a, f, swaps) = count(|| {
        let mut swaps = 0usize;
        loop {
            if h.take(&mut cur) {
                swaps += 1;
            }
            if done.load(Ordering::Acquire) == PRODUCERS && !h.has_pending() {
                break swaps;
            }
            // `Instant::now` does not allocate.
            if Instant::now() > deadline {
                break usize::MAX;
            }
        }
    });
    assert_ne!(swaps, usize::MAX, "consumer timed out");
    assert_eq!(
        (a, f),
        (0, 0),
        "take allocated {a} / freed {f} over {swaps} swaps"
    );
    assert!(swaps >= 1);

    for p in producers {
        p.join().unwrap();
    }
    stop_gc.store(true, Ordering::Release);
    collector.join().unwrap();
}
