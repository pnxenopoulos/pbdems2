use std::hint::black_box;
use std::io::Write;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::MappedDemo;
use pbdems2::demo::Demo;
use pbdems2_bench::workloads::framing_demo;
use tempfile::NamedTempFile;

fn scan(data: &[u8], decode: bool) -> u64 {
    let demo = Demo::new(data).expect("valid demo");
    let mut scratch = Vec::new();
    let mut checksum = 0;
    for frame in demo.commands() {
        let frame = frame.expect("valid command");
        checksum += frame.header().tick as u64;
        if decode {
            frame.decode_body(&mut scratch).expect("valid body");
            black_box(&scratch);
        }
    }
    checksum
}

fn input_benchmarks(criterion: &mut Criterion) {
    const COMMANDS: usize = 4096;
    let bytes = framing_demo(COMMANDS, 4096, false);
    let mut file = NamedTempFile::new().expect("temporary demo");
    file.write_all(&bytes).expect("write fixture");
    file.flush().expect("flush fixture");
    // SAFETY: this private temporary file is not changed while mapped. The map
    // drops before the file, including if a benchmark panics.
    let mapped = unsafe { MappedDemo::open(file.path()) }.expect("valid demo");

    // Touch every page before measuring. These are warm-cache input benchmarks,
    // not cold disk throughput or a measure of memory usage.
    black_box(scan(mapped.as_bytes(), true));
    let mut group = criterion.benchmark_group("input_warm_cache");
    group.throughput(Throughput::Elements(COMMANDS as u64));
    group.bench_function("heap_headers_4096", |b| {
        b.iter(|| black_box(scan(black_box(&bytes), false)));
    });
    group.bench_function("mmap_headers_4096", |b| {
        b.iter(|| black_box(scan(black_box(mapped.as_bytes()), false)));
    });
    group.throughput(Throughput::Bytes((COMMANDS * 4096) as u64));
    group.bench_function("heap_decode_16_mib", |b| {
        b.iter(|| black_box(scan(black_box(&bytes), true)));
    });
    group.bench_function("mmap_decode_16_mib", |b| {
        b.iter(|| black_box(scan(black_box(mapped.as_bytes()), true)));
    });
    group.bench_function("read_file_and_decode_16_mib", |b| {
        b.iter(|| {
            let owned = std::fs::read(black_box(file.path())).expect("read fixture");
            black_box(scan(&owned, true))
        });
    });
    group.bench_function("open_mmap_and_decode_16_mib", |b| {
        b.iter(|| {
            // SAFETY: the same unchanged private fixture is used for every map.
            let mapped = unsafe { MappedDemo::open(black_box(file.path())) }.expect("map fixture");
            black_box(scan(mapped.as_bytes(), true))
        });
    });
    group.finish();
}

criterion_group!(benches, input_benchmarks);
criterion_main!(benches);
