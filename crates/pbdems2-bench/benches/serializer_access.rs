use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::{FieldDecodeContext, PacketEntities, StringTableContainer};
use pbdems2_bench::serializer_workloads::SerializerWorkload;
use std::collections::HashSet;
use std::hint::black_box;

fn updates(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("serializer_access");
    let tables = StringTableContainer::new();
    let filter = HashSet::new();
    for class_count in [1, 16, 128] {
        for fields in [1, 24] {
            let fixture = SerializerWorkload::new(class_count, 512, fields);
            for skipped in [false, true] {
                let mut entities = fixture.populated(skipped);
                let mut context = FieldDecodeContext::new(1.0 / 64.0);
                let mut paths = Vec::new();
                let mode = if skipped { "skip" } else { "retain" };
                group.throughput(Throughput::Elements(512 * fields as u64));
                group.bench_function(
                    BenchmarkId::new(mode, format!("{class_count}_classes/{fields}_fields")),
                    |b| {
                        b.iter(|| {
                            let packet = PacketEntities::new(512, black_box(&fixture.updates), 0);
                            if skipped {
                                entities
                                    .handle_packet_entities_filtered(
                                        packet,
                                        &fixture.classes,
                                        &fixture.serializers,
                                        &tables,
                                        &mut context,
                                        &filter,
                                        &mut paths,
                                    )
                                    .unwrap();
                            } else {
                                entities
                                    .handle_packet_entities(
                                        packet,
                                        &fixture.classes,
                                        &fixture.serializers,
                                        &tables,
                                        &mut context,
                                        &mut paths,
                                    )
                                    .unwrap();
                            }
                            black_box(&entities);
                            entities.clear_tick_changes();
                        });
                    },
                );
            }
        }
    }
    group.finish();
}
criterion_group!(benches, updates);
criterion_main!(benches);
