use std::collections::HashSet;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::{DemoParser, ParserState};
use pbdems2_bench::{
    BENCH_CLASS,
    workloads::{PlaybackAdapter, playback_demo},
};

fn consume_tick(state: &ParserState) {
    black_box((
        state.tick(),
        state.entities().updated_indices(),
        state.entities().entity_changes(),
    ));
}

fn playback_benchmarks(criterion: &mut Criterion) {
    const ENTITIES: usize = 64;
    const FIELDS: usize = 16;
    const TICKS: usize = 128;
    let mut group = criterion.benchmark_group("playback");
    for compressed in [false, true] {
        let encoding = if compressed { "snappy" } else { "raw" };
        let bytes = playback_demo(ENTITIES, FIELDS, TICKS, compressed);
        let parser = DemoParser::new(&bytes).expect("valid demo");
        let adapter = PlaybackAdapter::new(ENTITIES, FIELDS);
        let prepared = parser.prepare(adapter, 1.0 / 64.0).expect("valid signon");

        group.throughput(Throughput::Elements(TICKS as u64));
        group.bench_function(BenchmarkId::new("fresh_run", encoding), |b| {
            b.iter(|| {
                let mut adapter = black_box(adapter);
                black_box(
                    parser
                        .run_to_end(&mut adapter, 1.0 / 64.0, consume_tick)
                        .expect("valid run"),
                )
            });
        });
        group.bench_function(BenchmarkId::new("prepared_run", encoding), |b| {
            b.iter(|| {
                black_box(
                    prepared
                        .session(black_box(parser))
                        .expect("same demo")
                        .run_to_end(consume_tick)
                        .expect("valid run"),
                )
            });
        });
        let filter = HashSet::from([BENCH_CLASS]);
        group.bench_function(BenchmarkId::new("filtered_run", encoding), |b| {
            b.iter(|| {
                black_box(
                    prepared
                        .session(black_box(parser))
                        .expect("same demo")
                        .run_to_end_filtered(&filter, consume_tick)
                        .expect("valid run"),
                )
            });
        });
        let segments = prepared.segment_plan(4);
        group.bench_function(
            BenchmarkId::new("four_segments_sequential", encoding),
            |b| {
                b.iter(|| {
                    for segment in &segments {
                        black_box(
                            prepared
                                .session(black_box(parser))
                                .expect("same demo")
                                .decode_segment(
                                    segment.start_offset(),
                                    segment.end_tick(),
                                    &filter,
                                    consume_tick,
                                )
                                .expect("valid segment"),
                        );
                    }
                });
            },
        );
    }
    group.finish();

    let bytes = playback_demo(ENTITIES, FIELDS, TICKS, false);
    let parser = DemoParser::new(&bytes).expect("valid demo");
    let adapter = PlaybackAdapter::new(ENTITIES, FIELDS);
    let prepared = parser.prepare(adapter, 1.0 / 64.0).expect("valid signon");
    let mut group = criterion.benchmark_group("prepared_playback");
    group.bench_function("prepare_signon_and_index", |b| {
        b.iter(|| {
            black_box(
                parser
                    .prepare(black_box(adapter), 1.0 / 64.0)
                    .expect("valid demo"),
            )
        });
    });
    group.bench_function("clone_populated_session", |b| {
        b.iter(|| black_box(prepared.session(black_box(parser)).expect("same demo")));
    });
    group.bench_function("seek_to_tick_126", |b| {
        b.iter(|| {
            black_box(
                prepared
                    .session(black_box(parser))
                    .expect("same demo")
                    .parse_to_tick(black_box(126))
                    .expect("valid seek"),
            )
        });
    });
    group.finish();
}

criterion_group!(benches, playback_benchmarks);
criterion_main!(benches);
