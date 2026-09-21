//! Opt-in allocator accounting for isolated profiling builds, never production.
//! Counts Rust allocation requests, not allocator overhead or input mmap pages.
//! Begin/end must surround quiescent dataset calls. Do not use these runs for timing.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

/// Disjoint caller scopes. Deallocation is attributed to the freeing scope.
#[derive(Clone, Copy)]
pub enum Stage {
    /// Parsing, adapter work, and untagged allocations.
    Other = 0,
    /// Extracting player snapshot rows.
    Rows = 1,
    /// Growing or merging snapshot columns.
    Columns = 2,
    /// Constructing the returned DataFrame.
    Frame = 3,
}

thread_local! { static STAGE: Cell<usize> = const { Cell::new(0) }; }

/// Restores the previous stage, including during unwinding.
pub struct Scope(usize);

impl Drop for Scope {
    fn drop(&mut self) {
        let _ = STAGE.try_with(|stage| stage.set(self.0));
    }
}

/// Tag allocations on this thread until the returned guard is dropped.
pub fn enter(stage: Stage) -> Scope {
    Scope(STAGE.with(|current| current.replace(stage as usize)))
}

/// Request counts for one scope.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Totals {
    pub allocations: u64,
    pub reallocations: u64,
    pub deallocations: u64,
    /// Full requested allocation/reallocation sizes, not net growth.
    pub requested_bytes: u64,
}

/// C-compatible snapshot, ordered as Other, Rows, Columns, Frame.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Snapshot {
    pub live_start: u64,
    pub live_end: u64,
    /// Largest observed live requested-byte count during the window.
    pub peak_live: u64,
    /// Must be zero for live/peak accounting to be usable.
    pub underflows: u64,
    pub stages: [Totals; 4],
}

#[derive(Default)]
struct Counters {
    allocations: AtomicU64,
    reallocations: AtomicU64,
    deallocations: AtomicU64,
    bytes: AtomicU64,
}

impl Counters {
    const fn new() -> Self {
        Self {
            allocations: AtomicU64::new(0),
            reallocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }

    fn reset(&self) {
        self.allocations.store(0, Relaxed);
        self.reallocations.store(0, Relaxed);
        self.deallocations.store(0, Relaxed);
        self.bytes.store(0, Relaxed);
    }

    fn snapshot(&self) -> Totals {
        Totals {
            allocations: self.allocations.load(Relaxed),
            reallocations: self.reallocations.load(Relaxed),
            deallocations: self.deallocations.load(Relaxed),
            requested_bytes: self.bytes.load(Relaxed),
        }
    }
}

struct Ledger {
    active: AtomicBool,
    live: AtomicU64,
    start: AtomicU64,
    peak: AtomicU64,
    underflows: AtomicU64,
    stages: [Counters; 4],
}

impl Ledger {
    const fn new() -> Self {
        Self {
            active: AtomicBool::new(false),
            live: AtomicU64::new(0),
            start: AtomicU64::new(0),
            peak: AtomicU64::new(0),
            underflows: AtomicU64::new(0),
            stages: [const { Counters::new() }; 4],
        }
    }

    fn begin(&self) {
        self.active.store(false, Relaxed);
        for stage in &self.stages {
            stage.reset();
        }
        let live = self.live.load(Relaxed);
        self.start.store(live, Relaxed);
        self.peak.store(live, Relaxed);
        self.active.store(true, Relaxed);
    }

    fn end(&self) -> Snapshot {
        self.active.store(false, Relaxed);
        Snapshot {
            live_start: self.start.load(Relaxed),
            live_end: self.live.load(Relaxed),
            peak_live: self.peak.load(Relaxed),
            underflows: self.underflows.load(Relaxed),
            stages: self.stages.each_ref().map(Counters::snapshot),
        }
    }

    fn grow(&self, size: usize) {
        let live = self.live.fetch_add(size as u64, Relaxed) + size as u64;
        if self.active.load(Relaxed) {
            self.peak.fetch_max(live, Relaxed);
        }
    }

    fn shrink(&self, size: usize) {
        if self
            .live
            .fetch_update(Relaxed, Relaxed, |live| live.checked_sub(size as u64))
            .is_err()
        {
            self.underflows.fetch_add(1, Relaxed);
        }
    }

