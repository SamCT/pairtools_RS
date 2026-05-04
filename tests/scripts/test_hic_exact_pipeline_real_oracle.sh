#!/usr/bin/env bash
set -euo pipefail

TEST_DATA_DIR="${TEST_DATA_DIR:-/mnt/d/pairtools_RS_test}"
SORT_THREADS="${SORT_THREADS:-2}"
RUN_REAL_DOWNSTREAM="${RUN_REAL_DOWNSTREAM:-0}"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

die() {
  echo "error: $*" >&2
  exit 2
}

log() {
  echo "$*" >&2
}

split_command() {
  local value="$1"
  local -n out_ref="$2"
  read -r -a out_ref <<< "$value"
}

pick_one() {
  local label="$1"
  shift
  local values=("$@")
  if (( ${#values[@]} == 0 )); then
    die "no $label discovered"
  fi
  if (( ${#values[@]} > 1 )); then
    printf 'ambiguous %s files:\n' "$label" >&2
    printf '  %s\n' "${values[@]}" >&2
    die "set ${label^^} explicitly"
  fi
  printf "%s" "${values[0]}"
}

extract_assignment() {
  local var="$1" file="$2"
  [[ -r "$file" ]] || return 0
  awk -v var="$var" '
    $0 ~ "^[[:space:]]*" var "=" {
      sub("^[[:space:]]*" var "=", "", $0)
      gsub(/^["'\'']|["'\'']$/, "", $0)
      print $0
      exit
    }
  ' "$file"
}

default_pairs_rs() {
  local candidate
  candidate="${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}/debug/pairs-rs"
  if [[ -x "$candidate" ]]; then
    printf "%s" "$candidate"
  elif command -v pairs-rs >/dev/null 2>&1; then
    printf "pairs-rs"
  else
    die "set PAIRS_RS to a built pairs-rs binary"
  fi
}

default_tool() {
  local tool="$1"
  if command -v "$tool" >/dev/null 2>&1; then
    printf "%s" "$tool"
  elif command -v pixi >/dev/null 2>&1; then
    printf "pixi run %s" "$tool"
  else
    die "set ${tool^^}; neither $tool nor pixi is available"
  fi
}

compare_stats_if_compatible() {
  local candidate="$1" oracle="$2" aligned="$3" provenance="$4"
  python3 - "$candidate" "$oracle" "$aligned" "$provenance" <<'PY'
import math
import sys
from pathlib import Path

candidate, oracle, aligned, provenance = map(Path, sys.argv[1:])

def read_stats(path):
    rows = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) >= 2:
            rows[parts[0]] = parts[1]
    return rows

def numeric(value):
    try:
        return float(value)
    except ValueError:
        return None

oracle_rows = read_stats(oracle)
candidate_rows = read_stats(candidate)
aligned_lines = sum(1 for _ in aligned.open("rb"))
oracle_total = numeric(oracle_rows.get("total", "nan"))

if "out_s01.pairtools.parse.stats" in oracle.name:
    print(f"SKIP stats comparison: {oracle} comes from p3.commands with --drop-sam/--min-mapq 1, not the exact M080 target flags")
    raise SystemExit(0)
if oracle_total is not None and math.isfinite(oracle_total) and oracle_total > aligned_lines:
    print(f"SKIP stats comparison: {oracle} total={oracle_total:g} exceeds aligned input lines={aligned_lines}; not a compatible parse-stat oracle")
    raise SystemExit(0)
if provenance.exists() and "pairtools parse" not in provenance.read_text(encoding="utf-8", errors="ignore"):
    print(f"SKIP stats comparison: no pairtools parse provenance found for {oracle}")
    raise SystemExit(0)

missing = sorted(set(oracle_rows) - set(candidate_rows))
extra = sorted(set(candidate_rows) - set(oracle_rows))
diffs = []
for key in sorted(set(oracle_rows) & set(candidate_rows)):
    if oracle_rows[key] != candidate_rows[key]:
        diffs.append((key, oracle_rows[key], candidate_rows[key]))
if missing or extra or diffs:
    print(f"stats comparison failed for {oracle}")
    if missing:
        print("missing keys:", ", ".join(missing[:20]))
    if extra:
        print("extra keys:", ", ".join(extra[:20]))
    for key, expected, observed in diffs[:50]:
        print(f"{key}\toracle={expected}\tcandidate={observed}")
    raise SystemExit(1)
print(f"stats comparison passed: {oracle}")
PY
}

verify_sorted_pairsam() {
  local candidate="$1"
  python3 - "$candidate" <<'PY'
import gzip
import sys

path = sys.argv[1]
last = None
rows = 0
with gzip.open(path, "rt", encoding="utf-8", errors="replace") as handle:
    for line in handle:
        if not line or line.startswith("#"):
            continue
        fields = line.rstrip("\n").split("\t")
        if len(fields) < 8:
            raise SystemExit(f"data row has fewer than 8 columns: {line[:200]!r}")
        key = (fields[1], fields[3], int(fields[2]), int(fields[4]), fields[7])
        if last is not None and key < last:
            raise SystemExit(f"candidate sorted pairsam is out of order at row {rows + 1}: {key} < {last}")
        last = key
        rows += 1
print(f"candidate sorted pairsam order check passed for {rows} rows")
PY
}

compare_sorted_pairsam_if_present() {
  local candidate="$1"
  shift
  local oracles=("$@")
  if (( ${#oracles[@]} == 0 )); then
    log "SKIP sorted pairsam comparison: no exact *.sorted.pairsam.gz oracle discovered"
    return 0
  fi
  python3 - "$candidate" "${oracles[0]}" <<'PY'
import gzip
import sys

candidate, oracle = sys.argv[1:3]
volatile = ("#command:", "#samheader: @PG")
important_prefixes = ("## pairs format", "#columns:", "#chromsize:", "#genome_assembly:")

def normalized(path):
    rows = []
    important = []
    with gzip.open(path, "rt", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if line.startswith(volatile):
                continue
            if line.startswith(important_prefixes):
                important.append(line.rstrip("\n"))
            rows.append(line)
    return important, rows

c_imp, c_rows = normalized(candidate)
o_imp, o_rows = normalized(oracle)
if c_imp != o_imp:
    print("important header lines differ")
    print("oracle:", o_imp[:20])
    print("candidate:", c_imp[:20])
    raise SystemExit(1)
if c_rows != o_rows:
    for idx, (expected, observed) in enumerate(zip(o_rows, c_rows), start=1):
        if expected != observed:
            print(f"semantic sorted pairsam differs at line {idx}")
            print("oracle:", expected[:500].rstrip())
            print("candidate:", observed[:500].rstrip())
            break
    else:
        print(f"semantic sorted pairsam length differs: oracle={len(o_rows)} candidate={len(c_rows)}")
    raise SystemExit(1)
print(f"semantic sorted pairsam comparison passed: {oracle}")
PY
}

run_downstream_if_requested() {
  local sorted="$1" tmp="$2"
  if [[ "$RUN_REAL_DOWNSTREAM" != "1" ]]; then
    log "SKIP downstream comparison: set RUN_REAL_DOWNSTREAM=1 to run pairtools dedup/select/split/stats on candidate"
    return 0
  fi

  split_command "${PAIRTOOLS:-$(default_tool pairtools)}" PAIRTOOLS_CMD
  split_command "${SAMTOOLS:-$(default_tool samtools)}" SAMTOOLS_CMD
  mkdir -p "$tmp"
  (
    cd "$tmp"
    "${PAIRTOOLS_CMD[@]}" dedup \
      --mark-dups \
      --output-stats candidate.dedup.stats.txt \
      --output-dups candidate.dups.pairsam.gz \
      --output-unmapped candidate.unmapped.pairsam.gz \
      -o candidate.nodups.pairsam.gz \
      "$sorted"
    "${PAIRTOOLS_CMD[@]}" select \
      '(pair_type == "UU")' \
      -o candidate.valid.pairsam.gz \
      candidate.nodups.pairsam.gz
    "${PAIRTOOLS_CMD[@]}" split \
      --output-pairs candidate.valid.pairs.gz \
      --output-sam - \
      candidate.valid.pairsam.gz \
      | "${SAMTOOLS_CMD[@]}" view -@ "$SORT_THREADS" -b - \
      | "${SAMTOOLS_CMD[@]}" sort -@ "$SORT_THREADS" -o candidate.valid.coord.bam -
    "${SAMTOOLS_CMD[@]}" index candidate.valid.coord.bam
    "${SAMTOOLS_CMD[@]}" quickcheck candidate.valid.coord.bam
    "${PAIRTOOLS_CMD[@]}" stats \
      --with-chromsizes \
      -o candidate.valid.stats.txt \
      candidate.valid.pairs.gz
  )
  log "candidate downstream pipeline completed in $tmp"
}

[[ -d "$TEST_DATA_DIR" ]] || die "TEST_DATA_DIR does not exist: $TEST_DATA_DIR"

log "Discovered files under $TEST_DATA_DIR:"
find "$TEST_DATA_DIR" -maxdepth 3 -type f | sort >&2

mapfile -t aligned_candidates < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "*.bam" -o -name "*.sam" -o -name "*.cram" \) | sort)
mapfile -t chrom_candidates < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f -name "*.chrom.sizes" | sort)
mapfile -t sorted_pairsam_oracles < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "*.sorted.pairsam.gz" -o -name "merged.sorted.pairsam.gz" \) | sort)
mapfile -t sorted_pairs_oracles < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f -name "*.sorted.pairs" | sort)
mapfile -t parse_stats_oracles < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "*.parse.stats.txt" -o -name "*.pairtools.parse.stats" \) | sort)
mapfile -t downstream_oracles < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "merged.nodups.pairsam.gz" -o -name "merged.dups.pairsam.gz" -o -name "merged.unmapped.pairsam.gz" -o -name "merged.valid.pairsam.gz" -o -name "merged.valid.pairs.gz" -o -name "merged.valid.coord.bam" -o -name "merged.valid.stats.txt" \) | sort)

