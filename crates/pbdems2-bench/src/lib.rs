use pbdems2::entity::field_path::FieldPath;
use pbdems2::entity::{
    BareCharEncoding, CreateStringTable, DecodeProfile, Entity, EntityContainer, FieldValue,
    FlattenedField, FlattenedSerializer, FlattenedSerializerDefinition, PreciseQAngleMode,
    SerializerContainer,
};

pub const BENCH_PROFILE: DecodeProfile = DecodeProfile::new(
    BareCharEncoding::NullTerminatedString,
    PreciseQAngleMode::Centered,
)
.with_ammo_field("m_ammo");

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

pub fn serializer_container(field_count: usize) -> SerializerContainer {
    SerializerContainer::parse(flattened_serializer(field_count), BENCH_PROFILE)
        .expect("benchmark serializer fixture must be valid")
}

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

pub fn create_string_table(name: &str, entry_count: usize) -> CreateStringTable {
    CreateStringTable::new(name, entry_count as i32, string_table_bits(entry_count))
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
