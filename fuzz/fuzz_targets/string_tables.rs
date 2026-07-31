#![no_main]

use libfuzzer_sys::fuzz_target;
use pbdems2::DecodeLimits;
use pbdems2::entity::{CreateStringTable, StringTableContainer, UpdateStringTable};

fuzz_target!(|input: &[u8]| {
    let entries = input.first().copied().unwrap_or(0) as i8 as i32;
    let flags = input.get(1).copied().unwrap_or(0);
    let data = input.get(2..).unwrap_or_default().to_vec();
    let mut create = CreateStringTable::new("fuzz", entries, data.clone());
    if flags & 1 != 0 {
        create = create.with_varint_bitcounts();
    }
    if flags & 2 != 0 {
        create = create.with_fixed_user_data(i32::from(flags >> 2), i32::from(flags & 31));
    }
    if flags & 4 != 0 {
        create = create.with_compressed_data();
    }

    let limits = DecodeLimits::default()
        .with_max_string_table_bytes(4 * 1024, 16 * 1024)
        .with_max_string_table_entries(256)
        .with_max_string_table_user_data_bytes(4 * 1024);
    let mut tables = StringTableContainer::new();
    if tables.handle_create_with_limits(create, &limits).is_ok() {
        let update = UpdateStringTable::new(0, entries, data);
        let _ = tables.handle_update_with_limits(update, &limits);
    }
});
