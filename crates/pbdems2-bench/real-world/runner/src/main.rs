use std::collections::HashSet;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: real-world-control awpy|boon all|players DEMO".into());
    }
    let path = Path::new(&args[3]);
    let started = Instant::now();
    let mut ticks = 0usize;
    match args[1].as_str() {
        "awpy" => {
            let parser = awpy::Parser::from_file(path)?;
            if args[2] == "players" {
                let filter = HashSet::from(["CCSPlayerController", "CCSPlayerPawn"]);
                parser.run_to_end_filtered(&filter, |_| ticks += 1)?;
            } else if args[2] == "all" {
                parser.run_to_end(|_| ticks += 1)?;
            } else {
                return Err("expected all or players".into());
            }
        }
        "boon" => {
            let parser = boon_parser::Parser::from_file(path)?;
            if args[2] == "players" {
                let filter = HashSet::from(["CCitadelPlayerController", "CCitadelPlayerPawn"]);
                parser.run_to_end_filtered(&filter, |_| ticks += 1)?;
            } else if args[2] == "all" {
                parser.run_to_end(|_| ticks += 1)?;
            } else {
                return Err("expected all or players".into());
            }
        }
        _ => return Err("expected awpy or boon".into()),
    }
    println!(
        "{}",
        serde_json::json!({
            "game": args[1], "mode": args[2], "ticks": black_box(ticks),
            "seconds": started.elapsed().as_secs_f64()
        })
    );
    Ok(())
}
