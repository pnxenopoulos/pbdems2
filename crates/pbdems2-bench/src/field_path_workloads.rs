//! Anonymized field-path lists sampled from real CS2 and Deadlock demos.
use pbdems2::entity::field_path::{FieldPath, read_field_paths};
use pbdems2::io::BitReader;

/// One encoded update's path list, without entity values.
pub struct PathRecord {
    /// Encoded bits, optional prefix alignment, and optional trailing padding.
    pub bytes: Vec<u8>,
    /// First path bit.
    pub offset: usize,
    /// Number of path-list bits, including finish.
    pub bits: usize,
    /// Expected number of decoded fields.
    pub fields: usize,
    /// Rolling digest of the reference decoder's packed paths.
    pub digest: u64,
}

/// Load two demos' deterministic 512-update reservoirs for a game.
pub fn records(game: &str, alignment: usize, trailing_bytes: usize) -> Vec<PathRecord> {
    assert!(alignment < 8);
    let sources = match game {
        "cs2" => [
            include_str!("../fixtures/field_paths/cs2-cache.txt"),
            include_str!("../fixtures/field_paths/cs2-dust2.txt"),
        ],
        "deadlock" => [
            include_str!("../fixtures/field_paths/deadlock-100655353.txt"),
            include_str!("../fixtures/field_paths/deadlock-103129247.txt"),
        ],
        _ => panic!("unknown fixture game"),
    };
    sources
        .into_iter()
        .flat_map(|source| source.lines())
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let mut parts = line.split_whitespace();
            let bits: usize = parts.next().unwrap().parse().unwrap();
            let fields = parts.next().unwrap().parse().unwrap();
            let digest = u64::from_str_radix(parts.next().unwrap(), 16).unwrap();
            let hex = parts.next().unwrap();
            let raw: Vec<_> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(raw.len(), bits.div_ceil(8));
            assert!(parts.next().is_none());
            let mut bytes = vec![0; (bits + alignment).div_ceil(8) + trailing_bytes];
            for bit in 0..bits {
                bytes[(bit + alignment) / 8] |=
                    ((raw[bit / 8] >> (bit % 8)) & 1) << ((bit + alignment) % 8);
            }
            PathRecord {
                bytes,
                offset: alignment,
                bits,
                fields,
                digest,
            }
        })
        .collect()
}

/// Check every decoded path and the exact position of the following field data.
pub fn validate(records: &[PathRecord]) {
    let mut paths = Vec::new();
    for record in records {
        let mut reader = BitReader::new(&record.bytes);
        reader.skip_bits(record.offset).unwrap();
        read_field_paths(&mut reader, &mut paths).expect("valid captured path list");
        assert_eq!(reader.position(), record.offset + record.bits);
        assert_eq!(paths.len(), record.fields);
        let digest = paths.iter().fold(0u64, |value, path: &FieldPath| {
            value.rotate_left(7) ^ path.pack()
        });
        assert_eq!(digest, record.digest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_path_lists_match_reference_at_every_alignment_and_at_short_tails() {
        for game in ["cs2", "deadlock"] {
            for offset in 0..8 {
                for trailing in [0, 16] {
                    let records = records(game, offset, trailing);
                    assert_eq!(records.len(), 1024);
                    validate(&records);
                }
            }
        }
    }
}