ALIGNED_BAM="${ALIGNED_BAM:-$(pick_one aligned "${aligned_candidates[@]}")}"
CHROMS="${CHROMS:-$(pick_one chroms "${chrom_candidates[@]}")}"
ASM="${ASM:-$(extract_assignment ASM "$TEST_DATA_DIR/pairtools_1.sh")}"
MAPQ="${MAPQ:-$(extract_assignment MAPQ "$TEST_DATA_DIR/pairtools_1.sh")}"
ASM="${ASM:-}"
MAPQ="${MAPQ:-}"
[[ -n "$ASM" ]] || die "assembly name is ambiguous; set ASM"
[[ -n "$MAPQ" ]] || die "MAPQ is ambiguous; set MAPQ"

PAIRS_RS="${PAIRS_RS:-$(default_pairs_rs)}"
split_command "$PAIRS_RS" PAIRS_RS_CMD

log "Classification:"
log "  aligned input: $ALIGNED_BAM"
log "  chrom sizes: $CHROMS"
log "  ASM: $ASM"
log "  MAPQ: $MAPQ"
printf '  exact sorted pairsam oracles:\n' >&2
printf '    %s\n' "${sorted_pairsam_oracles[@]:-none}" >&2
printf '  sorted pairs oracles:\n' >&2
printf '    %s\n' "${sorted_pairs_oracles[@]:-none}" >&2
printf '  parse stats oracles:\n' >&2
printf '    %s\n' "${parse_stats_oracles[@]:-none}" >&2
printf '  downstream oracles:\n' >&2
printf '    %s\n' "${downstream_oracles[@]:-none}" >&2

