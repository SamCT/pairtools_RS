#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

PAIRS_RS="${PAIRS_RS:-${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}/debug/pairs-rs}"
PAIRTOOLS="${PAIRTOOLS:-pixi run pairtools}"
BGZIP="${BGZIP:-pixi run bgzip}"
GZIP="${GZIP:-pixi run gzip}"
SAMTOOLS="${SAMTOOLS:-pixi run samtools}"

if [[ ! -x "$PAIRS_RS" ]]; then
  echo "missing pairs-rs binary: $PAIRS_RS" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

input_plain="$tmp/merged.valid.pairsam"
input_gz="$tmp/merged.valid.pairsam.gz"
candidate_pairs="$tmp/merged.valid.pairs.gz"
candidate_sam="$tmp/merged.valid.sam"
oracle_pairs="$tmp/oracle.valid.pairs.gz"
oracle_sam="$tmp/oracle.valid.sam"

python3 - <<'PY' > "$input_plain"
from pathlib import Path

text = Path("tests/data/mock.pairsam").read_text()
print(text.replace("\x19101M\x19", "\x192M\x19"), end="")
PY

$BGZIP -c "$input_plain" > "$input_gz"

"$PAIRS_RS" split \
  --output-pairs "$candidate_pairs" \
  --output-sam - \
  "$input_gz" \
  > "$candidate_sam"

test -s "$candidate_pairs"
test -s "$candidate_sam"

$GZIP -dc "$candidate_pairs" > "$tmp/candidate.pairs"
candidate_body_rows="$(grep -vc '^#' "$tmp/candidate.pairs")"
if [[ "$candidate_body_rows" -le 0 ]]; then
  echo "candidate pairs output has no body rows" >&2
  exit 1
fi

candidate_sam_rows="$(grep -vc '^@' "$candidate_sam")"
if [[ "$candidate_sam_rows" -le 0 ]]; then
  echo "candidate SAM output has no alignment rows" >&2
  exit 1
fi

if $SAMTOOLS --version >/dev/null 2>&1; then
  $SAMTOOLS view -S -b "$candidate_sam" >/dev/null
else
  echo "samtools unavailable; skipped SAM parse check" >&2
fi

$PAIRTOOLS split \
  --output-pairs "$oracle_pairs" \
  --output-sam "$oracle_sam" \
  "$input_gz"

$GZIP -dc "$oracle_pairs" > "$tmp/oracle.pairs"

grep -v '^#samheader: @PG	ID:pairtools_split' "$tmp/candidate.pairs" > "$tmp/candidate.pairs.norm"
grep -v '^#samheader: @PG	ID:pairtools_split' "$tmp/oracle.pairs" > "$tmp/oracle.pairs.norm"
diff -u "$tmp/oracle.pairs.norm" "$tmp/candidate.pairs.norm"

grep -v '^@PG	ID:pairtools_split' "$candidate_sam" > "$tmp/candidate.sam.norm"
grep -v '^@PG	ID:pairtools_split' "$oracle_sam" > "$tmp/oracle.sam.norm"
diff -u "$tmp/oracle.sam.norm" "$tmp/candidate.sam.norm"

cut -f1 "$tmp/candidate.pairs" | grep -v '^#' > "$tmp/candidate.pairs.readids"
cut -f1 "$tmp/oracle.pairs" | grep -v '^#' > "$tmp/oracle.pairs.readids"
diff -u "$tmp/oracle.pairs.readids" "$tmp/candidate.pairs.readids"

cut -f1 "$candidate_sam" | grep -v '^@' > "$tmp/candidate.sam.readids"
cut -f1 "$oracle_sam" | grep -v '^@' > "$tmp/oracle.sam.readids"
diff -u "$tmp/oracle.sam.readids" "$tmp/candidate.sam.readids"

echo "split production command shape validation passed"
