#!/usr/bin/env bash
set -euo pipefail
if [ "$#" -ne 4 ]; then
  echo "usage: PROFILE_PYTHON=... PROFILE_LIBS=... PERF=... PROFILE_OUT=... bash run-profiles.sh CS2_A CS2_B DEADLOCK_A DEADLOCK_B" >&2
  exit 2
fi
runner="$(dirname -- "$0")/datasets.py"
mkdir -p "${PROFILE_OUT:?}"
read -r -a segments <<< "${PROFILE_SEGMENTS:-1}"
record() {
  local game="$1" demo="$2" workload="$3" count="$4" prefix
  prefix="$PROFILE_OUT/$game-$(basename -- "$demo" .dem)-$workload-s$count"
  if [ -e "$prefix.perf" ]; then
    echo "Refusing to overwrite $prefix.perf" >&2
    exit 1
  fi
  echo "Recording $game $(basename -- "$demo") $workload" >&2
  "${PERF:?}" record -e cpu-clock:u -F 499 -k mono --call-graph dwarf,16384 \
    --no-buildid-cache -o "$prefix.perf" -- "${PROFILE_PYTHON:?}" "$runner" \
    --game "$game" --demo "$demo" --extension-dir "${PROFILE_LIBS:?}" \
    --workload "$workload" --segments "$count" --repeat "${PROFILE_REPEATS:-3}" --warmup 0 \
    > "$prefix.windows.jsonl" 2> "$prefix.record.log"
}
for demo in "$1" "$2"; do
  for workload in snapshots_64 snapshots_1 events; do
    for count in "${segments[@]}"; do
      if [ "$count" = 1 ] || [ "$workload" != events ]; then
        record awpy "$demo" "$workload" "$count"
      fi
    done
  done
done
for demo in "$3" "$4"; do
  for workload in player_ticks ability_ticks combat; do
    for count in "${segments[@]}"; do
      if [ "$count" = 1 ] || [ "$workload" = player_ticks ]; then
        record boon "$demo" "$workload" "$count"
      fi
    done
  done
done