mkdir -p "$TMPROOT/sort_tmp"
candidate="$TMPROOT/candidate.sorted.pairsam.gz"
candidate_stats="$TMPROOT/candidate.parse.stats.txt"

log "Running candidate pairs-rs parse+sort:"
log "$(printf '%q ' "${PAIRS_RS_CMD[@]}" parse --chroms-path "$CHROMS" --assembly "$ASM" --min-mapq "$MAPQ" --walks-policy 5unique --max-inter-align-gap 30 --report-alignment-end 5 --add-columns mapq,pos5,pos3,cigar,read_len --output-stats "$candidate_stats" "$ALIGNED_BAM") | \\"
log "  $(printf '%q ' "${PAIRS_RS_CMD[@]}" sort --nproc "$SORT_THREADS" --tmpdir "$TMPROOT/sort_tmp" -o "$candidate")"

"${PAIRS_RS_CMD[@]}" parse \
  --chroms-path "$CHROMS" \
  --assembly "$ASM" \
  --min-mapq "$MAPQ" \
  --walks-policy 5unique \
  --max-inter-align-gap 30 \
  --report-alignment-end 5 \
  --add-columns mapq,pos5,pos3,cigar,read_len \
  --output-stats "$candidate_stats" \
  "$ALIGNED_BAM" \
  | "${PAIRS_RS_CMD[@]}" sort \
      --nproc "$SORT_THREADS" \
      --tmpdir "$TMPROOT/sort_tmp" \
      -o "$candidate"

[[ -s "$candidate" ]] || die "candidate sorted pairsam was not created"
[[ -s "$candidate_stats" ]] || die "candidate parse stats were not created"

split_command "${BGZIP:-$(default_tool bgzip)}" BGZIP_CMD
"${BGZIP_CMD[@]}" -t "$candidate"
verify_sorted_pairsam "$candidate"

compare_sorted_pairsam_if_present "$candidate" "${sorted_pairsam_oracles[@]}"

if (( ${#sorted_pairs_oracles[@]} > 0 )); then
  log "SKIP sorted pairs comparison: discovered oracle is .pairs, while the M080 target emits .pairsam.gz with SAM and extra columns"
fi

for stats in "${parse_stats_oracles[@]}"; do
  compare_stats_if_compatible "$candidate_stats" "$stats" "$ALIGNED_BAM" "$TEST_DATA_DIR/pairtools_1.sh"
done

run_downstream_if_requested "$candidate" "$TMPROOT/downstream"
