//! Separate allocation diagnostics. Never use this binary for timing.
use awpy::weapons::weapon_info;
use awpy_output_bench::row_workloads::{
    LOADOUTS, RowDispatch, direct_tick_rows, fresh_tick_rows, loadout, prebound_loadout,
    reused_tick_rows,
};
use awpy_output_bench::rows;
use pbdems2_allocation_probe::{CountedSystem, Snapshot, begin, end};
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: CountedSystem = CountedSystem;

fn report(name: &str, snapshot: Snapshot, capacity: usize) {
    assert_eq!(snapshot.underflows, 0);
    let allocations: u64 = snapshot.stages.iter().map(|s| s.allocations).sum();
    let reallocations: u64 = snapshot.stages.iter().map(|s| s.reallocations).sum();
    let bytes: u64 = snapshot.stages.iter().map(|s| s.requested_bytes).sum();
    println!(
        "{name}: alloc={allocations} realloc={reallocations} requested_bytes={bytes} inventory_capacity={capacity}"
    );
}

fn main() {
    for &(name, classes) in LOADOUTS {
        let infos: Vec<_> = classes
            .iter()
            .filter_map(|class| weapon_info((*class)?))
            .collect();
        for hint in [0, 64] {
            begin();
            let state = black_box(loadout(black_box(classes), hint));
            let snapshot = end();
            report(
                &format!("{name}/reserve_{hint}"),
                snapshot,
                state.inventory.capacity(),
            );
        }
        begin();
        let state = black_box(prebound_loadout(black_box(&infos), 64));
        let snapshot = end();
        report(
            &format!("{name}/prebound_64"),
            snapshot,
            state.inventory.capacity(),
        );
    }
    for (name, dispatch) in [
        ("fresh", fresh_tick_rows as RowDispatch),
        ("reused", reused_tick_rows),
        ("direct", direct_tick_rows),
    ] {
        let mut states = rows(100_000);
        begin();
        let columns = black_box(dispatch(black_box(&mut states), black_box(10)));
        let snapshot = end();
        report(name, snapshot, 0);
        drop(columns);
    }
}
