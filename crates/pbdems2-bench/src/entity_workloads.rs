//! Mixed entity deltas for measuring paths, value decoding, and retained state.

use std::collections::HashSet;
use std::sync::OnceLock;

use pbdems2::entity::{
    ClassInfo, EntityContainer, FieldDecodeContext, FlattenedField, FlattenedSerializer,
    FlattenedSerializerDefinition, PacketEntities, SerializerContainer, StringTableContainer,
};

use crate::{BENCH_CLASS, BENCH_PROFILE, BitWriter, bench_class_info, find_op_code, op_codes};

/// Five repetitions of bool, unsigned/signed varints, float, vector, and string.
pub const MIXED_FIELDS: usize = 30;
/// Name of the leaf schema, also used to isolate value decoding.
pub const MIXED_LEAF: &str = "CMixedState";

/// Deterministic packets with identical leaf values in flat and nested schemas.
pub struct EntityWorkload {
    /// Server class table shared by all packets.
    pub classes: ClassInfo,
    /// Parsed schema, with one nested parent when requested.
    pub serializers: SerializerContainer,
    /// Complete initial state.
    pub creates: Vec<u8>,
    /// Number of entity slots created.
    pub entities: usize,
    nested: bool,
    string_bytes: usize,
}

impl EntityWorkload {
    /// Build all packet data before timing. String fields contain no embedded NULs.
    pub fn new(entities: usize, nested: bool, string_bytes: usize) -> Self {
        assert!((1..=16_384).contains(&entities));
        let mut symbols = vec![MIXED_LEAF.into(), BENCH_CLASS.into()];
        let mut fields = Vec::new();
        let types = ["bool", "uint32", "int32", "float32", "Vector", "CUtlString"];
        for i in 0..MIXED_FIELDS {
            let ty = symbols.len() as i32;
            symbols.push(types[i % types.len()].into());
            let name = symbols.len() as i32;
            symbols.push(format!("value_{i:02}"));
            fields.push(FlattenedField::new(Some(ty), Some(name)));
        }
        let leaf_indices: Vec<_> = (0..MIXED_FIELDS as i32).collect();
        let root_indices = if nested {
            let name = symbols.len() as i32;
            symbols.push("state".into());
            fields.push(FlattenedField::new(Some(0), Some(name)).with_serializer_name_sym(Some(0)));
            vec![MIXED_FIELDS as i32]
        } else {
            leaf_indices.clone()
        };
        let serializers = SerializerContainer::parse(
            FlattenedSerializer::new(
                vec![
                    FlattenedSerializerDefinition::new(Some(0), leaf_indices),
                    FlattenedSerializerDefinition::new(Some(1), root_indices),
                ],
                symbols,
                fields,
            ),
            BENCH_PROFILE,
        )
        .expect("valid mixed schema");
        let classes = bench_class_info();
        let mut writer = BitWriter::default();
        for index in 0..entities {
            writer.push_ubitvar(0);
            writer.push_bits(0b10, 2);
            writer.push_bits(0, classes.bits());
            writer.push_bits(7, 17);
            writer.push_uvarint32(0);
            push_paths(&mut writer, nested, MIXED_FIELDS);
            push_values(&mut writer, index, MIXED_FIELDS, 0, string_bytes);
        }
        Self {
            classes,
            serializers,
            creates: writer.finish(),
            entities,
            nested,
            string_bytes,
        }
    }

    /// Replay creates outside a steady-state update measurement.
    pub fn populated(&self) -> EntityContainer {
        let mut entities = EntityContainer::new();
        entities
            .handle_packet_entities(
                PacketEntities::new(self.entities as i32, &self.creates, 0),
                &self.classes,
                &self.serializers,
                &StringTableContainer::new(),
                &mut FieldDecodeContext::new(1.0 / 64.0),
                &mut Vec::new(),
            )
            .expect("valid mixed creates");
        entities.clear_tick_changes();
        entities
    }

