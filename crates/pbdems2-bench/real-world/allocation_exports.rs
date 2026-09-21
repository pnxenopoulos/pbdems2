//! Included only by isolated allocation-profiling copies of the Python bindings.
use pbdems2_allocation_probe::{CountedSystem, Snapshot};

#[global_allocator]
static ALLOCATOR: CountedSystem = CountedSystem;

// SAFETY: This diagnostic symbol is unique within a parser extension. The runner
// loads only one such extension per process and invokes it through the C ABI.
#[unsafe(no_mangle)]
pub extern "C" fn pbdems2_profile_alloc_begin() {
    pbdems2_allocation_probe::begin();
}

// SAFETY: The prefixed symbol and repr(C) return type match the ctypes bridge.
#[unsafe(no_mangle)]
pub extern "C" fn pbdems2_profile_alloc_end() -> Snapshot {
    pbdems2_allocation_probe::end()
}

