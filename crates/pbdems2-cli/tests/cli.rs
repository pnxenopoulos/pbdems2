use std::io::Write;
use std::process::Command;

use pbdems2::demo::{HEADER_SIZE, MAGIC, command};
use tempfile::NamedTempFile;

fn demo_file() -> NamedTempFile {
    let mut bytes = Vec::from(MAGIC);
    bytes.resize(HEADER_SIZE, 0);
    bytes.extend_from_slice(&[command::SYNC_TICK as u8, 0, 0]);
    bytes.extend_from_slice(&[command::STOP as u8, 1, 0]);

    let mut file = NamedTempFile::new().expect("temporary demo");
    file.write_all(&bytes).expect("write demo");
    file.flush().expect("flush demo");
    file
}

#[test]
fn validates_a_file_and_emits_json() {
    let file = demo_file();
    let output = Command::new(env!("CARGO_BIN_EXE_pbdems2"))
        .args(["validate", "--json"])
        .arg(file.path())
        .output()
        .expect("run pbdems2");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["valid"], true);
    assert_eq!(value["commands"], 2);
    assert_eq!(value["file_info_offset"], serde_json::Value::Null);
    assert_eq!(value["spawn_groups_offset"], serde_json::Value::Null);
}

#[test]
fn lists_command_types_in_file_order() {
    let file = demo_file();
    let output = Command::new(env!("CARGO_BIN_EXE_pbdems2"))
        .arg("commands")
        .arg(file.path())
        .output()
        .expect("run pbdems2");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(stdout.contains("SyncTick"));
    assert!(stdout.contains("Stop"));
}

#[test]
fn rejects_a_file_with_an_invalid_header() {
    let mut file = NamedTempFile::new().expect("temporary file");
    file.write_all(b"not a demo file")
        .expect("write invalid file");
    file.flush().expect("flush invalid file");

    let output = Command::new(env!("CARGO_BIN_EXE_pbdems2"))
        .arg("validate")
        .arg(file.path())
        .output()
        .expect("run pbdems2");

    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("UTF-8 error")
            .contains("failed to inspect")
    );
}