    /// Update every stride-th entity, changing the first few or all leaf fields.
    pub fn updates(&self, stride: usize, fields: usize) -> Vec<u8> {
        assert!(stride > 0);
        let mut writer = BitWriter::default();
        let mut previous = -1_i32;
        for index in (0..self.entities).step_by(stride) {
            writer.push_ubitvar((index as i32 - previous - 1) as u32);
            previous = index as i32;
            writer.push_bits(0b00, 2);
            push_paths(&mut writer, self.nested, fields);
            push_values(&mut writer, index, fields, 1, self.string_bytes);
        }
        writer.finish()
    }

    /// Field paths alone, for separating Huffman work from values and storage.
    pub fn paths(&self, fields: usize) -> Vec<u8> {
        let mut writer = BitWriter::default();
        push_paths(&mut writer, self.nested, fields);
        writer.finish()
    }

    /// One entity's values, without paths or packet headers.
    pub fn values(&self) -> Vec<u8> {
        let mut writer = BitWriter::default();
        push_values(&mut writer, 0, MIXED_FIELDS, 1, self.string_bytes);
        writer.finish()
    }

    /// Build skip-only state without retaining entity field values.
    pub fn skipped(&self) -> EntityContainer {
        let mut entities = EntityContainer::new();
        entities
            .handle_packet_entities_filtered(
                PacketEntities::new(self.entities as i32, &self.creates, 0),
                &self.classes,
                &self.serializers,
                &StringTableContainer::new(),
                &mut FieldDecodeContext::new(1.0 / 64.0),
                &HashSet::new(),
                &mut Vec::new(),
            )
            .expect("valid skipped creates");
        assert!(entities.is_empty());
        entities
    }
}

fn nested_first() -> &'static [bool] {
    static CODE: OnceLock<Vec<bool>> = OnceLock::new();
    CODE.get_or_init(|| {
        find_op_code(
            |paths| paths.len() == 1 && paths[0].last == 1 && paths[0].data[..2] == [0, 0],
            &op_codes().1,
        )
    })
}

fn push_paths(writer: &mut BitWriter, nested: bool, count: usize) {
    assert!((1..=MIXED_FIELDS).contains(&count));
    let (plus_one, finish) = op_codes();
    let first = if nested { nested_first() } else { plus_one };
    for &bit in first {
        writer.push_bool(bit);
    }
    for _ in 1..count {
        for &bit in plus_one {
            writer.push_bool(bit);
        }
    }
    for &bit in finish {
        writer.push_bool(bit);
    }
}

fn seed(entity: usize, field: usize, phase: usize) -> u32 {
    (entity * MIXED_FIELDS + field + phase) as u32
}

