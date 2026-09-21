#!/usr/bin/env bash
set -euo pipefail
runner="$(dirname -- "$0")/analyze.py"
for recording in "${PROFILE_OUT:?}"/*.perf; do
  prefix="${recording%.perf}"
  echo "Analyzing $(basename -- "$prefix")" >&2
  "${SAMPLY:?}" import "$recording" --save-only -o "$prefix.samply.json.gz" \
    > "$prefix.import.log" 2>&1
  python3 "$runner" --profile "$prefix.samply.json.gz" --perf-data "$recording" \
    --perf "${PERF:?}" --symbolizer "${LLVM_SYMBOLIZER:?}" \
    --windows "$prefix.windows.jsonl" --folded "$prefix.folded" > "$prefix.analysis.json"
done
