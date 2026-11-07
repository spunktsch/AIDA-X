//! Lock-free, allocation-free hand-off of heavy objects (neural models, convolvers) from
//! non-real-time threads to the audio thread.
//!
//! [`Handoff<T>`] is a two-slot mailbox (`pending` + `retired`). The audio thread
//! ([`Handoff::take`]) never allocates or frees under any interleaving: if the value it
//! would replace cannot be parked because the retired slot is still occupied (the producer
//! has not collected the previous swap yet), the swap is *deferred* to a later call instead
//! of freeing anything (TICKET-024).
//!
//! # Ownership
//!
//! - `pending`: filled by producers ([`Handoff::publish`]), emptied by the consumer
//!   ([`Handoff::take`]). A value that was never taken is freed by the producer that
//!   replaces it (latest wins), or by `Drop`.
//! - `retired`: filled only by the consumer (with the value it replaced), and only while
//!   empty; emptied and freed by producers ([`Handoff::collect_garbage`], also called by
//!   `publish`) or by `Drop`.
//! - The *active* value lives in the consumer's own `Option<Box<T>>`.
//!
//! # Liveness
//!
//! A deferred swap only goes through after someone calls
//! [`collect_garbage`](Handoff::collect_garbage) (or `publish`). The producer side must
//! therefore collect periodically (the plugin's loader thread does so every
//! `GC_INTERVAL`), otherwise the last published value can stay pending forever.
//!
//! # Memory ordering
//!
//! Every non-null pointer in a slot is a `Box` whose ownership moves with the pointer. A
//! thread that obtains a pointer from a slot must see all writes made to the pointee by the
//! thread that put it there, so every *store of a non-null pointer* is `Release` and every
//! *read-modify-write that takes a pointer out* is `Acquire`:
//!
//! | operation | ordering | why |
//! |---|---|---|
//! | `publish`: `pending.swap(new)` | `AcqRel` | `Release` publishes the freshly built `T` to the consumer; `Acquire` because the replaced (never taken) value may come from another producer and is dropped here. |
//! | `take`: `pending.swap(null)` | `Acquire` | Synchronizes with the producer's `Release`, so the new `T` is fully visible. Storing null publishes no data, so no `Release` is needed. |
//! | `take`: `retired.load()` (guard) | `Relaxed` | Only the consumer stores non-null into `retired`; producers only swap it to null. By read-after-write coherence the consumer cannot read an older value than its own last store, so reading null means a producer has emptied the slot and it stays empty until the consumer fills it. A stale non-null only defers the swap. No pointee is accessed based on this load. |
//! | `take`: `retired.store(old)` | `Release` | Publishes the consumer's writes to the old `T` (it was processing audio with it) to the producer that frees it. A plain store (not a swap) is enough because the guard proved the slot empty. |
//! | `collect_garbage`: `retired.swap(null)` | `AcqRel` | `Acquire` pairs with the consumer's `Release` store before the value is dropped. `Release` is not required (null carries no data) but costs nothing off the audio thread. |
//! | `has_pending` / `has_retired` | `Acquire` | Informational only; no pointer obtained from them is dereferenced. |
//! | `Drop` | `get_mut` | `&mut self` proves exclusivity; whatever handed out the `&mut` (thread join, `Arc` drop) already established happens-before with all other users. |
//!
//! Because each pointer is removed from a slot by exactly one atomic RMW (`swap`), each
//! value is owned, and finally dropped, by exactly one thread. With several producers the
//! "latest" value is the last one in `pending`'s modification order; values from any single
//! producer are taken in the order that producer published them.

use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Two-slot (pending + retired) lock-free mailbox. One consumer thread (the audio thread)
/// calls [`take`](Self::take); any number of non-real-time threads may call
/// [`publish`](Self::publish) and [`collect_garbage`](Self::collect_garbage).
pub struct Handoff<T> {
    pending: AtomicPtr<T>,
    retired: AtomicPtr<T>,
    /// Owns `T`s; `Send`/`Sync` are implemented manually below.
    _owns: PhantomData<Box<T>>,
}

