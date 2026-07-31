use pbdems2::demo::{Demo, MAGIC};
use pbdems2::entity::field_path::FieldPath;
use pbdems2::entity::{Entity, FieldValue};
use pbdems2::io::BitReader;
use proptest::collection::vec;
use proptest::prelude::*;
use rustc_hash::FxHashMap;

fn encode_uvarint32(mut value: u32, output: &mut Vec<u8>) {
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

fn reference_bits(data: &[u8], start: usize, count: usize) -> u64 {
    (0..count).fold(0_u64, |value, index| {
        let position = start + index;
        let bit = (data[position / 8] >> (position % 8)) & 1;
        value | u64::from(bit) << index
    })
}

proptest! {
    #[test]
    fn arbitrary_bit_reads_match_a_bitwise_reference(
        data in vec(any::<u8>(), 1..=40),
        raw_start in any::<usize>(),
        raw_count in 0_usize..=64,
    ) {
        let total_bits = data.len() * 8;
        let start = raw_start % total_bits;
        let count = raw_count.min(total_bits - start);
        let mut reader = BitReader::new(&data);
        for _ in 0..start {
            reader.read_bool().unwrap();
        }

        let before = reader.position();
        let expected = reference_bits(&data, start, count);
        prop_assert_eq!(reader.peek_bits(count).unwrap(), expected);
        prop_assert_eq!(reader.position(), before);
        prop_assert_eq!(reader.read_bits(count).unwrap(), expected);
        prop_assert_eq!(reader.position(), before + count);
    }

    #[test]
    fn unsigned_varints_round_trip(value in any::<u32>()) {
        let mut encoded = Vec::new();
        encode_uvarint32(value, &mut encoded);
        let mut reader = BitReader::new(&encoded);

        prop_assert_eq!(reader.read_uvarint32().unwrap(), value);
        prop_assert_eq!(reader.bits_remaining(), 0);
    }

    #[test]
    fn packed_field_paths_round_trip(
        data in any::<[u8; 7]>(),
        last in 0_usize..7,
    ) {
        let original = FieldPath { data, last, finished: false };
        let unpacked = FieldPath::unpack(original.pack());

        prop_assert_eq!(unpacked.data, original.data);
        prop_assert_eq!(unpacked.last, original.last);
        prop_assert!(!unpacked.finished);
    }

    #[test]
    fn one_command_demo_round_trips_through_the_stream_driver(
        command in 0_u32..64,
        tick in 0_u32..1_000_000,
        body in vec(any::<u8>(), 0..=256),
    ) {
        let mut data = Vec::from(MAGIC);
        data.extend_from_slice(&[0; 8]);
        encode_uvarint32(command, &mut data);
        encode_uvarint32(tick, &mut data);
        encode_uvarint32(body.len() as u32, &mut data);
        data.extend_from_slice(&body);

        let demo = Demo::new(&data).unwrap();
        let frames = demo.commands().collect::<pbdems2::Result<Vec<_>>>().unwrap();
        prop_assert_eq!(frames.len(), 1);
        prop_assert_eq!(frames[0].header().cmd, command as i32);
        prop_assert_eq!(frames[0].header().tick, tick as i32);
        prop_assert!(!frames[0].header().compressed);
        prop_assert_eq!(frames[0].encoded_body(), body.as_slice());

        let mut decoded = Vec::new();
        frames[0].decode_body(&mut decoded).unwrap();
        prop_assert_eq!(decoded, body);
    }
}

#[test]
fn strict_entity_access_preserves_range_and_utf8_errors() {
    let integer_key = FieldPath {
        data: [1, 0, 0, 0, 0, 0, 0],
        last: 0,
        finished: false,
    }
    .pack();
    let string_key = FieldPath {
        data: [2, 0, 0, 0, 0, 0, 0],
        last: 0,
        finished: false,
    }
    .pack();
    let mut fields = FxHashMap::default();
    fields.insert(integer_key, FieldValue::I32(-1));
    fields.insert(string_key, FieldValue::String(vec![0xff]));
    let entity = Entity::from_fields(1, 0, 0, "CTest", true, fields).unwrap();

    assert_eq!(entity.try_get::<i64>(Some(integer_key)).unwrap(), Some(-1));
    assert!(entity.try_get::<u32>(Some(integer_key)).is_err());
    assert_eq!(entity.try_get::<u32>(None).unwrap(), None);
    assert_eq!(entity.get_bytes(Some(string_key)), Some(&[0xff][..]));
    assert!(entity.get_str(Some(string_key)).is_err());
    assert_eq!(entity.get_string(Some(string_key)).as_deref(), Some("�"));
}
