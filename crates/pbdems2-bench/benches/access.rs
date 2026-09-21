use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::EntityContainer;
use pbdems2_bench::{BENCH_CLASS, bool_serializer_container, entity, populated_container};

fn occupancy_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("entity_occupancy");
    for slots in [64, 16_384] {
        for shape in ["empty", "dense", "last_only"] {
            let mut container = EntityContainer::new();
            container.reserve_slots(slots).expect("valid slots");
            let indices = match shape {
                "dense" => 0..slots,
                "last_only" => slots - 1..slots,
                _ => 0..0,
            };
            for index in indices {
                container
                    .insert(entity(index as i32, 0))
                    .expect("valid entity");
            }
            container.clear_tick_changes();
            assert_eq!(container.len(), container.iter().count());
            let label = format!("{shape}_{slots}");
            group.bench_function(BenchmarkId::new("len", &label), |b| {
                b.iter(|| black_box(&container).len());
            });
            group.bench_function(BenchmarkId::new("is_empty", &label), |b| {
                b.iter(|| black_box(&container).is_empty());
            });
        }
    }
    group.finish();
}

fn field_lookup_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("matched_field_lookup");
    group.throughput(Throughput::Elements(1));
    for fields in [16, 64, 128] {
        let serializers = bool_serializer_container(fields);
        let serializer = serializers.get(BENCH_CLASS).expect("serializer exists");
        let container = populated_container(1, fields);
        let entity = container.get(0).expect("entity exists");
        let name = format!("m_bField{}", fields - 1);
        let key = serializer.resolve_field_key(&name).expect("field exists");
        // Both paths return the same borrowed value from the same entity.
        assert!(std::ptr::eq(
            entity.get_by_name(&name, serializer).expect("value exists"),
            entity.field_value(Some(key)).expect("value exists")
        ));
        group.bench_function(BenchmarkId::new("by_name", fields), |b| {
            b.iter(|| black_box(entity).get_by_name(black_box(&name), black_box(serializer)));
        });
        group.bench_function(BenchmarkId::new("cached_key", fields), |b| {
            b.iter(|| black_box(entity).field_value(black_box(Some(key))));
        });
    }
    group.finish();
}

fn sampled_fields_benchmarks(criterion: &mut Criterion) {
    const ENTITIES: usize = 64;
    const FIELDS: usize = 128;
    let serializers = bool_serializer_container(FIELDS);
    let serializer = serializers.get(BENCH_CLASS).expect("serializer exists");
    let container = populated_container(ENTITIES, FIELDS);
    let names: Vec<_> = (0..8).map(|i| format!("m_bField{}", i * 17)).collect();
    let keys: Vec<_> = names
        .iter()
        .map(|name| serializer.resolve_field_key(name))
        .collect();
    assert!(keys.iter().all(Option::is_some));
    for (_, entity) in container.iter() {
        for (name, key) in names.iter().zip(&keys) {
            assert!(std::ptr::eq(
                entity.get_by_name(name, serializer).expect("value exists"),
                entity.field_value(*key).expect("value exists")
            ));
        }
    }

    let mut group = criterion.benchmark_group("sampled_fields");
    group.throughput(Throughput::Elements((ENTITIES * names.len()) as u64));
    group.bench_function("resolve_per_entity", |b| {
        b.iter(|| {
            for (_, entity) in black_box(&container).iter() {
                for name in black_box(&names) {
                    black_box(entity.get_by_name(name, black_box(serializer)));
                }
            }
        });
    });
    group.bench_function("cached_keys", |b| {
        b.iter(|| {
            for (_, entity) in black_box(&container).iter() {
                for key in black_box(&keys) {
                    black_box(entity.field_value(*key));
                }
            }
        });
    });
    group.bench_function("resolve_once_per_tick", |b| {
        let mut tick_keys = Vec::with_capacity(names.len());
        b.iter(|| {
            tick_keys.clear();
            tick_keys.extend(
                black_box(&names)
                    .iter()
                    .map(|name| black_box(serializer).resolve_field_key(name)),
            );
            for (_, entity) in black_box(&container).iter() {
                for key in &tick_keys {
                    black_box(entity.field_value(*key));
                }
            }
        });
    });
    group.throughput(Throughput::Elements(names.len() as u64));
    group.bench_function("build_key_cache", |b| {
        b.iter(|| {
            black_box(&names)
                .iter()
                .map(|name| black_box(serializer).resolve_field_key(name))
                .collect::<Vec<_>>()
        });
    });
    group.finish();
}

fn resolution_benchmarks(criterion: &mut Criterion) {
    let cases = pbdems2_bench::lookup_workloads::lookup_cases();
    let mut group = criterion.benchmark_group("lookup_resolution");
    for case in &cases {
        assert_eq!(case.serializer.resolve_field_key(&case.path), case.expected);
        group.bench_function(BenchmarkId::new("resolve", case.name), |b| {
            b.iter(|| black_box(&case.serializer).resolve_field_key(black_box(&case.path)));
        });
        // Measures tokenization/allocation alone, not a replacement resolver.
        group.bench_function(BenchmarkId::new("split_collect_control", case.name), |b| {
            b.iter(|| black_box(&case.path).split('.').collect::<Vec<_>>());
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    occupancy_benchmarks,
    field_lookup_benchmarks,
    sampled_fields_benchmarks,
    resolution_benchmarks
);
criterion_main!(benches);
