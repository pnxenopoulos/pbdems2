//! Count allocations separately from timing. This diagnostic is single-threaded.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use pbdems2_bench::lookup_workloads::lookup_cases;

struct CountedSystem;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Every operation forwards its pointer and layout unchanged to System.
// Counters do not allocate and never modify the returned allocation.
unsafe impl GlobalAlloc for CountedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: The caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The caller supplies a live pointer and its original layout.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size, Ordering::Relaxed);
        // SAFETY: The caller supplies a live allocation and a valid new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountedSystem = CountedSystem;

fn main() {
    const CALLS: usize = 1_000;
    let cases = lookup_cases();
    println!("case,calls,allocations,reallocations,requested_bytes");
    for case in cases {
        assert_eq!(case.serializer.resolve_field_key(&case.path), case.expected);
        ALLOCATIONS.store(0, Ordering::Relaxed);
        REALLOCATIONS.store(0, Ordering::Relaxed);
        BYTES.store(0, Ordering::Relaxed);
        for _ in 0..CALLS {
            black_box(black_box(&case.serializer).resolve_field_key(black_box(&case.path)));
        }
        let allocations = ALLOCATIONS.load(Ordering::Relaxed);
        let reallocations = REALLOCATIONS.load(Ordering::Relaxed);
        let bytes = BYTES.load(Ordering::Relaxed);
        println!(
            "{},{CALLS},{allocations},{reallocations},{bytes}",
            case.name
        );
    }
}
