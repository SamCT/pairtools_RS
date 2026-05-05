#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEST_DATA_DIR="${TEST_DATA_DIR:-/mnt/d/pairtools_RS_test}"
SORT_THREADS="${SORT_THREADS:-2}"
THREADS="${THREADS:-2}"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

die() {
  echo "error: $*" >&2
  exit 2
}

log() {
  echo "$*" >&2
}

pick_one_optional() {
  local label="$1"
  shift
  local values=("$@")
  if (( ${#values[@]} == 0 )); then
    return 1
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

command_or_pixi() {
  local tool="$1"
  if command -v "$tool" >/dev/null 2>&1; then
    printf "%s" "$tool"
  elif command -v pixi >/dev/null 2>&1; then
    printf "pixi run %s" "$tool"
  else
    return 1
  fi
}

split_command() {
  local value="$1"
  local -n out_ref="$2"
  read -r -a out_ref <<< "$value"
}

require_file() {
  local path="$1" label="$2"
  [[ -n "$path" ]] || die "$label is empty"
  [[ -r "$path" ]] || die "$label is not readable: $path"
}

normalize_text_gz() {
  local input="$1" output="$2"
  python3 - "$input" "$output" <<'PY'
import gzip
import sys

source, dest = sys.argv[1:3]
volatile_prefixes = ("#command:", "#samheader: @PG")
with gzip.open(source, "rt", encoding="utf-8", errors="replace") as handle, open(dest, "w", encoding="utf-8") as out:
    for line in handle:
        if line.startswith(volatile_prefixes):
            continue
        out.write(line)
PY
}

compare_gz_text() {
  local candidate="$1" oracle="$2" label="$3"
  local c_norm="$TMPROOT/${label}.candidate.txt"
  local o_norm="$TMPROOT/${label}.oracle.txt"
  normalize_text_gz "$candidate" "$c_norm"
  normalize_text_gz "$oracle" "$o_norm"
  diff -u "$o_norm" "$c_norm"
}

compare_stats() {
  local candidate="$1" oracle="$2"
  python3 - "$candidate" "$oracle" <<'PY'
import sys
from pathlib import Path

candidate, oracle = map(Path, sys.argv[1:3])

def rows(path):
    data = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) >= 2:
            data[parts[0]] = parts[1:]
    return data

c = rows(candidate)
o = rows(oracle)
if c != o:
    for key in sorted(set(c) | set(o)):
        if c.get(key) != o.get(key):
            print(f"{key}\toracle={o.get(key)}\tcandidate={c.get(key)}")
    raise SystemExit(1)
PY
}

discover() {
  [[ -d "$TEST_DATA_DIR" ]] || die "TEST_DATA_DIR does not exist: $TEST_DATA_DIR"

  log "Discovering external real-data files under $TEST_DATA_DIR"
  find "$TEST_DATA_DIR" -maxdepth 3 -type f | sort >&2

  mapfile -t r1_candidates < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "*R1*.fastq.gz" -o -name "*_1*.fastq.gz" \) | sort)
  mapfile -t r2_candidates < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "*R2*.fastq.gz" -o -name "*_2*.fastq.gz" \) | sort)
  mapfile -t chrom_candidates < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f -name "*.chrom.sizes" | sort)

  R1="${R1:-$(pick_one_optional r1 "${r1_candidates[@]}" || true)}"
  R2="${R2:-$(pick_one_optional r2 "${r2_candidates[@]}" || true)}"
  CHROMS="${CHROMS:-$(pick_one_optional chroms "${chrom_candidates[@]}" || true)}"
  ASM="${ASM:-$(extract_assignment ASM "$TEST_DATA_DIR/pairtools_1.sh")}"
  MAPQ="${MAPQ:-$(extract_assignment MAPQ "$TEST_DATA_DIR/pairtools_1.sh")}"
  ASM="${ASM:-}"
  MAPQ="${MAPQ:-}"

  require_file "$R1" "R1"
  require_file "$R2" "R2"
  require_file "$CHROMS" "CHROMS"
  [[ -n "$ASM" ]] || die "assembly name is ambiguous; set ASM"
  [[ -n "$MAPQ" ]] || die "MAPQ is ambiguous; set MAPQ"

  BWA_INDEX="${BWA_INDEX:-}"
  if [[ -z "$BWA_INDEX" ]]; then
    mapfile -t bwa_index_candidates < <(find "$TEST_DATA_DIR" -maxdepth 3 -type f \( -name "*.bwt" -o -name "*.0123" -o -name "*.amb" -o -name "*.ann" -o -name "*.pac" -o -name "*.sa" \) | sort)
    if (( ${#bwa_index_candidates[@]} > 0 )); then
      BWA_INDEX="${bwa_index_candidates[0]%.*}"
    fi
  fi

  REQUIRED_ORACLES=(
    "$TEST_DATA_DIR/merged.sorted.pairsam.gz"
    "$TEST_DATA_DIR/merged.nodups.pairsam.gz"
    "$TEST_DATA_DIR/merged.dups.pairsam.gz"
    "$TEST_DATA_DIR/merged.unmapped.pairsam.gz"
    "$TEST_DATA_DIR/merged.valid.pairsam.gz"
    "$TEST_DATA_DIR/merged.valid.pairs.gz"
    "$TEST_DATA_DIR/merged.valid.stats.txt"
  )
}

report_missing_oracles() {
  local missing=()
  local path
  for path in "${REQUIRED_ORACLES[@]}"; do
    [[ -r "$path" ]] || missing+=("$path")
  done
  if [[ -z "$BWA_INDEX" || ! -e "$BWA_INDEX" ]] && ! compgen -G "${BWA_INDEX}*" >/dev/null 2>&1; then
    missing+=("BWA_INDEX prefix with index files")
  fi

  if (( ${#missing[@]} > 0 )); then
    log "M161 blocker: required exact all-Rust pipeline oracle inputs are missing."
    log "Detected:"
    log "  R1: $R1"
    log "  R2: $R2"
    log "  CHROMS: $CHROMS"
    log "  ASM: $ASM"
    log "  MAPQ: $MAPQ"
    log "  BWA_INDEX: ${BWA_INDEX:-unset}"
    log "Missing:"
    printf '  - %s\n' "${missing[@]}" >&2
    die "external real-data oracle set is incomplete"
  fi
}

run_candidate_pipeline() {
  local outdir="$TMPROOT/candidate"
  mkdir -p "$outdir" "$TMPROOT/tmp"
  PAIRS_RS="${PAIRS_RS:-${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}/debug/pairs-rs}"
  BWA_MEM2="${BWA_MEM2:-$(command_or_pixi bwa-mem2)}"
  SAMTOOLS="${SAMTOOLS:-$(command_or_pixi samtools)}"
  BGZIP="${BGZIP:-$(command_or_pixi bgzip)}"

  require_file "$PAIRS_RS" "PAIRS_RS"
  split_command "$BWA_MEM2" BWA_MEM2_CMD
  split_command "$SAMTOOLS" SAMTOOLS_CMD
  split_command "$BGZIP" BGZIP_CMD

  log "Running all-Rust pipeline candidate in $outdir"
  (
    cd "$REPO_ROOT"
    THREADS="$THREADS" \
      SORT_THREADS="$SORT_THREADS" \
      MAPQ="$MAPQ" \
      BWA_INDEX="$BWA_INDEX" \
      CHROMS="$CHROMS" \
      ASM="$ASM" \
      PREFIX="$outdir/merged" \
      TMPDIR="$TMPROOT/tmp" \
      R1="$R1" \
      R2="$R2" \
      PAIRS_RS="$PAIRS_RS" \
      BWA_MEM2="$BWA_MEM2" \
      SAMTOOLS="$SAMTOOLS" \
      BGZIP="$BGZIP" \
      bash scripts/run_hic_all_rust_pairs_rs_pipeline.sh
  )
  CANDIDATE_DIR="$outdir"
}

compare_outputs() {
  compare_gz_text "$CANDIDATE_DIR/merged.sorted.pairsam.gz" "$TEST_DATA_DIR/merged.sorted.pairsam.gz" "merged.sorted.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.nodups.pairsam.gz" "$TEST_DATA_DIR/merged.nodups.pairsam.gz" "merged.nodups.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.dups.pairsam.gz" "$TEST_DATA_DIR/merged.dups.pairsam.gz" "merged.dups.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.unmapped.pairsam.gz" "$TEST_DATA_DIR/merged.unmapped.pairsam.gz" "merged.unmapped.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.valid.pairsam.gz" "$TEST_DATA_DIR/merged.valid.pairsam.gz" "merged.valid.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.valid.pairs.gz" "$TEST_DATA_DIR/merged.valid.pairs.gz" "merged.valid.pairs"
  compare_stats "$CANDIDATE_DIR/merged.valid.stats.txt" "$TEST_DATA_DIR/merged.valid.stats.txt"

  if [[ -r "$TEST_DATA_DIR/merged.valid.coord.bam" ]]; then
    split_command "${SAMTOOLS:-$(command_or_pixi samtools)}" SAMTOOLS_CMD
    "${SAMTOOLS_CMD[@]}" quickcheck "$CANDIDATE_DIR/merged.valid.coord.bam"
    "${SAMTOOLS_CMD[@]}" quickcheck "$TEST_DATA_DIR/merged.valid.coord.bam"
    diff -u <("${SAMTOOLS_CMD[@]}" flagstat "$TEST_DATA_DIR/merged.valid.coord.bam") <("${SAMTOOLS_CMD[@]}" flagstat "$CANDIDATE_DIR/merged.valid.coord.bam")
  fi
}

discover
report_missing_oracles
run_candidate_pipeline
compare_outputs
log "M161 all-Rust real-data oracle validation passed"
