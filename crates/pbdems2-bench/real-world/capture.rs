//! Untimed diagnostic module, injected into an isolated source copy only.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use super::FieldPath;
use crate::io::BitReader;

struct Capture {
    operations: [u64; 40],
    depths: [u64; 7],
    sizes: BTreeMap<usize, u64>,
    updates: u64,
    random: u64,
    records: Vec<String>,
}

thread_local! {
    static CAPTURE: RefCell<Capture> = RefCell::new(Capture {
        operations: [0; 40], depths: [0; 7], sizes: BTreeMap::new(),
        updates: 0, random: 0x5eed, records: Vec::new(),
    });
}

/// Record an operation after it has successfully executed.
pub fn operation(index: usize, path: &FieldPath) {
    CAPTURE.with_borrow_mut(|stats| {
        stats.operations[index] += 1;
        if !path.finished {
            stats.depths[path.last] += 1;
        }
    });
}

/// Reservoir-sample whole path lists, excluding entity values and framing.
pub fn update(reader: &BitReader, start: usize, paths: &[FieldPath]) {
    CAPTURE.with_borrow_mut(|stats| {
        stats.updates += 1;
        *stats.sizes.entry(paths.len()).or_default() += 1;
        stats.random = stats.random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let slot = if stats.updates <= 512 { stats.updates - 1 } else { stats.random % stats.updates };
        if slot >= 512 {
            return;
        }
        let bits = reader.position() - start;
        let mut encoded = vec![0u8; bits.div_ceil(8)];
        for bit in 0..bits {
            let source = start + bit;
            encoded[bit / 8] |= ((reader.data()[source / 8] >> (source % 8)) & 1) << (bit % 8);
        }
        let digest = paths.iter().fold(0u64, |value, path| value.rotate_left(7) ^ path.pack());
        let hex: String = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
        let record = format!("{bits} {} {digest:016x} {hex}", paths.len());
        if slot as usize == stats.records.len() {
            stats.records.push(record);
        } else {
            stats.records[slot as usize] = record;
        }
    });
}

/// Save metadata and path-only fixtures after parsing has completed.
pub fn save(prefix: &Path) -> std::io::Result<()> {
    CAPTURE.with_borrow(|stats| {
        let metadata = serde_json::json!({
            "updates": stats.updates, "operations": stats.operations.as_slice(),
            "depths": stats.depths, "update_sizes": stats.sizes,
            "reservoir_seed": 0x5eed_u64, "samples": stats.records.len(),
        });
        std::fs::write(prefix.with_extension("json"), serde_json::to_vec_pretty(&metadata)?)?;
        let mut output = std::io::BufWriter::new(std::fs::File::create(prefix.with_extension("txt"))?);
        writeln!(output, "# path-only reservoir: bit_length path_count packed_path_digest hex_bytes")?;
        for record in &stats.records { writeln!(output, "{record}")?; }
        output.flush()
    })
}
