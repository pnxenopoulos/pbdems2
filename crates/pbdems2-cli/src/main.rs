use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use pbdems2_cli::{inspect_path, write_commands, write_index, write_summary};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "pbdems2",
    version,
    about = "Inspect and validate Source 2 PBDEMS2 demo files"
)]
struct Cli {
    /// Emit machine-readable JSON instead of text.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate the whole file and show aggregate statistics.
    Summary { demo: PathBuf },
    /// List every outer command with offsets, ticks, and body sizes.
    Commands { demo: PathBuf },
    /// Show synchronization and full-packet seek points.
    Index { demo: PathBuf },
    /// Fully validate framing and decompress every command body.
    Validate { demo: PathBuf },
}

#[derive(Serialize)]
struct IndexView<'a> {
    playback_tick_range: &'a Option<pbdems2_cli::TickRange>,
    sync_points: &'a [pbdems2_cli::SyncPoint],
    full_packet_seek_points: &'a [pbdems2_cli::SeekPoint],
}

#[derive(Serialize)]
struct ValidationView {
    valid: bool,
    file_size: usize,
    commands: usize,
    file_info_offset: Option<usize>,
    spawn_groups_offset: Option<usize>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let path = match &cli.command {
        Command::Summary { demo }
        | Command::Commands { demo }
        | Command::Index { demo }
        | Command::Validate { demo } => demo,
    };
    let report = inspect_path(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    let stdout = io::stdout();
    let mut output = stdout.lock();

    match cli.command {
        Command::Summary { .. } => {
            if cli.json {
                write_json(&mut output, &report)
            } else {
                write_summary(&mut output, &report).map_err(|error| error.to_string())
            }
        }
        Command::Commands { .. } => {
            if cli.json {
                write_json(&mut output, &report.commands)
            } else {
                write_commands(&mut output, &report).map_err(|error| error.to_string())
            }
        }
        Command::Index { .. } => {
            if cli.json {
                write_json(
                    &mut output,
                    &IndexView {
                        playback_tick_range: &report.playback_tick_range,
                        sync_points: &report.sync_points,
                        full_packet_seek_points: &report.full_packet_seek_points,
                    },
                )
            } else {
                write_index(&mut output, &report).map_err(|error| error.to_string())
            }
        }
        Command::Validate { .. } => {
            if cli.json {
                write_json(
                    &mut output,
                    &ValidationView {
                        valid: true,
                        file_size: report.file_size,
                        commands: report.command_count(),
                        file_info_offset: report.header.file_info_offset,
                        spawn_groups_offset: report.header.spawn_groups_offset,
                    },
                )
            } else {
                writeln!(
                    output,
                    "valid PBDEMS2: {} bytes, {} commands, file-info offset {}, spawn-groups offset {}",
                    report.file_size,
                    report.command_count(),
                    format_offset(report.header.file_info_offset),
                    format_offset(report.header.spawn_groups_offset),
                )
                .map_err(|error| error.to_string())
            }
        }
    }
}

fn format_offset(offset: Option<usize>) -> String {
    offset.map_or_else(|| "none".to_owned(), |offset| offset.to_string())
}

fn write_json(output: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer_pretty(&mut *output, value).map_err(|error| error.to_string())?;
    writeln!(output).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_json_before_or_after_the_subcommand() {
        let before = Cli::try_parse_from(["pbdems2", "--json", "summary", "match.dem"])
            .expect("global option before command");
        let after = Cli::try_parse_from(["pbdems2", "summary", "match.dem", "--json"])
            .expect("global option after command");
        assert!(before.json);
        assert!(after.json);
    }

    #[test]
    fn requires_a_subcommand() {
        assert!(Cli::try_parse_from(["pbdems2"]).is_err());
    }

    #[test]
    fn serializes_validation_as_json() {
        let view = ValidationView {
            valid: true,
            file_size: 16,
            commands: 0,
            file_info_offset: None,
            spawn_groups_offset: None,
        };
        let mut output = Vec::new();
        write_json(&mut output, &view).expect("json");
        let value: serde_json::Value = serde_json::from_slice(&output).expect("valid json");
        assert_eq!(value["valid"], true);
        assert_eq!(value["file_size"], 16);
        assert_eq!(value["file_info_offset"], serde_json::Value::Null);
    }
}