fn push_values(
    writer: &mut BitWriter,
    entity: usize,
    count: usize,
    phase: usize,
    string_bytes: usize,
) {
    for field in 0..count {
        let value = seed(entity, field, phase);
        match field % 6 {
            0 => writer.push_bool(value.is_multiple_of(2)),
            1 => writer.push_uvarint32(300 + value),
            2 => writer.push_uvarint32(value * 2 + 1), // zigzag(-value - 1)
            3 => writer.push_bits(u64::from((value as f32 * 0.5).to_bits()), 32),
            4 => {
                for offset in [0.0, 0.25, 0.5] {
                    writer.push_bits(u64::from((value as f32 + offset).to_bits()), 32);
                }
            }
            _ => {
                for _ in 0..string_bytes {
                    writer.push_bits(u64::from(b'a' + (value % 26) as u8), 8);
                }
                writer.push_bits(0, 8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pbdems2::entity::field_path::{FieldPath, read_field_paths};
    use pbdems2::io::BitReader;

    use super::*;

    fn check_fields(workload: &EntityWorkload, entities: &EntityContainer, changed: bool) {
        for index in 0..workload.entities {
            let entity = entities.get(index as i32).unwrap();
            assert_eq!(entity.fields.len(), MIXED_FIELDS);
            for field in 0..MIXED_FIELDS {
                let phase = usize::from(changed && index.is_multiple_of(8) && field < 6);
                let value = seed(index, field, phase);
                let mut path = FieldPath::default();
                if workload.nested {
                    path.data[0] = 0;
                    path.data[1] = field as u8;
                    path.last = 1;
                } else {
                    path.data[0] = field as u8;
                }
                let key = Some(path.pack());
                match field % 6 {
                    0 => assert_eq!(entity.get_bool(key), value.is_multiple_of(2)),
                    1 => assert_eq!(entity.get_u32(key), 300 + value),
                    2 => assert_eq!(entity.get_i64(key), -i64::from(value) - 1),
                    3 => assert_eq!(entity.get_f32(key), value as f32 * 0.5),
                    4 => assert_eq!(
                        entity.get_vector3(key),
                        [value as f32, value as f32 + 0.25, value as f32 + 0.5]
                    ),
                    _ => assert_eq!(
                        entity.get_bytes(key).unwrap(),
                        vec![b'a' + (value % 26) as u8; workload.string_bytes]
                    ),
                }
            }
        }
    }

    #[test]
    fn mixed_packets_preserve_values_and_sparse_updates() {
        for nested in [false, true] {
            for size in [0, 128] {
                let workload = EntityWorkload::new(32, nested, size);
                let mut entities = workload.populated();
                check_fields(&workload, &entities, false);
                let updates = workload.updates(8, 6);
                entities
                    .handle_packet_entities(
                        PacketEntities::new(4, &updates, 0),
                        &workload.classes,
                        &workload.serializers,
                        &StringTableContainer::new(),
                        &mut FieldDecodeContext::new(1.0 / 64.0),
                        &mut Vec::new(),
                    )
                    .unwrap();
                check_fields(&workload, &entities, true);
                assert_eq!(entities.updated_indices(), &[0, 8, 16, 24]);
                assert_eq!(entities.entity_changes().len(), 4);
                let mut skipped = workload.skipped();
                skipped
                    .handle_packet_entities_filtered(
                        PacketEntities::new(4, &updates, 0),
                        &workload.classes,
                        &workload.serializers,
                        &StringTableContainer::new(),
                        &mut FieldDecodeContext::new(1.0 / 64.0),
                        &HashSet::new(),
                        &mut Vec::new(),
                    )
                    .unwrap();
                assert!(skipped.is_empty());
                assert!(skipped.entity_changes().is_empty());
            }
        }
    }

    #[test]
    fn mixed_paths_and_values_match_the_wire_decoders() {
        for nested in [false, true] {
            let workload = EntityWorkload::new(1, nested, 128);
            for count in [6, MIXED_FIELDS] {
                let bytes = workload.paths(count);
                let mut paths = Vec::new();
                read_field_paths(&mut BitReader::new(&bytes), &mut paths).unwrap();
                assert_eq!(paths.len(), count);
                for (i, path) in paths.iter().enumerate() {
                    assert_eq!(path.last, usize::from(nested));
                    assert_eq!(path.get(path.last), i);
                    if nested {
                        assert_eq!(path.get(0), 0);
                    }
                }
            }
            let values = workload.values();
            let mut decoded = BitReader::new(&values);
            let mut skipped = BitReader::new(&values);
            let mut context = FieldDecodeContext::new(1.0 / 64.0);
            for field in &workload.serializers.get(MIXED_LEAF).unwrap().fields {
                field
                    .metadata
                    .decoder
                    .decode(&mut context, &mut decoded)
                    .unwrap();
                field
                    .metadata
                    .decoder
                    .skip(&mut context, &mut skipped)
                    .unwrap();
                assert_eq!(decoded.position(), skipped.position());
            }
            assert!(decoded.bits_remaining() < 8);
        }
    }
}
