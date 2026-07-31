#![no_main]

use libfuzzer_sys::fuzz_target;
use pbdems2::DecodeLimits;
use pbdems2::entity::field_path::FieldPath;
use pbdems2::entity::{
    BareCharEncoding, ClassEntry, ClassInfo, DecodeProfile, EntityContainer, FieldDecodeContext,
    FlattenedSerializer, FlattenedSerializerDefinition, PacketEntities, PreciseQAngleMode,
    SerializerContainer, StringTableContainer,
};

const PROFILE: DecodeProfile = DecodeProfile::new(
    BareCharEncoding::NullTerminatedString,
    PreciseQAngleMode::Centered,
);

fuzz_target!(|input: &[u8]| {
    let limits = DecodeLimits::default()
        .with_class_limits(4, 4)
        .with_serializer_limits(4, 16, 16)
        .with_max_packet_entity_updates(64)
        .with_max_field_paths(128)
        .with_max_field_string_bytes(4 * 1024);
    let serializers = SerializerContainer::parse_with_limits(
        FlattenedSerializer::new(
            vec![FlattenedSerializerDefinition::new(Some(0), Vec::new())],
            vec!["CFuzz".to_owned()],
            Vec::new(),
        ),
        PROFILE,
        &limits,
    )
    .expect("static serializer fixture is valid");
    let classes =
        ClassInfo::try_from_entries_with_limits([ClassEntry::new(0, "CFuzz", "CFuzz")], &limits)
            .expect("static class fixture is valid");
    let mut entities = EntityContainer::new();
    let tables = StringTableContainer::new();
    let mut context = FieldDecodeContext::with_limits(1.0 / 64.0, limits);
    let mut paths = Vec::<FieldPath>::new();
    let updates = i32::from(input.first().copied().unwrap_or(0) % 65);
    let data = input.get(1..).unwrap_or_default();
    let message = PacketEntities::new(updates, data, u32::from(updates & 1 != 0));
    let _ = entities.handle_packet_entities(
        message,
        &classes,
        &serializers,
        &tables,
        &mut context,
        &mut paths,
    );
});