// SAFETY: the slots only ever move ownership of a `T` from one thread to another (like a
// channel); no `&T` is shared between threads. So `T: Send` is sufficient for both.
unsafe impl<T: Send> Send for Handoff<T> {}
unsafe impl<T: Send> Sync for Handoff<T> {}

fn free<T>(p: *mut T) {
    if !p.is_null() {
        // SAFETY: every non-null pointer stored in a slot came from `Box::into_raw` and is
        // owned exclusively by whoever swapped it out of the slot.
        drop(unsafe { Box::from_raw(p) });
    }
}

impl<T> Default for Handoff<T> {
    fn default() -> Self {
        Self {
            pending: AtomicPtr::new(ptr::null_mut()),
            retired: AtomicPtr::new(ptr::null_mut()),
            _owns: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Handoff<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handoff")
            .field("pending", &self.has_pending())
            .field("retired", &self.has_retired())
            .finish()
    }
}

impl<T> Handoff<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Producer: queue `value` for the consumer. A previously queued value that was never
    /// taken is dropped here (latest wins). Also collects garbage first, so a swap the
    /// consumer deferred can go through. Not real-time safe.
    ///
    /// Accepts `T` or `Box<T>`.
    pub fn publish(&self, value: impl Into<Box<T>>) {
        self.collect_garbage();
        self.publish_without_gc(value);
    }

    /// Like [`publish`](Self::publish) but leaves the retired slot alone, reproducing a
    /// publish that races with the consumer's previous swap. For tests only.
    #[doc(hidden)]
    pub fn publish_without_gc(&self, value: impl Into<Box<T>>) {
        let new = Box::into_raw(value.into());
        free(self.pending.swap(new, Ordering::AcqRel));
    }

    /// Producer: free the value the consumer retired, if any. Not real-time safe.
    pub fn collect_garbage(&self) {
        free(self.retired.swap(ptr::null_mut(), Ordering::AcqRel));
    }

    /// True if a value is waiting to be taken.
    pub fn has_pending(&self) -> bool {
        !self.pending.load(Ordering::Acquire).is_null()
    }

    /// True if a retired value is waiting to be freed.
    pub fn has_retired(&self) -> bool {
        !self.retired.load(Ordering::Acquire).is_null()
    }

    /// Consumer (single thread): install the pending value into `current`, parking the
    /// replaced value in the retired slot. Returns `true` if `current` changed.
    ///
    /// Real-time safe: never allocates or frees. If the retired slot is still occupied
    /// (the producer has not collected the previous swap yet), the swap is deferred to a
    /// later call and `false` is returned.
    ///
    /// Must only ever be called from one thread at a time (the single consumer).
    pub fn take(&self, current: &mut Option<Box<T>>) -> bool {
        // Only the consumer fills `retired`, so if it is empty now it stays empty until we
        // fill it below. See the module docs for why `Relaxed` is enough here.
        if current.is_some() && !self.retired.load(Ordering::Relaxed).is_null() {
            return false;
        }
        let p = self.pending.swap(ptr::null_mut(), Ordering::Acquire);
        if p.is_null() {
            return false;
        }
        // SAFETY: `p` came from `Box::into_raw` in `publish`; swapping it out of the slot
        // transferred ownership to us, and `Acquire` made its contents visible.
        let new = unsafe { Box::from_raw(p) };
        if let Some(old) = current.replace(new) {
            self.retired.store(Box::into_raw(old), Ordering::Release);
        }
        true
    }
}

