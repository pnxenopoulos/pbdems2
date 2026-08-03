//! Inspection and presentation support for the private `pbdems2` CLI.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;

use pbdems2::demo::{Demo, HEADER_SIZE, command};
use pbdems2::{MappedDemo, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TickRange {
    pub first: i32,
    pub last: i32,
    pub distinct: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SyncPoint {
    pub tick: i32,
    pub offset: usize,
    pub resume_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SeekPoint {
    pub tick: i32,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HeaderInfo {
    pub file_info_offset: Option<usize>,
    pub spawn_groups_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandInfo {
    pub index: usize,
    pub command: i32,
    pub command_type: &'static str,
    pub tick: i32,
    pub offset: usize,
    pub end_offset: usize,
    pub header_size: usize,
    pub encoded_size: usize,
    pub decoded_size: usize,
    pub compressed: bool,
    pub compression_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandCount {
    pub command: i32,
    pub command_type: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CompressionStats {
    pub total_encoded_bytes: u64,
    pub total_decoded_bytes: u64,
    pub compressed_commands: usize,
    pub compressed_encoded_bytes: u64,
    pub compressed_decoded_bytes: u64,
    pub compressed_bytes_saved: i64,
    pub compressed_ratio: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Inspection {
    pub file_size: usize,
    pub file_header_size: usize,
    pub header: HeaderInfo,
    pub command_header_bytes: u64,
    pub commands: Vec<CommandInfo>,
    pub command_counts: Vec<CommandCount>,
    pub tick_range: Option<TickRange>,
    pub playback_tick_range: Option<TickRange>,
    pub sync_points: Vec<SyncPoint>,
    pub full_packet_seek_points: Vec<SeekPoint>,
    pub compression: CompressionStats,
    pub first_stop_command: Option<usize>,
    pub commands_after_stop: usize,
    pub unknown_commands: usize,
}

impl Inspection {
    #[must_use]
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

/// Memory-map and fully validate one demo, including every compressed body.
///
/// The input must not be modified or truncated while inspection is running,
/// as required by every portable file-backed memory-map implementation.
pub fn inspect_path(path: &Path) -> Result<Inspection> {
    // SAFETY: The mapping is read-only, stays owned throughout inspection, and
    // callers are told that the input must remain stable while this runs.
    let mapped = unsafe { MappedDemo::open(path)? };
    inspect_demo(mapped.demo()?)
}

/// Fully validate and inspect a borrowed demo.
pub fn inspect_demo(demo: Demo<'_>) -> Result<Inspection> {
    let header = demo.header();
    let index = demo.index()?;
    let mut decoded = Vec::new();
    let mut commands = Vec::new();
    let mut counts = BTreeMap::<i32, usize>::new();
    let mut ticks = BTreeSet::new();
    let mut sync_points = Vec::new();
    let mut total_encoded_bytes = 0_u64;
    let mut total_decoded_bytes = 0_u64;
    let mut command_header_bytes = 0_u64;
    let mut compressed_commands = 0_usize;
    let mut compressed_encoded_bytes = 0_u64;
    let mut compressed_decoded_bytes = 0_u64;
    let mut first_stop_command = None;
    let mut commands_after_stop = 0_usize;
    let mut unknown_commands = 0_usize;

    for frame in demo.commands() {
        let frame = frame?;
        frame.decode_body(&mut decoded)?;
        let header = frame.header();
        let encoded_size = frame.encoded_body().len();
        let decoded_size = decoded.len();
        let frame_header_size = frame.end_offset() - frame.offset() - encoded_size;

        total_encoded_bytes += usize_to_u64(encoded_size);
        total_decoded_bytes += usize_to_u64(decoded_size);
        command_header_bytes += usize_to_u64(frame_header_size);
        *counts.entry(header.cmd).or_default() += 1;

        if header.tick >= 0 {
            ticks.insert(header.tick);
        }
        if header.cmd == command::SYNC_TICK {
            sync_points.push(SyncPoint {
                tick: header.tick,
                offset: frame.offset(),
                resume_offset: frame.end_offset(),
            });
        }
        if header.cmd == command::STOP && first_stop_command.is_none() {
            first_stop_command = Some(frame.index());
        } else if first_stop_command.is_some() {
            commands_after_stop += 1;
        }

        let name = command_name(header.cmd);
        if name == "Unknown" {
            unknown_commands += 1;
        }
        let compression_ratio = if header.compressed {
            ratio(encoded_size, decoded_size)
        } else {
            None
        };
        if header.compressed {
            compressed_commands += 1;
            compressed_encoded_bytes += usize_to_u64(encoded_size);
            compressed_decoded_bytes += usize_to_u64(decoded_size);
        }

        commands.push(CommandInfo {
            index: frame.index(),
            command: header.cmd,
            command_type: name,
            tick: header.tick,
            offset: frame.offset(),
            end_offset: frame.end_offset(),
            header_size: frame_header_size,
            encoded_size,
            decoded_size,
            compressed: header.compressed,
            compression_ratio,
        });
    }

    let playback_ticks = index
        .distinct_ticks()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let command_counts = counts
        .into_iter()
        .map(|(command, count)| CommandCount {
            command,
            command_type: command_name(command),
            count,
        })
        .collect();
    let full_packet_seek_points = index
        .full_packets()
        .iter()
        .map(|point| SeekPoint {
            tick: point.tick(),
            offset: point.offset(),
        })
        .collect();

    Ok(Inspection {
        file_size: demo.data().len(),
        file_header_size: HEADER_SIZE,
        header: HeaderInfo {
            file_info_offset: header.file_info_offset(),
            spawn_groups_offset: header.spawn_groups_offset(),
        },
        command_header_bytes,
        commands,
        command_counts,
        tick_range: tick_range(&ticks),
        playback_tick_range: tick_range(&playback_ticks),
        sync_points,
        full_packet_seek_points,
        compression: CompressionStats {
            total_encoded_bytes,
            total_decoded_bytes,
            compressed_commands,
            compressed_encoded_bytes,
            compressed_decoded_bytes,
            compressed_bytes_saved: signed_difference(
                compressed_decoded_bytes,
                compressed_encoded_bytes,
            ),
            compressed_ratio: ratio_u64(compressed_encoded_bytes, compressed_decoded_bytes),
        },
        first_stop_command,
        commands_after_stop,
        unknown_commands,
    })
}

#[must_use]
pub const fn command_name(value: i32) -> &'static str {
    match value {
        command::STOP => "Stop",
        command::FILE_HEADER => "FileHeader",
        command::FILE_INFO => "FileInfo",
        command::SYNC_TICK => "SyncTick",
        command::SEND_TABLES => "SendTables",
        command::CLASS_INFO => "ClassInfo",
        command::STRING_TABLES => "StringTables",
        command::PACKET => "Packet",
        command::SIGNON_PACKET => "SignonPacket",
        command::CONSOLE_CMD => "ConsoleCmd",
        command::CUSTOM_DATA => "CustomData",
        command::CUSTOM_DATA_CALLBACKS => "CustomDataCallbacks",
        command::USER_CMD => "UserCmd",
        command::FULL_PACKET => "FullPacket",
        command::SAVE_GAME => "SaveGame",
        command::SPAWN_GROUPS => "SpawnGroups",
        command::ANIMATION_DATA => "AnimationData",
        command::ANIMATION_HEADER => "AnimationHeader",
        command::RECOVERY => "Recovery",
        _ => "Unknown",
    }
}

pub fn write_summary(mut output: impl Write, report: &Inspection) -> io::Result<()> {
    writeln!(output, "status: valid PBDEMS2")?;
    writeln!(output, "file size: {} bytes", report.file_size)?;
    write_header_offset(
        &mut output,
        "file-info offset",
        report.header.file_info_offset,
    )?;
    write_header_offset(
        &mut output,
        "spawn-groups offset",
        report.header.spawn_groups_offset,
    )?;
    writeln!(output, "commands: {}", report.command_count())?;
    writeln!(
        output,
        "command headers: {} bytes",
        report.command_header_bytes
    )?;
    write_tick_range(&mut output, "ticks", report.tick_range)?;
    write_tick_range(&mut output, "playback ticks", report.playback_tick_range)?;
    writeln!(output, "sync points: {}", report.sync_points.len())?;
    writeln!(
        output,
        "full-packet seek points: {}",
        report.full_packet_seek_points.len()
    )?;
    match report.first_stop_command {
        Some(index) => writeln!(
            output,
            "first stop: command {index} ({} command(s) follow it)",
            report.commands_after_stop
        )?,
        None => writeln!(output, "first stop: not present")?,
    }
    writeln!(output, "unknown commands: {}", report.unknown_commands)?;
    let compression = report.compression;
    writeln!(
        output,
        "bodies: {} encoded bytes, {} decoded bytes",
        compression.total_encoded_bytes, compression.total_decoded_bytes
    )?;
    if let Some(ratio) = compression.compressed_ratio {
        writeln!(
            output,
            "compression: {} command(s), {} -> {} bytes, {:.2}% of decoded size, {:+} bytes saved",
            compression.compressed_commands,
            compression.compressed_encoded_bytes,
            compression.compressed_decoded_bytes,
            ratio * 100.0,
            compression.compressed_bytes_saved
        )?;
    } else {
        writeln!(output, "compression: no non-empty compressed bodies")?;
    }
    writeln!(output, "command counts:")?;
    for count in &report.command_counts {
        writeln!(
            output,
            "  {:>3} {:<20} {}",
            count.command, count.command_type, count.count
        )?;
    }
    Ok(())
}

pub fn write_commands(mut output: impl Write, report: &Inspection) -> io::Result<()> {
    writeln!(
        output,
        "{:>6} {:<20} {:>4} {:>10} {:>12} {:>8} {:>8} {:>5}",
        "index", "type", "id", "tick", "offset", "encoded", "decoded", "comp"
    )?;
    for command in &report.commands {
        writeln!(
            output,
            "{:>6} {:<20} {:>4} {:>10} {:>12} {:>8} {:>8} {:>5}",
            command.index,
            command.command_type,
            command.command,
            command.tick,
            command.offset,
            command.encoded_size,
            command.decoded_size,
            if command.compressed { "yes" } else { "no" }
        )?;
    }
    Ok(())
}

pub fn write_index(mut output: impl Write, report: &Inspection) -> io::Result<()> {
    write_tick_range(&mut output, "playback ticks", report.playback_tick_range)?;
    writeln!(output, "sync-tick seek points:")?;
    for point in &report.sync_points {
        writeln!(
            output,
            "  tick {:>10}  command offset {:>12}  resume offset {:>12}",
            point.tick, point.offset, point.resume_offset
        )?;
    }
    writeln!(output, "full-packet seek points:")?;
    for point in &report.full_packet_seek_points {
        writeln!(
            output,
            "  tick {:>10}  command offset {:>12}",
            point.tick, point.offset
        )?;
    }
    Ok(())
}

fn tick_range(ticks: &BTreeSet<i32>) -> Option<TickRange> {
    Some(TickRange {
        first: *ticks.first()?,
        last: *ticks.last()?,
        distinct: ticks.len(),
    })
}

fn write_tick_range(
    output: &mut impl Write,
    label: &str,
    range: Option<TickRange>,
) -> io::Result<()> {
    match range {
        Some(range) => writeln!(
            output,
            "{label}: {}..={} ({} distinct)",
            range.first, range.last, range.distinct
        ),
        None => writeln!(output, "{label}: none"),
    }
}

fn write_header_offset(
    output: &mut impl Write,
    label: &str,
    offset: Option<usize>,
) -> io::Result<()> {
    match offset {
        Some(offset) => writeln!(output, "{label}: {offset}"),
        None => writeln!(output, "{label}: none"),
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn ratio(encoded: usize, decoded: usize) -> Option<f64> {
    ratio_u64(usize_to_u64(encoded), usize_to_u64(decoded))
}

fn ratio_u64(encoded: u64, decoded: u64) -> Option<f64> {
    (decoded != 0).then_some(encoded as f64 / decoded as f64)
}

fn signed_difference(left: u64, right: u64) -> i64 {
    let difference = i128::from(left) - i128::from(right);
    i64::try_from(difference).unwrap_or(if difference.is_negative() {
        i64::MIN
    } else {
        i64::MAX
    })
}

#[cfg(test)]
mod tests {
    use pbdems2::demo::{COMPRESSED_FLAG, MAGIC};
    use snap::raw::Encoder;

    use super::*;

    fn push_varint(mut value: u32, output: &mut Vec<u8>) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            output.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    fn push_command(output: &mut Vec<u8>, id: i32, tick: i32, body: &[u8], compressed: bool) {
        let raw = u32::try_from(id).expect("non-negative command")
            | if compressed { COMPRESSED_FLAG } else { 0 };
        let encoded = if compressed {
            Encoder::new().compress_vec(body).expect("compress fixture")
        } else {
            body.to_vec()
        };
        push_varint(raw, output);
        push_varint(tick as u32, output);
        push_varint(encoded.len() as u32, output);
        output.extend_from_slice(&encoded);
    }

    fn fixture() -> Vec<u8> {
        let mut data = Vec::from(MAGIC);
        data.resize(HEADER_SIZE, 0);
        push_command(&mut data, command::FILE_HEADER, -1, b"metadata", false);
        push_command(&mut data, command::SYNC_TICK, 0, b"", false);
        push_command(&mut data, command::PACKET, 10, &[b'a'; 128], true);
        push_command(&mut data, command::FULL_PACKET, 20, b"keyframe", false);
        let spawn_groups_offset = data.len();
        push_command(&mut data, command::SPAWN_GROUPS, 20, b"spawn", false);
        push_command(&mut data, command::STOP, 20, b"", false);
        let file_info_offset = data.len();
        push_command(&mut data, command::FILE_INFO, 20, b"info", false);
        push_command(&mut data, 63, 21, b"trailing", false);
        data[8..12].copy_from_slice(&(file_info_offset as u32).to_le_bytes());
        data[12..16].copy_from_slice(&(spawn_groups_offset as u32).to_le_bytes());
        data
    }

    #[test]
    fn inspects_structure_sizes_ticks_and_seek_points() {
        let data = fixture();
        let report = inspect_demo(Demo::new(&data).expect("valid demo")).expect("valid bodies");

        assert_eq!(report.command_count(), 8);
        assert_eq!(
            report.header.file_info_offset,
            Some(report.commands[6].offset)
        );
        assert_eq!(
            report.header.spawn_groups_offset,
            Some(report.commands[4].offset)
        );
        assert_eq!(
            report.tick_range,
            Some(TickRange {
                first: 0,
                last: 21,
                distinct: 4,
            })
        );
        assert_eq!(
            report.playback_tick_range,
            Some(TickRange {
                first: 10,
                last: 20,
                distinct: 2,
            })
        );
        assert_eq!(report.sync_points.len(), 1);
        assert_eq!(report.full_packet_seek_points.len(), 1);
        assert_eq!(report.first_stop_command, Some(5));
        assert_eq!(report.commands_after_stop, 2);
        assert_eq!(report.unknown_commands, 1);
        assert_eq!(report.commands[2].decoded_size, 128);
        assert!(report.commands[2].compressed);
        assert!(report.compression.compressed_bytes_saved > 0);
    }

    #[test]
    fn renders_each_human_readable_view() {
        let data = fixture();
        let report = inspect_demo(Demo::new(&data).expect("valid demo")).expect("valid bodies");

        let mut summary = Vec::new();
        write_summary(&mut summary, &report).expect("summary");
        let summary = String::from_utf8(summary).expect("utf-8");
        assert!(summary.contains("status: valid PBDEMS2"));
        assert!(summary.contains("file-info offset:"));
        assert!(summary.contains("spawn-groups offset:"));
        assert!(summary.contains("FullPacket"));

        let mut commands = Vec::new();
        write_commands(&mut commands, &report).expect("commands");
        let commands = String::from_utf8(commands).expect("utf-8");
        assert!(commands.contains("encoded"));
        assert!(commands.contains("Packet"));

        let mut index = Vec::new();
        write_index(&mut index, &report).expect("index");
        let index = String::from_utf8(index).expect("utf-8");
        assert!(index.contains("resume offset"));
        assert!(index.contains("full-packet seek points"));
    }

    #[test]
    fn reports_empty_tick_and_compression_ranges() {
        let mut data = Vec::from(MAGIC);
        data.resize(HEADER_SIZE, 0);
        push_command(&mut data, command::STOP, -1, b"", false);
        let report = inspect_demo(Demo::new(&data).expect("valid demo")).expect("valid body");

        assert_eq!(report.tick_range, None);
        assert_eq!(report.playback_tick_range, None);
        assert_eq!(
            report.header,
            HeaderInfo {
                file_info_offset: None,
                spawn_groups_offset: None,
            }
        );
        assert_eq!(report.compression.compressed_ratio, None);
        let mut output = Vec::new();
        write_summary(&mut output, &report).expect("summary");
        assert!(
            String::from_utf8(output)
                .expect("utf-8")
                .contains("ticks: none")
        );
    }

    #[test]
    fn names_every_standard_command_and_unknown_values() {
        let names = (command::STOP..=command::RECOVERY)
            .map(command_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 19);
        assert_eq!(command_name(99), "Unknown");
    }
}
