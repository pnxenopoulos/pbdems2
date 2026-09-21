//! Entity-update workloads spanning one or many network classes.
use crate::{BENCH_PROFILE, BitWriter, bool_serializer, packet_entity_updates, push_bool_fields};
use pbdems2::entity::{
    ClassEntry, ClassInfo, EntityContainer, FieldDecodeContext, FlattenedSerializer,
    FlattenedSerializerDefinition, PacketEntities, SerializerContainer, StringTableContainer,
};
use std::collections::HashSet;

/// Matching schemas and encoded packets for interleaved entity classes.
pub struct SerializerWorkload {
    /// Class-ID mapping.
    pub classes: ClassInfo,
    /// Definitions with identical fields but distinct class names.
    pub serializers: SerializerContainer,
    /// Encoded creates, interleaved by class ID.
    pub creates: Vec<u8>,
    /// Encoded updates for the same slots.
    pub updates: Vec<u8>,
    /// Number of occupied slots.
    pub count: usize,
}

impl SerializerWorkload {
    /// Construct a one-field or dense update workload across multiple classes.
    pub fn new(class_count: usize, count: usize, fields: usize) -> Self {
        assert!(class_count > 0 && class_count <= 128);
        let base = bool_serializer(fields);
        let mut symbols = base.symbols.clone();
        let mut definitions = Vec::new();
        let entries: Vec<_> = (0..class_count)
            .map(|index| {
                let name = format!("CRepresentativeNetworkClass{index:03}");
                let symbol = symbols.len() as i32;
                symbols.push(name.clone());
                definitions.push(FlattenedSerializerDefinition::new(
                    Some(symbol),
                    (0..fields as i32).collect(),
                ));
                ClassEntry::new(index as i32, name.clone(), name)
            })
            .collect();
        let classes = ClassInfo::try_from_entries(entries).unwrap();
        let serializers = SerializerContainer::parse(
            FlattenedSerializer::new(definitions, symbols, base.fields),
            BENCH_PROFILE,
        )
        .unwrap();
        let mut writer = BitWriter::default();
        for index in 0..count {
            writer.push_ubitvar(0);
            writer.push_bits(0b10, 2);
            writer.push_bits((index % class_count) as u64, classes.bits());
            writer.push_bits(0, 17);
            writer.push_uvarint32(0);
            push_bool_fields(&mut writer, fields);
        }
        Self {
            classes,
            serializers,
            creates: writer.finish(),
            updates: packet_entity_updates(count, fields),
            count,
        }
    }

    /// Decode the create packet, retaining all entities or filtering all of them.
    pub fn populated(&self, skipped: bool) -> EntityContainer {
        let mut entities = EntityContainer::new();
        let mut context = FieldDecodeContext::new(1.0 / 64.0);
        let mut paths = Vec::new();
        let tables = StringTableContainer::new();
        let message = PacketEntities::new(self.count as i32, &self.creates, 0);
        if skipped {
            entities
                .handle_packet_entities_filtered(
                    message,
                    &self.classes,
                    &self.serializers,
                    &tables,
                    &mut context,
                    &HashSet::new(),
                    &mut paths,
                )
                .unwrap();
        } else {
            entities
                .handle_packet_entities(
                    message,
                    &self.classes,
                    &self.serializers,
                    &tables,
                    &mut context,
                    &mut paths,
                )
                .unwrap();
        }
        entities.clear_tick_changes();
        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_class_packets_retain_or_skip_the_expected_slots() {
        for classes in [1, 16, 128] {
            let fixture = SerializerWorkload::new(classes, 256, 6);
            let kept = fixture.populated(false);
            assert_eq!(kept.iter().count(), 256);
            for (index, entity) in kept.iter() {
                assert_eq!(entity.class_id, index % classes as i32);
                assert_eq!(entity.fields.len(), 6);
            }
            assert_eq!(fixture.populated(true).iter().count(), 0);
        }
    }
}
