use std::collections::HashSet;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::field_path::read_field_paths;
use pbdems2::entity::{EntityContainer, FieldDecodeContext, PacketEntities, StringTableContainer};
use pbdems2::io::BitReader;
use pbdems2_bench::entity_workloads::{EntityWorkload, MIXED_FIELDS, MIXED_LEAF};

fn packet_benchmarks(criterion: &mut Criterion) {
    const ENTITIES: usize = 512;
    let tables = StringTableContainer::new();
    let filter = HashSet::new();
    let mut group = criterion.benchmark_group("mixed_entities");
    for nested in [false, true] {
        for string_bytes in [0, 128] {
            let workload = EntityWorkload::new(ENTITIES, nested, string_bytes);
            let shape = if nested { "nested" } else { "flat" };
            let label = format!("{shape}_strings_{string_bytes}");
            group.throughput(Throughput::Elements((ENTITIES * MIXED_FIELDS) as u64));
            group.bench_function(BenchmarkId::new("create", &label), |b| {
                b.iter(|| {
                    let mut entities = EntityContainer::new();
                    let mut context = FieldDecodeContext::new(1.0 / 64.0);
                    let mut paths = Vec::new();
                    entities
                        .handle_packet_entities(
                            PacketEntities::new(ENTITIES as i32, black_box(&workload.creates), 0),
                            &workload.classes,
                            &workload.serializers,
                            &tables,
                            &mut context,
                            &mut paths,
                        )
                        .expect("valid mixed creates");
                    black_box(&entities);
                });
            });
            for (stride, fields) in [(32, 6), (1, MIXED_FIELDS)] {
                let count = ENTITIES.div_ceil(stride);
                let bytes = workload.updates(stride, fields);
                let label = format!("{label}/{count}x{fields}");
                group.throughput(Throughput::Elements((count * fields) as u64));
                for skip in [false, true] {
                    let name = if skip {
                        "skip_updates"
                    } else {
                        "retain_updates"
                    };
                    let mut entities = if skip {
                        workload.skipped()
                    } else {
                        workload.populated()
                    };
                    let mut context = FieldDecodeContext::new(1.0 / 64.0);
                    let mut paths = Vec::new();
                    group.bench_function(BenchmarkId::new(name, &label), |b| {
                        b.iter(|| {
                            let message = PacketEntities::new(count as i32, black_box(&bytes), 0);
                            if skip {
                                entities
                                    .handle_packet_entities_filtered(
                                        message,
                                        &workload.classes,
                                        &workload.serializers,
                                        &tables,
                                        &mut context,
                                        &filter,
                                        &mut paths,
                                    )
                                    .expect("valid skipped updates");
                            } else {
                                entities
                                    .handle_packet_entities(
                                        message,
                                        &workload.classes,
                                        &workload.serializers,
                                        &tables,
                                        &mut context,
                                        &mut paths,
                                    )
                                    .expect("valid retained updates");
                            }
                            black_box(&entities);
                            entities.clear_tick_changes();
                        });
                    });
                }
            }
        }
    }
    group.finish();
}

fn stage_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("mixed_stages");
    for nested in [false, true] {
        let workload = EntityWorkload::new(1, nested, 0);
        for count in [6, MIXED_FIELDS] {
            let bytes = workload.paths(count);
            let shape = if nested { "nested" } else { "flat" };
            group.throughput(Throughput::Elements(count as u64));
            group.bench_function(BenchmarkId::new("paths", format!("{shape}_{count}")), |b| {
                let mut paths = Vec::with_capacity(count);
                b.iter(|| {
                    read_field_paths(&mut BitReader::new(black_box(&bytes)), &mut paths)
                        .expect("valid paths");
                    black_box(&paths);
                });
            });
        }
    }
    for string_bytes in [0, 128] {
        let workload = EntityWorkload::new(1, false, string_bytes);
        let bytes = workload.values();
        let fields = &workload
            .serializers
            .get(MIXED_LEAF)
            .expect("leaf exists")
            .fields;
        group.throughput(Throughput::Elements(MIXED_FIELDS as u64));
        group.bench_function(BenchmarkId::new("decode_values", string_bytes), |b| {
            let mut context = FieldDecodeContext::new(1.0 / 64.0);
            b.iter(|| {
                let mut reader = BitReader::new(black_box(&bytes));
                for field in black_box(fields) {
                    black_box(
                        field
                            .metadata
                            .decoder
                            .decode(&mut context, &mut reader)
                            .expect("valid value"),
                    );
                }
            });
        });
        group.bench_function(BenchmarkId::new("skip_values", string_bytes), |b| {
            let mut context = FieldDecodeContext::new(1.0 / 64.0);
            b.iter(|| {
                let mut reader = BitReader::new(black_box(&bytes));
                for field in black_box(fields) {
                    field
                        .metadata
                        .decoder
                        .skip(&mut context, &mut reader)
                        .expect("valid value");
                }
                black_box(reader.position());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, packet_benchmarks, stage_benchmarks);
criterion_main!(benches);
