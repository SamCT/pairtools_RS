#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 5 ]]; then
  echo "Usage: $0 <bwa_index_prefix> <R1.fastq.gz> <R2.fastq.gz> <chromsizes.txt> <out_prefix> [threads]"
  exit 1
fi

IDX="$1"
R1="$2"
R2="$3"
CHROMSIZES="$4"
OUT="$5"
THREADS="${6:-8}"

# 1) Align with bwa-mem2 and stream SAM
# 2) Parse with Rust parse-lite into minimal .pairs
# 3) Keep pairtools sort/dedup unchanged
/usr/bin/time -f 'pairs-rs pipeline real=%E user=%U sys=%S maxrss_kb=%M' \
  bash -lc "bwa-mem2 mem -t ${THREADS} -5SP '${IDX}' '${R1}' '${R2}' \
  | cargo run --release -- --no-header --threads ${THREADS} \
  > '${OUT}.pairs'"

/usr/bin/time -f 'pairtools sort real=%E user=%U sys=%S maxrss_kb=%M' \
  bash -lc "pairtools sort --threads ${THREADS} --tmpdir . --chroms-path '${CHROMSIZES}' \
  '${OUT}.pairs' > '${OUT}.sorted.pairs'"

/usr/bin/time -f 'pairtools dedup real=%E user=%U sys=%S maxrss_kb=%M' \
  bash -lc "pairtools dedup --threads-in ${THREADS} --threads-out ${THREADS} \
  '${OUT}.sorted.pairs' > '${OUT}.dedup.pairs'"

wc -l "${OUT}.pairs" "${OUT}.sorted.pairs" "${OUT}.dedup.pairs"