impl<T> Drop for Handoff<T> {
    fn drop(&mut self) {
        free(*self.pending.get_mut());
        free(*self.retired.get_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize};
    use std::sync::Arc;

    struct Tracked(u32, Arc<AtomicUsize>);
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn swap_retires_and_defers_until_collected() {
        let drops = Arc::new(AtomicUsize::new(0));
        let h = Handoff::new();
        let mut cur: Option<Box<Tracked>> = None;
        assert!(!h.take(&mut cur));

        h.publish(Tracked(1, drops.clone()));
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 1);
        assert!(!h.has_retired(), "nothing replaced yet");

        h.publish(Box::new(Tracked(2, drops.clone())));
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 2);
        assert!(h.has_retired());
        assert_eq!(drops.load(Ordering::SeqCst), 0, "consumer must not free");

        // Third value arrives while #1 is still retired: publish collects #1 first.
        h.publish(Tracked(3, drops.clone()));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 3);

        // Simulate the race: a pending value while the retired slot is full -> deferred.
        h.publish_without_gc(Tracked(4, drops.clone()));
        assert!(h.has_retired());
        assert!(!h.take(&mut cur), "swap must be deferred");
        assert_eq!(cur.as_ref().unwrap().0, 3);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        h.collect_garbage();
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 4);

        drop(h);
        assert_eq!(drops.load(Ordering::SeqCst), 3, "retired #3 freed by Drop");
        drop(cur);
        assert_eq!(drops.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn first_take_ignores_full_retired_slot() {
        // With no active value there is nothing to retire, so a full retired slot must not
        // block the first install.
        let drops = Arc::new(AtomicUsize::new(0));
        let h: Handoff<Tracked> = Handoff::new();
        let mut a = Some(Box::new(Tracked(0, drops.clone())));
        h.publish(Tracked(1, drops.clone()));
        assert!(h.take(&mut a));
        assert!(h.has_retired());
        h.publish_without_gc(Tracked(2, drops.clone()));
        let mut b = None;
        assert!(h.take(&mut b));
        assert_eq!(b.as_ref().unwrap().0, 2);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn latest_publish_wins() {
        let drops = Arc::new(AtomicUsize::new(0));
        let h: Handoff<Tracked> = Handoff::new();
        h.publish(Tracked(1, drops.clone()));
        h.publish(Tracked(2, drops.clone()));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        let mut cur = None;
        assert!(h.take(&mut cur));
        assert_eq!(cur.unwrap().0, 2);
    }

    #[test]
    fn concurrent_publish_and_take() {
        let drops = Arc::new(AtomicUsize::new(0));
        let h = Arc::new(Handoff::new());
        let producer = {
            let (h, drops) = (h.clone(), drops.clone());
            std::thread::spawn(move || {
                for i in 0..2000 {
                    h.publish(Tracked(i, drops.clone()));
                    if i % 7 == 0 {
                        h.collect_garbage();
                    }
                }
            })
        };
        let mut cur: Option<Box<Tracked>> = None;
        let mut last = 0;
        while !producer.is_finished() || h.has_pending() {
            h.take(&mut cur);
            if let Some(c) = &cur {
                assert!(c.0 >= last, "values arrive in order");
                last = c.0;
            }
            h.collect_garbage(); // stands in for the loader's periodic collection
        }
        producer.join().unwrap();
        drop(cur);
        drop(Arc::try_unwrap(h).ok().unwrap());
        assert_eq!(drops.load(Ordering::SeqCst), 2000, "all freed");
    }

    // -----------------------------------------------------------------------------------
    // Interleaving stress test: several publishers, one dedicated collector, one consumer.
    // -----------------------------------------------------------------------------------

    thread_local! {
        static IS_CONSUMER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// Value that records its own drop in a per-id counter and flags drops that happen on
    /// the consumer thread.
    struct Probe {
        producer: usize,
        seq: usize,
        id: usize,
        drops: Arc<Vec<AtomicU8>>,
        consumer_drops: Arc<AtomicUsize>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            self.drops[self.id].fetch_add(1, Ordering::SeqCst);
            if IS_CONSUMER.with(|c| c.get()) {
                self.consumer_drops.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[test]
    fn stress_many_publishers_one_consumer() {
        const PRODUCERS: usize = 4;
        const PER_PRODUCER: usize = 3000;
        const TOTAL: usize = PRODUCERS * PER_PRODUCER;

        let drops: Arc<Vec<AtomicU8>> = Arc::new((0..TOTAL).map(|_| AtomicU8::new(0)).collect());
        let consumer_drops = Arc::new(AtomicUsize::new(0));
        let h: Arc<Handoff<Probe>> = Arc::new(Handoff::new());
        let stop_gc = Arc::new(AtomicBool::new(false));
        let producers_done = Arc::new(AtomicUsize::new(0));

        let producers: Vec<_> = (0..PRODUCERS)
            .map(|p| {
                let (h, drops, cd) = (h.clone(), drops.clone(), consumer_drops.clone());
                let done = producers_done.clone();
                std::thread::spawn(move || {
                    for seq in 0..PER_PRODUCER {
                        let v = Probe {
                            producer: p,
                            seq,
                            id: p * PER_PRODUCER + seq,
                            drops: drops.clone(),
                            consumer_drops: cd.clone(),
                        };
                        // Mix GC'ing and racing publishes so the consumer hits the
                        // "retired full" deferral path often.
                        if seq % 3 == 0 {
                            h.publish_without_gc(v);
                        } else {
                            h.publish(v);
                        }
                        if seq % 64 == 0 {
                            std::thread::yield_now();
                        }
                    }
                    // Release: our last publish happens-before the consumer seeing `done`.
                    done.fetch_add(1, Ordering::Release);
                })
            })
            .collect();

        // Dedicated collector (the loader's periodic GC), needed for liveness once the
        // publishers are done.
        let collector = {
            let (h, stop) = (h.clone(), stop_gc.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    h.collect_garbage();
                    std::thread::yield_now();
                }
            })
        };

        let consumer = {
            let (h, done) = (h.clone(), producers_done.clone());
            std::thread::spawn(move || {
                IS_CONSUMER.with(|c| c.set(true));
                let mut cur: Option<Box<Probe>> = None;
                let mut last_seq = [None::<usize>; PRODUCERS];
                let (mut swaps, mut deferred) = (0usize, 0usize);
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
                loop {
                    let had_pending = h.has_pending();
                    if h.take(&mut cur) {
                        swaps += 1;
                        let c = cur.as_ref().unwrap();
                        if let Some(prev) = last_seq[c.producer] {
                            assert!(c.seq > prev, "per-producer order violated");
                        }
                        last_seq[c.producer] = Some(c.seq);
                    } else if had_pending {
                        deferred += 1;
                    }
                    // Read `done` first: if all publishers finished, every publish is visible
                    // to the following `has_pending`.
                    if done.load(Ordering::Acquire) == PRODUCERS && !h.has_pending() {
                        break;
                    }
                    assert!(
                        std::time::Instant::now() < deadline,
                        "stress test timed out"
                    );
                }
                IS_CONSUMER.with(|c| c.set(false));
                (cur, swaps, deferred)
            })
        };

        for p in producers {
            p.join().unwrap();
        }
        let (cur, swaps, deferred) = consumer.join().unwrap();
        stop_gc.store(true, Ordering::Release);
        collector.join().unwrap();

        assert!(swaps >= 2, "consumer swapped {swaps} times");
        assert_eq!(
            consumer_drops.load(Ordering::SeqCst),
            0,
            "consumer thread freed a value"
        );
        let last = cur.as_ref().unwrap();
        assert!(
            last.seq == PER_PRODUCER - 1,
            "the final value of some producer is active"
        );
        eprintln!("stress: {swaps} swaps, {deferred} deferred takes");

        drop(cur);
        drop(Arc::try_unwrap(h).unwrap_or_else(|_| panic!("all other owners joined")));
        for (id, d) in drops.iter().enumerate() {
            assert_eq!(
                d.load(Ordering::SeqCst),
                1,
                "value {id} dropped {} times",
                d.load(Ordering::SeqCst)
            );
        }
    }
}
