#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <input.sam|input.bam> <out_prefix>"
  exit 1
fi

INPUT="$1"
PREFIX="$2"

/usr/bin/time -f 'pairs-rs parse-lite real=%E user=%U sys=%S maxrss_kb=%M' \
  bash -lc "cat '$INPUT' | cargo run --release -- --no-header > '${PREFIX}.pairs'"

/usr/bin/time -f 'pairtools parse real=%E user=%U sys=%S maxrss_kb=%M' \
  bash -lc "cat '$INPUT' | pairtools parse --no-sam-headers --walks-policy 5unique --drop-readid > '${PREFIX}.pairtools.pairs'"

wc -l "${PREFIX}.pairs" "${PREFIX}.pairtools.pairs"