    fn alloc(&self, stage: usize, size: usize) {
        self.grow(size);
        if self.active.load(Relaxed) {
            self.stages[stage].allocations.fetch_add(1, Relaxed);
            self.stages[stage].bytes.fetch_add(size as u64, Relaxed);
        }
    }

    fn free(&self, stage: usize, size: usize) {
        self.shrink(size);
        if self.active.load(Relaxed) {
            self.stages[stage].deallocations.fetch_add(1, Relaxed);
        }
    }

    fn resize(&self, stage: usize, old: usize, new: usize) {
        if new >= old {
            self.grow(new - old);
        } else {
            self.shrink(old - new);
        }
        if self.active.load(Relaxed) {
            self.stages[stage].reallocations.fetch_add(1, Relaxed);
            self.stages[stage].bytes.fetch_add(new as u64, Relaxed);
        }
    }
}

static LEDGER: Ledger = Ledger::new();

/// Start a measurement after all previous dataset work has joined.
pub fn begin() {
    LEDGER.begin();
}

/// Finish a measurement after the current dataset work has joined.
pub fn end() -> Snapshot {
    LEDGER.end()
}

fn stage() -> usize {
    STAGE.try_with(Cell::get).unwrap_or(0)
}

/// System allocator with request accounting. Install only in a diagnostic binary.
pub struct CountedSystem;

// SAFETY: All allocator operations preserve System's pointer/layout contract.
// Counters and TLS tags do not allocate or modify user allocations.
unsafe impl GlobalAlloc for CountedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LEDGER.alloc(stage(), layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies a valid allocation layout.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            LEDGER.alloc(stage(), layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // Account before freeing so address reuse cannot invert the live-byte count.
        LEDGER.free(stage(), layout.size());
        // SAFETY: The caller supplies a live pointer with its original layout.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: The caller supplies a live allocation and a valid new size.
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            LEDGER.resize(stage(), layout.size(), size);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_track_live_bytes_reallocation_and_outside_window_frees() {
        let ledger = Ledger::new();
        ledger.alloc(0, 100);
        ledger.begin();
        ledger.alloc(1, 40);
        ledger.resize(1, 40, 80);
        ledger.resize(1, 80, 20);
        ledger.free(2, 100);
        let result = ledger.end();
        assert_eq!(
            (result.live_start, result.live_end, result.peak_live),
            (100, 20, 180)
        );
        assert_eq!(result.stages[1].allocations, 1);
        assert_eq!(result.stages[1].reallocations, 2);
        assert_eq!(result.stages[1].requested_bytes, 140);
        assert_eq!(result.stages[2].deallocations, 1);
        ledger.free(0, 20);
        ledger.begin();
        let result = ledger.end();
        assert_eq!(
            (
                result.live_start,
                result.live_end,
                result.peak_live,
                result.underflows
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(result.stages[1].allocations, 0);
    }

    #[test]
    fn concurrent_counts_balance_after_workers_join() {
        let ledger = Ledger::new();
        ledger.begin();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let ledger = &ledger;
                scope.spawn(move || {
                    for _ in 0..1000 {
                        ledger.alloc(0, 16);
                        ledger.resize(0, 16, 32);
                        ledger.free(0, 32);
                    }
                });
            }
        });
        let result = ledger.end();
        assert_eq!(result.live_end, 0);
        assert_eq!(result.underflows, 0);
        assert_eq!(result.stages[0].allocations, 4000);
        assert_eq!(result.stages[0].requested_bytes, 4000 * 48);
        assert!((32..=128).contains(&result.peak_live));
    }

    #[test]
    fn invalid_accounting_is_visible_and_scopes_restore_on_unwind() {
        let ledger = Ledger::new();
        ledger.free(0, 1);
        assert_eq!(ledger.end().underflows, 1);
        let _outer = enter(Stage::Rows);
        let _ = std::panic::catch_unwind(|| {
            let _inner = enter(Stage::Frame);
            assert_eq!(stage(), 3);
            panic!("test");
        });
        assert_eq!(stage(), 1);
    }
}
