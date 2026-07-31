use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pbdems2::entity::{
    FieldDecodeContext, FlattenedField, FlattenedSerializer, FlattenedSerializerDefinition,
    SerializerContainer,
};
use pbdems2::io::BitReader;
use pbdems2_bench::BENCH_PROFILE;

struct DecodeCase {
    name: &'static str,
    serializers: SerializerContainer,
    data: Vec<u8>,
}

fn serializer_for(
    var_type: &str,
    bit_count: Option<i32>,
    low_value: Option<f32>,
    high_value: Option<f32>,
    encoder: Option<&str>,
) -> SerializerContainer {
    let mut symbols = vec![
        "CBenchmarkEntity".to_owned(),
        var_type.to_owned(),
        "m_value".to_owned(),
    ];
    let var_encoder_sym = encoder.map(|encoder| {
        let index = symbols.len() as i32;
        symbols.push(encoder.to_owned());
        index
    });

    SerializerContainer::parse(
        FlattenedSerializer::new(
            vec![FlattenedSerializerDefinition::new(Some(0), vec![0])],
            symbols,
            vec![
                FlattenedField::new(Some(1), Some(2))
                    .with_bit_count(bit_count)
                    .with_range(low_value, high_value)
                    .with_encode_flags(Some(0))
                    .with_encoder_sym(var_encoder_sym),
            ],
        ),
        BENCH_PROFILE,
    )
    .expect("decoder fixture must be valid")
}

fn decode_cases() -> Vec<DecodeCase> {
    let mut vector = Vec::new();
    vector.extend_from_slice(&1.0_f32.to_le_bytes());
    vector.extend_from_slice(&2.0_f32.to_le_bytes());
    vector.extend_from_slice(&3.0_f32.to_le_bytes());

    vec![
        DecodeCase {
            name: "bool",
            serializers: serializer_for("bool", None, None, None, None),
            data: vec![1],
        },
        DecodeCase {
            name: "signed_varint",
            serializers: serializer_for("int32", None, None, None, None),
            data: vec![2],
        },
        DecodeCase {
            name: "unsigned_varint",
            serializers: serializer_for("uint32", None, None, None, None),
            data: vec![0xac, 0x02],
        },
        DecodeCase {
            name: "float_no_scale",
            serializers: serializer_for("float32", None, None, None, None),
            data: 1.5_f32.to_le_bytes().to_vec(),
        },
        DecodeCase {
            name: "quantized_float_12_bit",
            serializers: serializer_for("float32", Some(12), Some(0.0), Some(100.0), None),
            data: vec![0x5a, 0x05],
        },
        DecodeCase {
            name: "string",
            serializers: serializer_for("CUtlString", None, None, None, None),
            data: b"benchmark-value\0".to_vec(),
        },
        DecodeCase {
            name: "vector3",
            serializers: serializer_for("Vector", None, None, None, None),
            data: vector,
        },
        DecodeCase {
            name: "qangle_12_bit",
            serializers: serializer_for("QAngle", Some(12), None, None, None),
            data: vec![0x5a; 5],
        },
        DecodeCase {
            name: "qangle_precise",
            serializers: serializer_for("QAngle", Some(20), None, None, Some("qangle_precise")),
            data: vec![0xff; 9],
        },
    ]
}

fn field_decode_benchmarks(criterion: &mut Criterion) {
    let cases = decode_cases();
    let mut group = criterion.benchmark_group("field_decode");
    group.throughput(Throughput::Elements(1));

    for case in &cases {
        let decoder = &case
            .serializers
            .get("CBenchmarkEntity")
            .expect("serializer exists")
            .fields[0]
            .metadata
            .decoder;
        group.bench_with_input(
            BenchmarkId::from_parameter(case.name),
            case,
            |bencher, case| {
                let mut context = FieldDecodeContext::new(1.0 / 64.0);
                bencher.iter(|| {
                    let mut reader = BitReader::new(black_box(&case.data));
                    black_box(
                        decoder
                            .decode(&mut context, &mut reader)
                            .expect("decoder fixture is valid"),
                    )
                });
            },
        );
    }

    group.finish();
}

fn field_skip_benchmarks(criterion: &mut Criterion) {
    let cases = decode_cases();
    let mut group = criterion.benchmark_group("field_skip");
    group.throughput(Throughput::Elements(1));

    for case in &cases {
        let decoder = &case
            .serializers
            .get("CBenchmarkEntity")
            .expect("serializer exists")
            .fields[0]
            .metadata
            .decoder;
        group.bench_with_input(
            BenchmarkId::from_parameter(case.name),
            case,
            |bencher, case| {
                let mut context = FieldDecodeContext::new(1.0 / 64.0);
                bencher.iter(|| {
                    let mut reader = BitReader::new(black_box(&case.data));
                    decoder
                        .skip(&mut context, &mut reader)
                        .expect("decoder fixture is valid");
                    black_box(reader.position())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, field_decode_benchmarks, field_skip_benchmarks);
criterion_main!(benches);
