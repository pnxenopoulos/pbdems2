use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::field_path::{FieldPath, read_field_paths};
use pbdems2::entity::{
    FlattenedField, FlattenedSerializer, FlattenedSerializerDefinition, SerializerContainer,
};
use pbdems2::io::BitReader;
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

fn nested_name_benchmarks(criterion: &mut Criterion) {
    let serializers = SerializerContainer::parse(
        FlattenedSerializer::new(
            vec![
                FlattenedSerializerDefinition::new(Some(0), vec![0]),
                FlattenedSerializerDefinition::new(Some(1), vec![1]),
            ],
            [
                "CLeaf",
                "CRoot",
                "int32",
                "m_value",
                "CNetworkUtlVectorBase< CLeaf >",
                "m_items",
                "outer.node",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            vec![
                FlattenedField::new(Some(2), Some(3)),
                FlattenedField::new(Some(4), Some(5))
                    .with_serializer_name_sym(Some(0))
                    .with_send_node_sym(Some(6)),
            ],
        ),
        BENCH_PROFILE,
    )
    .expect("valid nested serializer fixture");
    let serializer = serializers.get("CRoot").unwrap();
    let name = "outer.node.m_items.12.m_value";
    let key = serializer.resolve_field_key(name).unwrap();
    assert_eq!(serializer.field_name_for_key(key).as_deref(), Some(name));
    let mut group = criterion.benchmark_group("serializer_names");
    group.bench_function("resolve_nested_array", |bencher| {
        bencher.iter(|| black_box(serializer.resolve_field_key(black_box(name))));
    });
    group.bench_function("format_nested_array", |bencher| {
        bencher.iter(|| black_box(serializer.field_name_for_key(black_box(key))));
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

    // PLUS_ONE, six PUSH_ONE_LEFT_DELTA_ZERO_RIGHT_ZERO operations, six
    // POP_ONE_PLUS_ONE operations, then FINISH: exercise all seven levels.
    let encoded = [
        0x36, 0x76, 0x63, 0x37, 0x76, 0x63, 0x37, 0x76, 0x63, 0x37, 0x86, 0x1b, 0xc3, 0x8d, 0xe1,
        0xc6, 0x70, 0x63, 0xb8, 0x31, 0x0c,
    ];
    let mut decoded = Vec::new();
    read_field_paths(&mut BitReader::new(&encoded), &mut decoded).unwrap();
    assert_eq!(decoded.len(), 13);
    assert_eq!(decoded[6].last, 6);
    assert_eq!(decoded[12].last, 0);
    group.throughput(Throughput::Elements(decoded.len() as u64));
    group.bench_function("decode_push_pop", |bencher| {
        bencher.iter(|| {
            read_field_paths(&mut BitReader::new(black_box(&encoded)), &mut decoded).unwrap();
            black_box(&decoded);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    serializer_benchmarks,
    nested_name_benchmarks,
    field_path_benchmarks
);
criterion_main!(benches);
