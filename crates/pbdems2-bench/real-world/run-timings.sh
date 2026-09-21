#!/usr/bin/env bash
set -euo pipefail
if [ "$#" -ne 4 ]; then
  echo "usage: PROFILE_PYTHON=... PROFILE_LIBS=... bash run-timings.sh CS2_A CS2_B DEADLOCK_A DEADLOCK_B" >&2
  exit 2
fi
runner="$(dirname -- "$0")/datasets.py"
for demo in "$1" "$2"; do
  for workload in snapshots_64 snapshots_1 events; do
    "${PROFILE_PYTHON:?}" "$runner" --game awpy --demo "$demo" \
      --extension-dir "${PROFILE_LIBS:?}" --workload "$workload" --repeat 3 --warmup 1
  done
done
for demo in "$3" "$4"; do
  for workload in player_ticks ability_ticks combat; do
    "${PROFILE_PYTHON:?}" "$runner" --game boon --demo "$demo" \
      --extension-dir "${PROFILE_LIBS:?}" --workload "$workload" --repeat 3 --warmup 1
  done
done
