//! Shared fixtures for the pbdems2 benchmark suite.
//!
//! Everything here builds deterministic synthetic input so benchmark runs are
//! comparable across machines and do not depend on real demo files.
#![deny(missing_docs)]

use std::sync::OnceLock;

use pbdems2::DecodeLimits;
use pbdems2::entity::field_path::{FieldPath, read_field_paths_with_limits};
use pbdems2::entity::{
    BareCharEncoding, ClassEntry, ClassInfo, CreateStringTable, DecodeProfile, Entity,
    EntityContainer, FieldDecodeContext, FieldValue, FlattenedField, FlattenedSerializer,
    FlattenedSerializerDefinition, PacketEntities, PreciseQAngleMode, SerializerContainer,
    StringTableContainer,
};
use pbdems2::io::BitReader;

/// Decode profile shared by every benchmark, so runs stay comparable.
pub const BENCH_PROFILE: DecodeProfile = DecodeProfile::new(
    BareCharEncoding::NullTerminatedString,
    PreciseQAngleMode::Centered,
)
.with_ammo_field("m_ammo");

/// Build a synthetic flattened-serializer message with `field_count` fields
/// cycling through the common Source 2 field types.
pub fn flattened_serializer(field_count: usize) -> FlattenedSerializer {
    const TYPES: [&str; 8] = [
        "uint32",
        "int64",
        "float32",
        "bool",
        "Vector",
        "Vector2D",
        "QAngle",
        "CUtlSymbolLarge",
    ];

    let mut symbols = Vec::with_capacity(1 + field_count * 2);
    symbols.push("CBenchmarkEntity".to_owned());
    let mut fields = Vec::with_capacity(field_count);

    for index in 0..field_count {
        let type_symbol = symbols.len() as i32;
        symbols.push(TYPES[index % TYPES.len()].to_owned());
        let name_symbol = symbols.len() as i32;
        symbols.push(format!("m_field_{index}"));
        fields.push(FlattenedField::new(Some(type_symbol), Some(name_symbol)));
    }

    FlattenedSerializer::new(
        vec![FlattenedSerializerDefinition::new(
            Some(0),
            (0..field_count as i32).collect(),
        )],
        symbols,
        fields,
    )
}

/// Parse [`flattened_serializer`] into a ready-to-use container.
pub fn serializer_container(field_count: usize) -> SerializerContainer {
    SerializerContainer::parse(flattened_serializer(field_count), BENCH_PROFILE)
        .expect("benchmark serializer fixture must be valid")
}

/// Build one entity at `index` populated with `field_count` decoded fields.
pub fn entity(index: i32, field_count: usize) -> Entity {
    let mut fields = rustc_hash::FxHashMap::default();
    for field_index in 0..field_count {
        let key = FieldPath {
            data: [field_index as u8, 0, 0, 0, 0, 0, 0],
            last: 0,
            finished: false,
        }
        .pack();
        let value = match field_index % 4 {
            0 => FieldValue::U64(field_index as u64),
            1 => FieldValue::F32(field_index as f32 * 0.5),
            2 => FieldValue::Bool(field_index.is_multiple_of(2)),
            _ => FieldValue::Vector3([
                field_index as f32,
                field_index as f32 + 1.0,
                field_index as f32 + 2.0,
            ]),
        };
        fields.insert(key, value);
    }

    Entity::from_fields(
        index,
        index as u32,
        index % 64,
        "CBenchmarkEntity",
        true,
        fields,
    )
    .expect("valid benchmark entity")
}

/// Fill a container with `slot_count` slots, occupying every `stride`-th one.
pub fn entity_container(slot_count: usize, stride: usize, field_count: usize) -> EntityContainer {
    let mut container = EntityContainer::new();
    container
        .reserve_slots(slot_count)
        .expect("valid benchmark slot count");
    for index in (0..slot_count).step_by(stride) {
        container
            .insert(entity(index as i32, field_count))
            .expect("valid benchmark entity");
    }
    container
}

/// Encode `entry_count` string-table entries into a bit-packed payload.
pub fn string_table_bits(entry_count: usize) -> Vec<u8> {
    let mut writer = BitWriter::default();
    for index in 0..entry_count {
        writer.push_bool(true);
        writer.push_bool(true);
        writer.push_bool(false);
        for byte in format!("entry-{index:05}").bytes().chain([0]) {
            writer.push_bits(u64::from(byte), 8);
        }
        writer.push_bool(false);
    }
    writer.finish()
}

/// Wrap [`string_table_bits`] in a create-string-table message named `name`.
pub fn create_string_table(name: &str, entry_count: usize) -> CreateStringTable {
    CreateStringTable::new(name, entry_count as i32, string_table_bits(entry_count))
}

/// Class name used by every packet-entities fixture in this crate.
pub const BENCH_CLASS: &str = "CBenchEntity";

