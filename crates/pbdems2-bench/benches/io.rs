use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::io::{BitReader, ByteReader};

const BUFFER_SIZE: usize = 64 * 1024;

fn bit_reader_benchmarks(criterion: &mut Criterion) {
    let data: Vec<u8> = (0..BUFFER_SIZE)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(17))
        .collect();
    let mut group = criterion.benchmark_group("bit_reader");

    group.throughput(Throughput::Bytes(data.len() as u64));
    group.bench_function("read_bool", |bencher| {
        bencher.iter(|| {
            let mut reader = BitReader::new(black_box(&data));
            let mut checksum = 0_u64;
            while reader.bits_remaining() > 0 {
                checksum ^= u64::from(reader.read_bool().expect("fixture is in bounds"));
            }
            black_box(checksum)
        });
    });

    group.bench_function("read_u8_aligned", |bencher| {
        bencher.iter(|| {
            let mut reader = BitReader::new(black_box(&data));
            let mut checksum = 0_u64;
            while reader.bits_remaining() >= 8 {
                checksum ^= u64::from(reader.read_u8().expect("fixture is in bounds"));
            }
            black_box(checksum)
        });
    });

    group.bench_function("read_u8_unaligned", |bencher| {
        bencher.iter(|| {
            let mut reader = BitReader::new(black_box(&data));
            reader.skip_bits(3).expect("fixture is in bounds");
            let mut checksum = 0_u64;
            while reader.bits_remaining() >= 8 {
                checksum ^= u64::from(reader.read_u8().expect("fixture is in bounds"));
            }
            black_box(checksum)
        });
    });

    for width in [1_usize, 6, 17, 32, 57, 64] {
        group.bench_with_input(
            BenchmarkId::new("read_bits", width),
            &width,
            |bencher, &width| {
                bencher.iter(|| {
                    let mut reader = BitReader::new(black_box(&data));
                    let mut checksum = 0_u64;
                    while reader.bits_remaining() >= width {
                        checksum ^= reader.read_bits(width).expect("fixture is in bounds");
                    }
                    black_box(checksum)
                });
            },
        );
    }

    let varints: Vec<u8> = [0xac, 0x02].repeat(BUFFER_SIZE / 2);
    group.throughput(Throughput::Elements((varints.len() / 2) as u64));
    group.bench_function("read_uvarint32", |bencher| {
        bencher.iter(|| {
            let mut reader = BitReader::new(black_box(&varints));
            let mut checksum = 0_u64;
            while reader.bits_remaining() >= 16 {
                checksum ^= u64::from(reader.read_uvarint32().expect("valid varint"));
            }
            black_box(checksum)
        });
    });

    let coord_data = vec![0_u8; BUFFER_SIZE];
    group.throughput(Throughput::Elements(32_768));
    group.bench_function("read_bitcoord_zero", |bencher| {
        bencher.iter(|| {
            let mut reader = BitReader::new(black_box(&coord_data));
            let mut checksum = 0.0_f32;
            for _ in 0..32_768 {
                checksum += reader.read_bitcoord().expect("fixture is in bounds");
            }
            black_box(checksum)
        });
    });

    group.finish();
}

fn byte_reader_benchmarks(criterion: &mut Criterion) {
    let fixed: Vec<u8> = (0..BUFFER_SIZE)
        .map(|index| (index as u8).wrapping_mul(13))
        .collect();
    let mut group = criterion.benchmark_group("byte_reader");

    group.throughput(Throughput::Bytes(fixed.len() as u64));
    group.bench_function("read_u32", |bencher| {
        bencher.iter(|| {
            let mut reader = ByteReader::new(black_box(&fixed));
            let mut checksum = 0_u32;
            while reader.remaining() >= 4 {
                checksum ^= reader.read_u32().expect("fixture is in bounds");
            }
            black_box(checksum)
        });
    });

    group.bench_function("read_64_byte_slices", |bencher| {
        bencher.iter(|| {
            let mut reader = ByteReader::new(black_box(&fixed));
            let mut checksum = 0_u8;
            while reader.remaining() >= 64 {
                checksum ^= reader.read_bytes(64).expect("fixture is in bounds")[0];
            }
            black_box(checksum)
        });
    });

    let varints: Vec<u8> = [0xac, 0x02].repeat(BUFFER_SIZE / 2);
    group.throughput(Throughput::Elements((varints.len() / 2) as u64));
    group.bench_function("read_uvarint32", |bencher| {
        bencher.iter(|| {
            let mut reader = ByteReader::new(black_box(&varints));
            let mut checksum = 0_u32;
            while reader.remaining() >= 2 {
                checksum ^= reader.read_uvarint32().expect("valid varint");
            }
            black_box(checksum)
        });
    });

    group.finish();
}

criterion_group!(benches, bit_reader_benchmarks, byte_reader_benchmarks);
criterion_main!(benches);
