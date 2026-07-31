#![no_main]

use libfuzzer_sys::fuzz_target;
use pbdems2::io::BitReader;

fuzz_target!(|input: &[u8]| {
    let Some((&selector, data)) = input.split_first() else {
        return;
    };
    let mut reader = BitReader::new(data);
    for step in 0..=255_u8 {
        let result = match selector.wrapping_add(step) % 12 {
            0 => reader.read_bits(usize::from(selector % 65)).map(|_| ()),
            1 => reader.read_bool().map(|_| ()),
            2 => reader.read_u8().map(|_| ()),
            3 => reader.read_u16().map(|_| ()),
            4 => reader.read_u32().map(|_| ()),
            5 => reader.read_u64().map(|_| ()),
            6 => reader.read_uvarint32().map(|_| ()),
            7 => reader.read_uvarint64().map(|_| ()),
            8 => reader.read_varint32().map(|_| ()),
            9 => reader.read_varint64().map(|_| ()),
            10 => reader.read_bitcoord().map(|_| ()),
            _ => reader.read_bitnormal().map(|_| ()),
        };
        if result.is_err() {
            break;
        }
    }
});