/// Serializer whose fields are all `bool`, so a field value is exactly one bit.
///
/// Keeping the value encoding trivial isolates the cost of the surrounding
/// delta loop — index deltas, field paths, decoder dispatch, map insertion —
/// which is what [`packet_entity_creates`] and [`packet_entity_updates`]
/// exist to measure.
pub fn bool_serializer(field_count: usize) -> FlattenedSerializer {
    let mut symbols = vec![BENCH_CLASS.to_owned(), "bool".to_owned()];
    let fields = (0..field_count)
        .map(|index| {
            symbols.push(format!("m_bField{index}"));
            FlattenedField::new(Some(1), Some(index as i32 + 2))
        })
        .collect();
    FlattenedSerializer::new(
        vec![FlattenedSerializerDefinition::new(
            Some(0),
            (0..field_count as i32).collect(),
        )],
        symbols,
        fields,
    )
}

/// Parsed [`bool_serializer`] container for the packet-entities benchmarks.
pub fn bool_serializer_container(field_count: usize) -> SerializerContainer {
    SerializerContainer::parse(bool_serializer(field_count), BENCH_PROFILE)
        .expect("benchmark bool serializer must be valid")
}

/// Single-class [`ClassInfo`] matching [`bool_serializer_container`].
pub fn bench_class_info() -> ClassInfo {
    ClassInfo::try_from_entries([ClassEntry::new(0, BENCH_CLASS, BENCH_CLASS)])
        .expect("benchmark class fixture must be valid")
}

// The field-path operation codes are a Huffman table internal to pbdems2, so
// the bit patterns for the two ops we need are recovered through the public
// decoder instead of being duplicated here. `PLUS_ONE` advances to the next
// field and emits a path; `FINISH` terminates the path list.
fn find_op_code(accept: impl Fn(&[FieldPath]) -> bool, suffix: &[bool]) -> Vec<bool> {
    let limits = DecodeLimits::default();
    for length in 1..=24usize {
        for pattern in 0..(1u32 << length) {
            let bits: Vec<bool> = (0..length).map(|i| (pattern >> i) & 1 == 1).collect();
            let mut writer = BitWriter::default();
            for bit in bits.iter().chain(suffix) {
                writer.push_bool(*bit);
            }
            let data = writer.finish();
            let mut reader = BitReader::new(&data);
            let mut paths = Vec::new();
            // `finish` zero-pads to a byte boundary, so a candidate shorter than
            // the real code can still decode by borrowing padding bits. Requiring
            // the reader to stop exactly at the emitted length rejects those.
            if read_field_paths_with_limits(&mut reader, &mut paths, &limits).is_ok()
                && reader.position() == bits.len() + suffix.len()
                && accept(&paths)
            {
                return bits;
            }
        }
    }
    panic!("could not recover a field-path operation code from the public decoder");
}

fn op_codes() -> &'static (Vec<bool>, Vec<bool>) {
    static CODES: OnceLock<(Vec<bool>, Vec<bool>)> = OnceLock::new();
    CODES.get_or_init(|| {
        // FINISH alone yields no paths at all.
        let finish = find_op_code(|paths| paths.is_empty(), &[]);
        // PLUS_ONE followed by FINISH yields exactly the first field, `[0]`.
        let plus_one = find_op_code(
            |paths| paths.len() == 1 && paths[0].last == 0 && paths[0].get(0) == 0,
            &finish,
        );

        // Validate the pair: three increments must address fields 0, 1, and 2.
        let mut writer = BitWriter::default();
        for _ in 0..3 {
            for bit in &plus_one {
                writer.push_bool(*bit);
            }
        }
        for bit in &finish {
            writer.push_bool(*bit);
        }
        let data = writer.finish();
        let mut reader = BitReader::new(&data);
        let mut paths = Vec::new();
        read_field_paths_with_limits(&mut reader, &mut paths, &DecodeLimits::default())
            .expect("recovered field-path codes must decode");
        assert_eq!(paths.len(), 3, "recovered PLUS_ONE code is wrong");
        for (expected, path) in paths.iter().enumerate() {
            assert_eq!(path.last, 0);
            assert_eq!(path.get(0), expected);
        }
        (plus_one, finish)
    })
}

// Emit `field_count` sequential field paths followed by one bit per field.
fn push_bool_fields(writer: &mut BitWriter, field_count: usize) {
    let (plus_one, finish) = op_codes();
    for _ in 0..field_count {
        for bit in plus_one {
            writer.push_bool(*bit);
        }
    }
    for bit in finish {
        writer.push_bool(*bit);
    }
    for index in 0..field_count {
        writer.push_bool(index % 3 == 0);
    }
}

/// Encode `count` entity creates, each setting `field_count` boolean fields.
///
/// Entities are created at consecutive indices starting from 0.
pub fn packet_entity_creates(count: usize, field_count: usize) -> Vec<u8> {
    let class_bits = bench_class_info().bits();
    let mut writer = BitWriter::default();
    for _ in 0..count {
        writer.push_ubitvar(0); // +1 relative to the previous index
        writer.push_bits(0b10, 2); // create
        writer.push_bits(0, class_bits); // class_id 0
        writer.push_bits(0, 17); // serial number
        writer.push_uvarint32(0);
        push_bool_fields(&mut writer, field_count);
    }
    writer.finish()
}

