#![no_main]

use libfuzzer_sys::fuzz_target;
use pbdems2::DecodeLimits;
use pbdems2::demo::{Demo, MAGIC};

fuzz_target!(|input: &[u8]| {
    let limits = DecodeLimits::default()
        .with_max_command_body_bytes(4 * 1024)
        .with_max_decompressed_command_bytes(16 * 1024);
    let mut data = Vec::with_capacity(16 + input.len());
    data.extend_from_slice(&MAGIC);
    data.extend_from_slice(&[0; 8]);
    data.extend_from_slice(input);

    let Ok(demo) = Demo::with_limits(&data, limits) else {
        return;
    };
    let mut decoded = Vec::new();
    for frame in demo.commands().take(256) {
        let Ok(frame) = frame else {
            break;
        };
        let _ = frame.decode_body(&mut decoded);
    }
    let _ = demo.index();
});
