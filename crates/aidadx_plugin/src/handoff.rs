//! Lock-free, allocation-free hand-off of heavy objects (neural models, convolvers) from a
//! loader thread to the audio thread.
//!
//! This generalises the pattern of `aidadx_core::model::AtomicModel` to any `T` and closes
//! its one gap: `AtomicModel::take_pending` frees the previously retired model on the audio
//! thread if two swaps happen before the loader collected it. [`Handoff::take`] instead
//! *defers* a swap while the retired slot is still occupied, so the audio thread never
//! allocates or deallocates, under any interleaving.
//!
//! Ownership:
//!
//! - `pending`: written by the producer ([`Handoff::publish`]), emptied by the consumer
//!   ([`Handoff::take`]). A value that was never taken is freed by the producer when it is
//!   replaced (latest wins), or by `Drop`.
//! - `retired`: filled only by the consumer (with the value it replaced), and only while
//!   empty; emptied and freed by the producer ([`Handoff::collect_garbage`]) or by `Drop`.
//! - The *active* value lives in the consumer's own `Option<Box<T>>`.

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
    _owns: PhantomData<*mut T>,
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
    /// taken is dropped here (latest wins). Also collects garbage. Not real-time safe.
    pub fn publish(&self, value: Box<T>) {
        self.collect_garbage();
        free(self.pending.swap(Box::into_raw(value), Ordering::AcqRel));
    }

    /// Like [`publish`](Self::publish) but leaves the retired slot alone, reproducing a
    /// publish that races with the consumer's previous swap. For tests only.
    #[doc(hidden)]
    pub fn publish_without_gc(&self, value: Box<T>) {
        free(self.pending.swap(Box::into_raw(value), Ordering::AcqRel));
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
    pub fn take(&self, current: &mut Option<Box<T>>) -> bool {
        // Only this thread fills `retired`, so if it is empty now it stays empty until we
        // fill it below.
        if current.is_some() && self.has_retired() {
            return false;
        }
        let p = self.pending.swap(ptr::null_mut(), Ordering::AcqRel);
        if p.is_null() {
            return false;
        }
        // SAFETY: `p` came from `Box::into_raw` in `publish`; swapping it out of the slot
        // transferred ownership to us.
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
    use std::sync::atomic::AtomicUsize;
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

        h.publish(Box::new(Tracked(1, drops.clone())));
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 1);
        assert!(!h.has_retired(), "nothing replaced yet");

        h.publish(Box::new(Tracked(2, drops.clone())));
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 2);
        assert!(h.has_retired());
        assert_eq!(drops.load(Ordering::SeqCst), 0, "consumer must not free");

        // Third value arrives while #1 is still retired: publish collects #1 first.
        h.publish(Box::new(Tracked(3, drops.clone())));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(h.take(&mut cur));
        assert_eq!(cur.as_ref().unwrap().0, 3);

        // Simulate the race: a pending value while the retired slot is full -> deferred.
        h.publish_without_gc(Box::new(Tracked(4, drops.clone())));
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
    fn latest_publish_wins() {
        let drops = Arc::new(AtomicUsize::new(0));
        let h = Handoff::new();
        h.publish(Box::new(Tracked(1, drops.clone())));
        h.publish(Box::new(Tracked(2, drops.clone())));
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
                    h.publish(Box::new(Tracked(i, drops.clone())));
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
        let alive = usize::from(cur.is_some()) + usize::from(h.has_retired());
        drop(cur);
        drop(Arc::try_unwrap(h).ok().unwrap());
        assert_eq!(
            drops.load(Ordering::SeqCst),
            2000,
            "{alive} alive at end, all freed"
        );
    }
}
