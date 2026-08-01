use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::field_path::FieldPath;
use pbdems2::entity::{
    ClassEntry, ClassInfo, ENTITY_HANDLE_INDEX_MASK, EntityContainer, FieldDecodeContext,
    PacketEntities, StringTableContainer,
};
use pbdems2::position::cell_to_world;
use pbdems2_bench::{
    bench_class_info, bool_serializer_container, entity_container, packet_entity_creates,
    packet_entity_updates, populated_container, serializer_container,
};

const SLOT_COUNT: usize = 16_384;

fn entity_container_benchmarks(criterion: &mut Criterion) {
    let dense = entity_container(SLOT_COUNT, 1, 16);
    let sparse = entity_container(SLOT_COUNT, 8, 16);
    let mut group = criterion.benchmark_group("entity_container");

    group.throughput(Throughput::Elements(SLOT_COUNT as u64));
    group.bench_function("indexed_lookup_dense", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0_i32;
            for index in 0..SLOT_COUNT as i32 {
                checksum ^= dense
                    .get(black_box(index))
                    .expect("dense entity exists")
                    .class_id;
            }
            black_box(checksum)
        });
    });

    group.bench_function("handle_lookup_dense", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0_i32;
            for index in 0..SLOT_COUNT as u32 {
                let handle = (17 << 14) | index;
                checksum ^= dense
                    .get_by_handle(black_box(handle))
                    .expect("dense entity exists")
                    .class_id;
            }
            black_box(checksum)
        });
    });

    group.bench_function("iterate_dense", |bencher| {
        bencher.iter(|| {
            black_box(dense.iter().fold(0_i64, |sum, (index, entity)| {
                sum + i64::from(index) + i64::from(entity.class_id)
            }))
        });
    });

    group.bench_function("iterate_sparse_one_in_eight", |bencher| {
        bencher.iter(|| {
            black_box(sparse.iter().fold(0_i64, |sum, (index, entity)| {
                sum + i64::from(index) + i64::from(entity.class_id)
            }))
        });
    });

    group.bench_function("len_dense", |bencher| {
        bencher.iter(|| black_box(dense.len()));
    });
    group.bench_function("len_sparse", |bencher| {
        bencher.iter(|| black_box(sparse.len()));
    });

    group.finish();
}

fn entity_field_benchmarks(criterion: &mut Criterion) {
    let entities = entity_container(1, 1, 64);
    let entity = entities.get(0).expect("entity exists");
    let serializers = serializer_container(64);
    let serializer = serializers
        .get("CBenchmarkEntity")
        .expect("serializer exists");
    let integer_key = FieldPath {
        data: [0, 0, 0, 0, 0, 0, 0],
        last: 0,
        finished: false,
    }
    .pack();
    let vector_key = FieldPath {
        data: [3, 0, 0, 0, 0, 0, 0],
        last: 0,
        finished: false,
    }
    .pack();

    let mut group = criterion.benchmark_group("entity_fields");
    group.throughput(Throughput::Elements(1));

    group.bench_function("typed_integer_lookup", |bencher| {
        bencher.iter(|| black_box(entity.get_u64(black_box(Some(integer_key)))));
    });
    group.bench_function("typed_vector_lookup", |bencher| {
        bencher.iter(|| black_box(entity.get_vector3(black_box(Some(vector_key)))));
    });
    group.bench_function("resolve_and_lookup_by_name", |bencher| {
        bencher.iter(|| black_box(entity.get_by_name(black_box("m_field_63"), serializer)));
    });
    group.bench_function("world_position", |bencher| {
        bencher.iter(|| {
            black_box(entity.world_position([Some(integer_key); 3], [Some(integer_key); 3]))
        });
    });

    group.finish();
}

fn class_and_position_benchmarks(criterion: &mut Criterion) {
    let entries: Vec<ClassEntry> = (0..1_024)
        .map(|class_id| ClassEntry::new(class_id, format!("CClass{class_id}"), ""))
        .collect();
    let class_info = ClassInfo::try_from_entries(entries.clone()).expect("valid classes");
    let mut group = criterion.benchmark_group("class_and_position");

    group.throughput(Throughput::Elements(entries.len() as u64));
    group.bench_function("build_class_info_1024", |bencher| {
        bencher.iter(|| {
            black_box(
                ClassInfo::try_from_entries(black_box(entries.clone())).expect("valid classes"),
            )
        });
    });
    group.bench_function("class_id_lookup_1024", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0_usize;
            for class_id in 0..1_024 {
                checksum ^= class_info
                    .by_id(black_box(class_id))
                    .expect("class exists")
                    .network_name
                    .len();
            }
            black_box(checksum)
        });
    });

    group.throughput(Throughput::Elements(16_384));
    group.bench_function("cell_to_world", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0.0_f32;
            for cell in 0..16_384 {
                checksum += cell_to_world(black_box(cell), black_box(127.5));
            }
            black_box(checksum)
        });
    });

    group.finish();
}

fn handle_mask_benchmark(criterion: &mut Criterion) {
    criterion.bench_function(
        &format!("entity_handle_mask/{ENTITY_HANDLE_INDEX_MASK:#x}"),
        |bencher| {
            bencher.iter(|| {
                let handle = black_box(0x1234_5678_u32);
                black_box(handle & ENTITY_HANDLE_INDEX_MASK)
            });
        },
    );
}

/// The decode hot path: `handle_packet_entities` over a whole tick's worth of
/// entity deltas. Every other benchmark in this file measures reads of entities
/// that are *already* decoded; this one measures producing them.
fn packet_entities_benchmarks(criterion: &mut Criterion) {
    const ENTITY_COUNT: usize = 512;
    const FIELD_COUNT: usize = 16;

    let classes = bench_class_info();
    let serializers = bool_serializer_container(FIELD_COUNT);
    let tables = StringTableContainer::new();
    let creates = packet_entity_creates(ENTITY_COUNT, FIELD_COUNT);
    let updates = packet_entity_updates(ENTITY_COUNT, FIELD_COUNT);

    let mut group = criterion.benchmark_group("packet_entities");
    group.throughput(Throughput::Elements(ENTITY_COUNT as u64));

    group.bench_function("decode_creates_512x16", |bencher| {
        bencher.iter(|| {
            let mut container = EntityContainer::new();
            container.reserve_slots(ENTITY_COUNT).expect("valid slots");
            let mut context = FieldDecodeContext::new(1.0 / 64.0);
            let mut paths = Vec::new();
            container
                .handle_packet_entities(
                    PacketEntities::new(ENTITY_COUNT as i32, black_box(&creates), 0),
                    &classes,
                    &serializers,
                    &tables,
                    &mut context,
                    &mut paths,
                )
                .expect("creates decode");
            black_box(container.len())
        });
    });

    group.bench_function("decode_updates_512x16", |bencher| {
        let mut container = populated_container(ENTITY_COUNT, FIELD_COUNT);
        let mut context = FieldDecodeContext::new(1.0 / 64.0);
        let mut paths = Vec::new();
        bencher.iter(|| {
            container
                .handle_packet_entities(
                    PacketEntities::new(ENTITY_COUNT as i32, black_box(&updates), 0),
                    &classes,
                    &serializers,
                    &tables,
                    &mut context,
                    &mut paths,
                )
                .expect("updates decode");
            container.clear_updated();
            black_box(container.len())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    entity_container_benchmarks,
    entity_field_benchmarks,
    class_and_position_benchmarks,
    handle_mask_benchmark,
    packet_entities_benchmarks
);
criterion_main!(benches);
