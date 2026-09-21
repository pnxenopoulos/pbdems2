use awpy::weapons::weapon_info;
use awpy_output_bench::row_workloads::{
    LOADOUTS, RowDispatch, direct_tick_rows, fresh_tick_rows, loadout, prebound_loadout,
    reused_tick_rows,
};
use awpy_output_bench::rows;
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

fn inventory(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("awpy_inventory");
    group.throughput(Throughput::Elements(1));
    for &(name, classes) in LOADOUTS {
        let infos: Vec<_> = classes
            .iter()
            .filter_map(|class| weapon_info((*class)?))
            .collect();
        // Prebinding is outside the timer. This control excludes cache construction
        // and runtime class-ID lookup, so it only estimates possible headroom.
        for (method, hint) in [("current", 0), ("reserve_64", 64)] {
            group.bench_function(BenchmarkId::new(method, name), |b| {
                b.iter(|| black_box(loadout(black_box(classes), hint)));
            });
        }
        group.bench_function(BenchmarkId::new("prebound_reserve_64", name), |b| {
            b.iter(|| black_box(prebound_loadout(black_box(&infos), 64)));
        });
    }
    group.finish();
}

fn row_dispatch(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("awpy_tick_row_dispatch");
    for count in [10_000, 100_000] {
        let states = rows(count);
        group.throughput(Throughput::Elements(count as u64));
        for (name, dispatch) in [
            ("fresh", fresh_tick_rows as RowDispatch),
            ("reused", reused_tick_rows as RowDispatch),
            ("direct", direct_tick_rows as RowDispatch),
        ] {
            group.bench_function(BenchmarkId::new(name, count), |b| {
                // Keep the synthetic input buffer's destruction outside the
                // timer, just like row cloning and output destruction.
                b.iter_batched_ref(
                    || states.clone(),
                    |states| black_box(dispatch(black_box(states), black_box(10))),
                    BatchSize::LargeInput,
                );
            });
        }
    }
    group.finish();
}

criterion_group!(benches, inventory, row_dispatch);
criterion_main!(benches);
