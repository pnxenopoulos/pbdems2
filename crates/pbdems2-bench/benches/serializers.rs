use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::SerializerContainer;
use pbdems2::entity::field_path::FieldPath;
use pbdems2_bench::{BENCH_PROFILE, flattened_serializer, serializer_container};

fn serializer_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("serializers");

    for field_count in [16_usize, 128, 512] {
        group.throughput(Throughput::Elements(field_count as u64));
        group.bench_with_input(
            BenchmarkId::new("parse", field_count),
            &field_count,
            |bencher, &field_count| {
                bencher.iter_batched(
                    || flattened_serializer(field_count),
                    |fixture| {
                        black_box(
                            SerializerContainer::parse(fixture, BENCH_PROFILE)
                                .expect("valid serializer"),
                        )
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    let serializers = serializer_container(128);
    let serializer = serializers
        .get("CBenchmarkEntity")
        .expect("benchmark serializer exists");

    group.throughput(Throughput::Elements(1));
    group.bench_function("container_lookup", |bencher| {
        bencher.iter(|| black_box(serializers.get(black_box("CBenchmarkEntity"))));
    });

    group.bench_function("resolve_first_field", |bencher| {
        bencher.iter(|| black_box(serializer.resolve_field_key(black_box("m_field_0"))));
    });
    group.bench_function("resolve_last_of_128_fields", |bencher| {
        bencher.iter(|| black_box(serializer.resolve_field_key(black_box("m_field_127"))));
    });

    let last_key = serializer
        .resolve_field_key("m_field_127")
        .expect("field key exists");
    group.bench_function("field_name_from_key", |bencher| {
        bencher.iter(|| black_box(serializer.field_name_for_key(black_box(last_key))));
    });

    group.finish();
}

fn field_path_benchmarks(criterion: &mut Criterion) {
    let paths: Vec<FieldPath> = (0..4_096)
        .map(|index| FieldPath {
            data: [
                index as u8,
                (index >> 2) as u8,
                (index >> 4) as u8,
                (index >> 6) as u8,
                0,
                0,
                0,
            ],
            last: 3,
            finished: false,
        })
        .collect();
    let keys: Vec<u64> = paths.iter().map(FieldPath::pack).collect();
    let mut group = criterion.benchmark_group("field_path");
    group.throughput(Throughput::Elements(paths.len() as u64));

    group.bench_function("pack", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0_u64;
            for path in black_box(&paths) {
                checksum ^= path.pack();
            }
            black_box(checksum)
        });
    });

    group.bench_function("unpack", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0_usize;
            for &key in black_box(&keys) {
                checksum ^= FieldPath::unpack(key).get(3);
            }
            black_box(checksum)
        });
    });

    group.finish();
}

criterion_group!(benches, serializer_benchmarks, field_path_benchmarks);
criterion_main!(benches);
