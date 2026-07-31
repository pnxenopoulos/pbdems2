use crate::error::Error;
use crate::io::BitReader;
use crate::limits::DecodeLimits;
use crate::test_utils::BitWriter;

use super::{FIELDOP_DESCRIPTORS, FIELDOP_HIERARCHY, Node, read_field_paths_with_limits};

pub(crate) const PLUS_ONE: usize = 0;
pub(crate) const FINISH: usize = FIELDOP_DESCRIPTORS.len() - 1;

fn find_code(node: &Node, target: usize, path: &mut Vec<bool>) -> bool {
    match node {
        Node::Leaf { num, .. } => *num == target,
        Node::Branch { left, right, .. } => {
            path.push(false);
            if find_code(left, target, path) {
                return true;
            }
            path.pop();

            path.push(true);
            if find_code(right, target, path) {
                return true;
            }
            path.pop();
            false
        }
    }
}

pub(crate) fn emit_op(writer: &mut BitWriter, index: usize) {
    let mut code = Vec::new();
    assert!(find_code(&FIELDOP_HIERARCHY, index, &mut code));
    for bit in code {
        writer.push_bool(bit);
    }
}

pub(crate) fn emit_single_bool_update(writer: &mut BitWriter, value: bool) {
    emit_op(writer, PLUS_ONE);
    emit_op(writer, FINISH);
    writer.push_bool(value);
}

fn emit_operands(writer: &mut BitWriter, index: usize) {
    match index {
        0..=3 | 5 | 7 | 27 | 29 | 37 | 39 => {}
        4 | 6 | 8 | 9 | 28 | 30 | 33 => writer.push_ubitvarfp(1),
        10 => {
            writer.push_ubitvarfp(1);
            writer.push_ubitvarfp(1);
        }
        11 => {
            writer.push_bits(1, 3);
            writer.push_bits(2, 3);
        }
        12 => {
            writer.push_bits(1, 4);
            writer.push_bits(2, 4);
        }
        13 | 17 => {
            writer.push_ubitvarfp(1);
            writer.push_ubitvarfp(2);
        }
        14 | 18 => {
            writer.push_bits(1, 5);
            writer.push_bits(2, 5);
        }
        15 | 19 => {
            writer.push_ubitvarfp(1);
            writer.push_ubitvarfp(2);
            writer.push_ubitvarfp(3);
        }
        16 | 20 => {
            writer.push_bits(1, 5);
            writer.push_bits(2, 5);
            writer.push_bits(3, 5);
        }
        21 => {
            writer.push_ubitvar(0);
            writer.push_ubitvarfp(1);
            writer.push_ubitvarfp(2);
        }
        23 => {
            writer.push_ubitvar(0);
            writer.push_ubitvarfp(1);
            writer.push_ubitvarfp(2);
            writer.push_ubitvarfp(3);
        }
        22 => {
            writer.push_ubitvar(0);
            writer.push_bits(1, 5);
            writer.push_bits(2, 5);
        }
        24 => {
            writer.push_ubitvar(0);
            writer.push_bits(1, 5);
            writer.push_bits(2, 5);
            writer.push_bits(3, 5);
        }
        25 => {
            writer.push_ubitvar(2);
            writer.push_ubitvar(1);
            writer.push_ubitvarfp(2);
            writer.push_ubitvarfp(3);
        }
        26 => {
            writer.push_bool(false);
            writer.push_ubitvar(2);
            writer.push_ubitvarfp(2);
            writer.push_ubitvarfp(3);
        }
        31 => writer.push_bits(1, 3),
        32 => writer.push_bits(1, 6),
        34 => {
            writer.push_ubitvarfp(1);
            writer.push_varint32(2);
        }
        35 => {
            writer.push_ubitvarfp(1);
            writer.push_bool(true);
            writer.push_varint32(2);
            writer.push_bool(false);
            writer.push_bool(true);
            writer.push_varint32(-1);
        }
        36 => {
            writer.push_bool(true);
            writer.push_varint32(2);
        }
        38 => {
            writer.push_bool(true);
            writer.push_bits(8, 4);
        }
        _ => panic!("missing operands for field operation {index}"),
    }
}

fn emit_initialized_path(writer: &mut BitWriter) {
    emit_op(writer, PLUS_ONE);
}

fn emit_deep_path(writer: &mut BitWriter) {
    emit_initialized_path(writer);
    emit_op(writer, 15);
    emit_operands(writer, 15);
}

#[test]
fn every_field_operation_decodes_through_the_huffman_tree() {
    for index in 0..FIELDOP_DESCRIPTORS.len() {
        let mut writer = BitWriter::default();
        if index != FINISH {
            if matches!(index, 27..=35 | 37) {
                emit_deep_path(&mut writer);
            } else if index != PLUS_ONE {
                emit_initialized_path(&mut writer);
            }
            emit_op(&mut writer, index);
            emit_operands(&mut writer, index);
            emit_op(&mut writer, FINISH);
        } else {
            emit_op(&mut writer, FINISH);
        }

        let bytes = writer.finish();
        let mut paths = Vec::new();
        read_field_paths_with_limits(
            &mut BitReader::new(&bytes),
            &mut paths,
            &DecodeLimits::default(),
        )
        .unwrap_or_else(|error| {
            panic!("field operation {index} failed: {error}; bytes={bytes:02x?}")
        });

        if index == FINISH {
            assert!(paths.is_empty());
        } else {
            let decoded = paths.last().expect("target operation emits a path");
            assert!(decoded.last < decoded.data.len(), "operation {index}");
            assert!(!decoded.finished);
        }
    }
}

#[test]
fn field_path_limit_stops_before_appending_the_excess_path() {
    let mut writer = BitWriter::default();
    emit_op(&mut writer, PLUS_ONE);
    emit_op(&mut writer, PLUS_ONE);
    emit_op(&mut writer, FINISH);
    let bytes = writer.finish();
    let limits = DecodeLimits::default().with_max_field_paths(1);
    let mut paths = Vec::new();

    let error = read_field_paths_with_limits(&mut BitReader::new(&bytes), &mut paths, &limits)
        .expect_err("second path exceeds the limit");

    assert!(matches!(
        error,
        Error::LimitExceeded {
            resource: "entity field paths",
            limit: 1,
            actual: 2
        }
    ));
    assert_eq!(paths.len(), 1);
}

#[test]
fn empty_field_path_stream_is_reported_as_truncated() {
    let error = read_field_paths_with_limits(
        &mut BitReader::new(&[]),
        &mut Vec::new(),
        &DecodeLimits::default(),
    )
    .expect_err("empty stream cannot contain a Huffman operation");

    assert!(matches!(error, Error::Overflow { available: 0, .. }));
}

#[test]
fn representative_operations_produce_expected_paths() {
    let mut writer = BitWriter::default();
    emit_initialized_path(&mut writer);
    emit_op(&mut writer, 11);
    emit_operands(&mut writer, 11);
    emit_op(&mut writer, 27);
    emit_operands(&mut writer, 27);
    emit_op(&mut writer, FINISH);
    let bytes = writer.finish();
    let mut paths = Vec::new();

    read_field_paths_with_limits(
        &mut BitReader::new(&bytes),
        &mut paths,
        &DecodeLimits::default(),
    )
    .expect("valid field path stream");

    assert_eq!(paths.len(), 3);
    assert_eq!(&paths[0].data[..=paths[0].last], &[0]);
    assert_eq!(&paths[1].data[..=paths[1].last], &[3, 3]);
    assert_eq!(&paths[2].data[..=paths[2].last], &[4]);
}