/// Encode `count` entity updates, each rewriting `field_count` boolean fields.
///
/// Targets the same consecutive indices as [`packet_entity_creates`], so the
/// container must already hold those entities.
pub fn packet_entity_updates(count: usize, field_count: usize) -> Vec<u8> {
    let mut writer = BitWriter::default();
    for _ in 0..count {
        writer.push_ubitvar(0);
        writer.push_bits(0b00, 2); // update
        push_bool_fields(&mut writer, field_count);
    }
    writer.finish()
}

/// Container pre-populated by replaying [`packet_entity_creates`].
pub fn populated_container(count: usize, field_count: usize) -> EntityContainer {
    let classes = bench_class_info();
    let serializers = bool_serializer_container(field_count);
    let tables = StringTableContainer::new();
    let mut context = FieldDecodeContext::new(1.0 / 64.0);
    let mut paths = Vec::new();
    let mut container = EntityContainer::new();
    container
        .reserve_slots(count)
        .expect("benchmark slot count must be valid");
    let data = packet_entity_creates(count, field_count);
    container
        .handle_packet_entities(
            PacketEntities::new(count as i32, &data, 0),
            &classes,
            &serializers,
            &tables,
            &mut context,
            &mut paths,
        )
        .expect("benchmark create fixture must decode");
    container
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bit_position: usize,
}

impl BitWriter {
    fn push_bool(&mut self, value: bool) {
        self.push_bits(u64::from(value), 1);
    }

    /// Mirror of `BitReader::read_ubitvar`.
    fn push_ubitvar(&mut self, value: u32) {
        match value {
            0..=15 => self.push_bits(u64::from(value), 6),
            16..=255 => {
                self.push_bits(u64::from(16 | (value & 15)), 6);
                self.push_bits(u64::from(value >> 4), 4);
            }
            256..=4095 => {
                self.push_bits(u64::from(32 | (value & 15)), 6);
                self.push_bits(u64::from(value >> 4), 8);
            }
            _ => {
                self.push_bits(u64::from(48 | (value & 15)), 6);
                self.push_bits(u64::from(value >> 4), 28);
            }
        }
    }

    fn push_uvarint32(&mut self, mut value: u32) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            self.push_bits(u64::from(byte), 8);
            if value == 0 {
                break;
            }
        }
    }

    fn push_bits(&mut self, value: u64, count: usize) {
        for bit in 0..count {
            let byte_index = self.bit_position / 8;
            let bit_index = self.bit_position % 8;
            if byte_index == self.bytes.len() {
                self.bytes.push(0);
            }
            if value & (1 << bit) != 0 {
                self.bytes[byte_index] |= 1 << bit_index;
            }
            self.bit_position += 1;
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use pbdems2::entity::StringTableContainer;

    use super::*;

    #[test]
    fn packet_entity_fixtures_decode() {
        let field_count = 8;
        let count = 64;
        let classes = bench_class_info();
        let serializers = bool_serializer_container(field_count);
        let tables = StringTableContainer::new();
        let mut context = FieldDecodeContext::new(1.0 / 64.0);
        let mut paths = Vec::new();

        let mut container = EntityContainer::new();
        container.reserve_slots(count).unwrap();
        let creates = packet_entity_creates(count, field_count);
        container
            .handle_packet_entities(
                PacketEntities::new(count as i32, &creates, 0),
                &classes,
                &serializers,
                &tables,
                &mut context,
                &mut paths,
            )
            .expect("creates decode");
        assert_eq!(container.len(), count);
        let entity = container.get(0).expect("entity 0 exists");
        assert_eq!(entity.fields.len(), field_count);

        // Creates are recorded as changes too, so reset before measuring updates.
        container.clear_updated();
        let updates = packet_entity_updates(count, field_count);
        container
            .handle_packet_entities(
                PacketEntities::new(count as i32, &updates, 0),
                &classes,
                &serializers,
                &tables,
                &mut context,
                &mut paths,
            )
            .expect("updates decode");
        assert_eq!(container.len(), count);
        assert_eq!(container.updated_indices().len(), count);
    }

    #[test]
    fn serializer_fixture_is_valid() {
        let serializers = serializer_container(32);
        assert_eq!(
            serializers
                .get("CBenchmarkEntity")
                .expect("serializer exists")
                .fields
                .len(),
            32
        );
    }

    #[test]
    fn string_table_fixture_is_valid() {
        let mut tables = StringTableContainer::new();
        tables
            .handle_create(create_string_table("benchmark", 32))
            .expect("table fixture parses");
        assert_eq!(
            tables
                .find_table("benchmark")
                .expect("table exists")
                .entries()
                .len(),
            32
        );
    }
}
