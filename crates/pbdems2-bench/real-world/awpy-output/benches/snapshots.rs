use awpy::PlayerState;
use awpy_output_bench::{columns_growing, columns_owned, reference_owned, rows};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use polars::prelude::{DataFrame, PolarsResult};
use std::hint::black_box;

type Convert = fn(Vec<PlayerState>) -> PolarsResult<DataFrame>;

fn output(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("awpy_snapshot_output");
    for count in [10_000, 100_000, 1_000_000] {
        let states = rows(count);
        assert!(
            columns_owned(states.clone())
                .unwrap()
                .equals_missing(&reference_owned(states.clone()).unwrap())
        );
        group.throughput(Throughput::Elements(count as u64));
        for (name, convert) in [
            ("row_scans", reference_owned as Convert),
            ("owned_columns", columns_owned as Convert),
            ("growing_columns", columns_growing as Convert),
        ] {
            group.bench_function(BenchmarkId::new(name, count), |b| {
                b.iter_batched(
                    || states.clone(),
                    |states| black_box(convert(states).unwrap()),
                    BatchSize::LargeInput,
                );
            });
        }
    }
    group.finish();
}
criterion_group!(benches, output);
criterion_main!(benches);
