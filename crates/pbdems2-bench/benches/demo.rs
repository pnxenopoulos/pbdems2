use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::demo::{
    CmdHeader, Demo, MAGIC, command, read_cmd_body, read_cmd_header, verify_header,
};
use pbdems2::io::ByteReader;

const BODY_SIZE: usize = 64 * 1024;
const COMPRESSED_FLAG: u32 = 1 << 31;
const COMMAND_COUNT: usize = 16_384;

fn write_uvarint32(mut value: u32, output: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn demo_fixture() -> Vec<u8> {
    let mut data = Vec::from(MAGIC);
    data.extend_from_slice(&[0; 8]);
    for index in 0..COMMAND_COUNT {
        let command = if index == 0 {
            command::SYNC_TICK
        } else if index % 1024 == 0 {
            command::FULL_PACKET
        } else {
            command::PACKET
        };
        write_uvarint32(command as u32, &mut data);
        write_uvarint32((index / 4) as u32, &mut data);
        write_uvarint32(8, &mut data);
        data.extend_from_slice(&(index as u64).to_le_bytes());
    }
    data
}

fn framing_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("demo_framing");

    let mut file_header = Vec::from(MAGIC);
    file_header.extend_from_slice(&[0; 8]);
    group.throughput(Throughput::Bytes(file_header.len() as u64));
    group.bench_function("verify_header", |bencher| {
        bencher.iter(|| verify_header(black_box(&file_header), 16).expect("valid header"));
    });

    let command_headers = [7_u8, 42, 3].repeat(16_384);
    group.throughput(Throughput::Elements(16_384));
    group.bench_function("read_command_headers", |bencher| {
        bencher.iter(|| {
            let mut reader = ByteReader::new(black_box(&command_headers));
            let mut checksum = 0_i32;
            while !reader.is_empty() {
                checksum ^= read_cmd_header(&mut reader, COMPRESSED_FLAG)
                    .expect("valid command header")
                    .tick;
            }
            black_box(checksum)
        });
    });

    let file = demo_fixture();
    let demo = Demo::new(&file).expect("valid benchmark demo");
    group.throughput(Throughput::Elements(COMMAND_COUNT as u64));
    group.bench_function("iterate_complete_demo", |bencher| {
        bencher.iter(|| {
            let mut checksum = 0_i64;
            for frame in demo.commands() {
                let frame = frame.expect("valid command frame");
                checksum ^= i64::from(frame.header().tick);
                checksum ^= frame.encoded_body().len() as i64;
            }
            black_box(checksum)
        });
    });

    group.bench_function("build_seek_index", |bencher| {
        bencher.iter(|| {
            let index = demo.index().expect("valid command index");
            black_box((
                index.distinct_ticks().len(),
                index.full_packets().len(),
                index.stream_start(),
            ))
        });
    });

    group.finish();
}

fn command_body_benchmarks(criterion: &mut Criterion) {
    let raw_body: Vec<u8> = (0..BODY_SIZE)
        .map(|index| (index as u8).wrapping_mul(17))
        .collect();
    let raw_header = CmdHeader::new(1, 1, false, raw_body.len() as u32);
    let compressed_body = snap::raw::Encoder::new()
        .compress_vec(&raw_body)
        .expect("compression succeeds");
    let compressed_header = CmdHeader::new(1, 1, true, compressed_body.len() as u32);

    let mut group = criterion.benchmark_group("command_body");
    group.throughput(Throughput::Bytes(raw_body.len() as u64));

    group.bench_function("copy_uncompressed_64_kib", |bencher| {
        let mut output = Vec::with_capacity(raw_body.len());
        bencher.iter(|| {
            let mut reader = ByteReader::new(black_box(&raw_body));
            read_cmd_body(&mut reader, &raw_header, &mut output).expect("valid body");
            black_box(output.len())
        });
    });

    group.bench_function("decompress_snappy_64_kib", |bencher| {
        let mut output = Vec::with_capacity(raw_body.len());
        bencher.iter(|| {
            let mut reader = ByteReader::new(black_box(&compressed_body));
            read_cmd_body(&mut reader, &compressed_header, &mut output).expect("valid body");
            black_box(output.len())
        });
    });

    group.finish();
}

criterion_group!(benches, framing_benchmarks, command_body_benchmarks);
criterion_main!(benches);
