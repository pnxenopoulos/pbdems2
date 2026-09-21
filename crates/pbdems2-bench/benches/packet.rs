use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::PacketMessageIter;
use pbdems2::io::BitReader;
use pbdems2_bench::workloads::{packet_messages, varied_bytes};

fn packet_benchmarks(criterion: &mut Criterion) {
    const MESSAGES: usize = 256;
    let mut group = criterion.benchmark_group("packet_messages");
    for size in [16, 256, 4096] {
        let bytes = packet_messages(MESSAGES, size);
        group.throughput(Throughput::Elements(MESSAGES as u64));
        group.bench_with_input(
            BenchmarkId::new("headers_only", size),
            &bytes,
            |b, bytes| {
                b.iter(|| {
                    let mut checksum = 0;
                    for frame in PacketMessageIter::new(black_box(bytes)) {
                        let frame = frame.expect("valid message");
                        checksum ^= frame.message_type() as usize ^ frame.encoded_size();
                    }
                    black_box(checksum)
                });
            },
        );
        group.throughput(Throughput::Bytes((size * MESSAGES) as u64));
        group.bench_with_input(
            BenchmarkId::new("payload_or_copy", size),
            &bytes,
            |b, bytes| {
                let mut scratch = Vec::with_capacity(size);
                b.iter(|| {
                    for frame in PacketMessageIter::new(black_box(bytes)) {
                        let frame = frame.expect("valid message");
                        black_box(frame.payload_or_copy(&mut scratch).expect("valid payload"));
                    }
                });
            },
        );
    }
    group.finish();
}

fn bulk_read_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bulk_read");
    for size in [64, 4096, 65_536] {
        let bytes = varied_bytes(size + 1);
        group.throughput(Throughput::Bytes(size as u64));
        for offset in [0, 1, 3, 7] {
            group.bench_function(BenchmarkId::new(format!("offset_{offset}"), size), |b| {
                let mut output = vec![0; size];
                b.iter(|| {
                    let mut reader = BitReader::new(black_box(&bytes));
                    reader.skip_bits(offset).expect("valid offset");
                    reader.read_bytes(&mut output).expect("valid size");
                    black_box(&output);
                });
            });
        }
    }
    group.finish();
}

fn bulk_read_tail_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bulk_read_tails");
    for size in [0, 1, 7, 8, 9, 15, 16, 17, 63, 65] {
        // Three-bit alignment and exact input size exercise short reads and tails.
        let bytes = varied_bytes(size + 1);
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            let mut output = vec![0; size];
            b.iter(|| {
                let mut reader = BitReader::new(black_box(&bytes));
                reader.skip_bits(3).expect("valid offset");
                reader.read_bytes(&mut output).expect("valid size");
                black_box(&output);
                black_box(reader.position());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    packet_benchmarks,
    bulk_read_benchmarks,
    bulk_read_tail_benchmarks
);
criterion_main!(benches);
