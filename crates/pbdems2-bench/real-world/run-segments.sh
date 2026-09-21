#!/usr/bin/env bash
set -euo pipefail
if [ "$#" -ne 4 ]; then
  echo "usage: PROFILE_PYTHON=... PROFILE_LIBS=... bash run-segments.sh CS2_A CS2_B DEADLOCK_A DEADLOCK_B" >&2
  exit 2
fi
runner="$(dirname -- "$0")/datasets.py"
for demo in "$1" "$2"; do
  for workload in snapshots_64 snapshots_1; do
    for segments in 1 4; do
      "${PROFILE_PYTHON:?}" "$runner" --game awpy --demo "$demo" \
        --extension-dir "${PROFILE_LIBS:?}" --workload "$workload" \
        --segments "$segments" --repeat 3 --warmup 1 --checksum
    done
  done
done
for demo in "$3" "$4"; do
  for segments in 1 4; do
    "${PROFILE_PYTHON:?}" "$runner" --game boon --demo "$demo" \
      --extension-dir "${PROFILE_LIBS:?}" --workload player_ticks \
      --segments "$segments" --repeat 3 --warmup 1 --checksum
  done
done
