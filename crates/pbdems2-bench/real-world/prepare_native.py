"""Prepare a disposable consumer workspace with its own lockfile and features."""
import argparse
import json
from pathlib import Path
import shlex
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[3]
HARNESS = Path(__file__).resolve().parent


def replace_once(path, old, new):
    text = path.read_text()
    if text.count(old) != 1:
        raise ValueError(f"instrumentation anchor is missing or ambiguous: {path}: {old}")
    path.write_text(text.replace(old, new))


def scope(path, anchor, stage, indent="        "):
    replace_once(path, anchor, anchor +
                 f"\n{indent}let _allocation_scope = pbdems2_allocation_probe::enter("
                 f"pbdems2_allocation_probe::Stage::{stage});")


def instrument(output, game):
    probe = json.dumps(str(HARNESS / "allocation-probe"), ensure_ascii=False)
    for name in ([game, f"{game}-python"] if game == "awpy" else [f"{game}-python"]):
        replace_once(output / f"crates/{name}/Cargo.toml", "[dependencies]\n",
                     f"[dependencies]\npbdems2-allocation-probe = {{ path = {probe} }}\n")
    lib = output / f"crates/{game}-python/src/lib.rs"
    anchor = "use std::collections::"
    exports = json.dumps(str(HARNESS / "allocation_exports.rs"), ensure_ascii=False)
    replace_once(lib, anchor, f"#[path = {exports}]\nmod allocation_exports;\n\n{anchor}")
    if game == "awpy":
        scope(output / "crates/awpy/src/datasets.rs",
              "    fn player_states(ctx: &Context, keys: &SnapshotKeys) -> Vec<PlayerState> {",
              "Rows")
        columns = output / "crates/awpy-python/src/snapshot_columns.rs"
        for signature, stage in (
            ("pub fn push(&mut self, state: PlayerState) {", "Columns"),
            ("pub fn append(&mut self, mut other: Self) {", "Columns"),
            ("pub fn into_frame(self) -> PolarsResult<DataFrame> {", "Frame"),
        ):
            scope(columns, signature, stage, "                ")
    else:
        columns = output / "crates/boon-python/src/snapshots.rs"
        scope(columns, "        barriers: &BarrierState,\n    ) {", "Rows")
        scope(columns, "    pub(super) fn append(&mut self, mut o: PtCols) {", "Columns")
        scope(columns,
              "    /// Build the `player_ticks` DataFrame. Column order/names must match `load()`.\n"
              "    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {", "Frame")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", choices=("awpy", "boon"), required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--core", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--kind", choices=("release", "symbols", "allocations"), default="release")
    args = parser.parse_args()
    repo, core, output = (p.resolve() for p in (args.repo, args.core, args.output))
    if not output.is_relative_to(ROOT / "target") or output.exists():
        parser.error("--output must be a new directory beneath pbdems2/target")
    git_root = Path(subprocess.check_output(
        ["git", "-C", str(repo), "rev-parse", "--show-toplevel"], text=True).strip()).resolve()
    if git_root != repo:
        parser.error("--repo must be the consumer's Git root")
    if not (core / "Cargo.toml").is_file():
        parser.error("--core must contain the pbdems2 manifest")
    paths = subprocess.check_output([
        "git", "-C", str(repo), "ls-files", "-z", "--cached", "--others",
        "--exclude-standard", "--", "Cargo.toml", "Cargo.lock", "crates", ".cargo",
        "rust-toolchain.toml", "rust-toolchain",
    ]).decode().split("\0")
    output.mkdir(parents=True)
    for relative in filter(None, paths):
        source, destination = repo / relative, output / relative
        if not source.resolve().is_relative_to(repo) or not destination.resolve().is_relative_to(output):
            raise ValueError(f"snapshot path escapes its root: {relative}")
        if not source.exists():  # Respect working-tree deletions.
            continue
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    config = f"[patch.crates-io]\npbdems2 = {{ path = {json.dumps(str(core))} }}\n"
    if args.kind == "symbols":
        config += "\n[profile.release]\ndebug = 1\nstrip = false\n"
        for name in ("pbdems2", "awpy" if args.game == "awpy" else "boon-deadlock",
                     f"{args.game}-python"):
            config += f"[profile.release.package.{name}]\ndebug = 2\n"
    elif args.kind == "allocations":
        instrument(output, args.game)
    patch = output / "profile-config.toml"
    patch.write_text(config)
    (output / "profile-source.json").write_text(json.dumps({
        "consumer": str(repo), "core": str(core), "kind": args.kind,
        "consumer_head": subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip(),
        "consumer_status": subprocess.check_output(
            ["git", "-C", str(repo), "status", "--short"], text=True),
    }, indent=2) + "\n")
    common = ["--manifest-path", str(output / "Cargo.toml"), "--config", str(patch)]
    print(shlex.join(["cargo", "update", *common, "-p", "pbdems2", "--offline"]))
    print(shlex.join(["cargo", "build", *common, "-p", f"{args.game}-python",
                      "--release", "--locked", "--offline", "--target-dir",
                      str(output / "build"), "-j2"]))


if __name__ == "__main__":
    main()

