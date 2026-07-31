#![no_main]

use libfuzzer_sys::fuzz_target;
use pbdems2::DecodeLimits;
use pbdems2::entity::field_path::{FieldPath, read_field_paths_with_limits};
use pbdems2::io::BitReader;

fuzz_target!(|input: &[u8]| {
    let limits = DecodeLimits::default().with_max_field_paths(256);
    let mut reader = BitReader::new(input);
    let mut paths = Vec::<FieldPath>::new();
    let _ = read_field_paths_with_limits(&mut reader, &mut paths, &limits);
    for path in paths {
        let _ = FieldPath::unpack(path.pack());
    }
});
