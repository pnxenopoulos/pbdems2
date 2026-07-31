use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::{
    CreateStringTable, StringTableContainer, StringTableEntry, UpdateStringTable,
};
use pbdems2_bench::{create_string_table, string_table_bits};

fn create_and_update_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("string_table_decode");

    for entry_count in [1_usize, 32, 256] {
        let encoded = string_table_bits(entry_count);
        group.throughput(Throughput::Elements(entry_count as u64));
        group.bench_with_input(
            BenchmarkId::new("create", entry_count),
            &entry_count,
            |bencher, &entry_count| {
                bencher.iter_batched(
                    || CreateStringTable::new("benchmark", entry_count as i32, encoded.clone()),
                    |message| {
                        let mut tables = StringTableContainer::new();
                        black_box(tables.handle_create(message).expect("valid table fixture"));
                        black_box(tables)
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }

    let mut base = StringTableContainer::new();
    base.handle_create(create_string_table("benchmark", 0))
        .expect("empty table fixture parses");
    let update_data = string_table_bits(256);
    group.throughput(Throughput::Elements(256));
    group.bench_function("update_256", |bencher| {
        bencher.iter_batched(
            || (base.clone(), update_data.clone()),
            |(mut tables, data)| {
                tables
                    .handle_update(UpdateStringTable::new(0, 256, data))
                    .expect("valid table update");
                black_box(tables)
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn snapshot_and_lookup_benchmarks(criterion: &mut Criterion) {
    let mut base = StringTableContainer::new();
    base.handle_create(create_string_table("benchmark", 0))
        .expect("empty table fixture parses");
    let entries: Vec<StringTableEntry> = (0..1_024)
        .map(|index| {
            StringTableEntry::new(Some(format!("entry-{index}")), Some(vec![index as u8; 16]))
        })
        .collect();

    let mut group = criterion.benchmark_group("string_table_container");
    group.throughput(Throughput::Elements(entries.len() as u64));
    group.bench_function("full_snapshot_1024", |bencher| {
        bencher.iter_batched(
            || (base.clone(), entries.clone()),
            |(mut tables, entries)| {
                tables
                    .do_full_update([("benchmark".to_owned(), entries)])
                    .expect("valid full update");
                black_box(tables)
            },
            BatchSize::SmallInput,
        );
    });

    let mut many_tables = StringTableContainer::new();
    for index in 0..64 {
        many_tables
            .handle_create(create_string_table(&format!("table-{index}"), 0))
            .expect("empty table fixture parses");
    }
    group.throughput(Throughput::Elements(1));
    group.bench_function("lookup_last_of_64", |bencher| {
        bencher.iter(|| black_box(many_tables.find_table(black_box("table-63"))));
    });

    group.finish();
}

criterion_group!(
    benches,
    create_and_update_benchmarks,
    snapshot_and_lookup_benchmarks
);
criterion_main!(benches);
