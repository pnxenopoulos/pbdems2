use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::field_path::read_field_paths;
use pbdems2::io::BitReader;
use pbdems2_bench::field_path_workloads::{records, validate};

fn real_paths(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("field_path_real");
    for game in ["cs2", "deadlock"] {
        for offset in [0, 3, 7] {
            for trailing in [0, 16] {
                let records = records(game, offset, trailing);
                validate(&records);
                let count: usize = records.iter().map(|record| record.fields).sum();
                let mut paths =
                    Vec::with_capacity(records.iter().map(|record| record.fields).max().unwrap());
                let mode = if trailing == 0 { "tail" } else { "packet" };
                group.throughput(Throughput::Elements(count as u64));
                group.bench_function(
                    BenchmarkId::new(game, format!("{mode}/offset_{offset}")),
                    |b| {
                        b.iter(|| {
                            for record in black_box(&records) {
                                let mut reader = BitReader::new(&record.bytes);
                                reader.skip_bits(record.offset).unwrap();
                                read_field_paths(&mut reader, &mut paths).unwrap();
                                black_box(&paths);
                            }
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

criterion_group!(benches, real_paths);
criterion_main!(benches);
