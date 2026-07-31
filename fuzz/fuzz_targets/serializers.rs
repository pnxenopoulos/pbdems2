#![no_main]

use libfuzzer_sys::fuzz_target;
use pbdems2::DecodeLimits;
use pbdems2::entity::{
    BareCharEncoding, DecodeProfile, FlattenedField, FlattenedSerializer,
    FlattenedSerializerDefinition, PreciseQAngleMode, SerializerContainer,
};

const PROFILE: DecodeProfile = DecodeProfile::new(
    BareCharEncoding::NullTerminatedString,
    PreciseQAngleMode::Centered,
);

fuzz_target!(|input: &[u8]| {
    let symbols: Vec<String> = input
        .chunks(8)
        .take(32)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    let fields: Vec<FlattenedField> = input
        .chunks(6)
        .take(32)
        .map(|chunk| {
            let at = |index: usize| chunk.get(index).copied().unwrap_or(0) as i8 as i32;
            FlattenedField::new(Some(at(0)), Some(at(1)))
                .with_bit_count(Some(at(2)))
                .with_range(Some(at(3) as f32 / 4.0), Some(at(4) as f32 / 4.0))
                .with_encoder_sym((at(5) & 1 != 0).then_some(at(5)))
        })
        .collect();
    let field_indices = input
        .iter()
        .take(32)
        .map(|byte| i32::from(*byte as i8))
        .collect();
    let message = FlattenedSerializer::new(
        vec![FlattenedSerializerDefinition::new(
            input.first().map(|byte| i32::from(*byte as i8)),
            field_indices,
        )],
        symbols,
        fields,
    );
    let limits = DecodeLimits::default()
        .with_serializer_limits(8, 64, 64)
        .with_max_fixed_array_length(64)
        .with_max_field_string_bytes(128);
    let _ = SerializerContainer::parse_with_limits(message, PROFILE, &limits);
});
