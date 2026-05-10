#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEST_DATA_DIR="${TEST_DATA_DIR:-/mnt/d/pairtools_RS_test}"
SORT_THREADS="${SORT_THREADS:-2}"
THREADS="${THREADS:-2}"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT
CANDIDATE_DIR="${CANDIDATE_DIR:-$TMPROOT/candidate}"
CANDIDATE_PREFIX="${CANDIDATE_PREFIX:-$CANDIDATE_DIR/candidate}"
ORACLE_METADATA_DIR="${ORACLE_METADATA_DIR:-}"
ORACLE_METADATA_TARBALL="${ORACLE_METADATA_TARBALL:-}"
PARSE_MATRIX_METADATA_DIR="${PARSE_MATRIX_METADATA_DIR:-}"
PARSE_MATRIX_METADATA_TARBALL="${PARSE_MATRIX_METADATA_TARBALL:-}"

die() {
  echo "error: $*" >&2
  exit 2
}

log() {
  echo "$*" >&2
}

quote() {
  printf "%q" "$1"
}

quote_cmd() {
  local out="" q
  for arg in "$@"; do
    printf -v q "%q" "$arg"
    out+="${q} "
  done
  printf "%s" "${out% }"
}

print_env_line() {
  local key="$1" value="$2" suffix="${3:-\\}"
  printf '  %s=%q %s\n' "$key" "$value" "$suffix" >&2
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

display_command() {
  local var_name="$1" tool="$2" value
  value="${!var_name:-}"
  if [[ -n "$value" ]]; then
    printf "%s" "$value"
  elif command -v "$tool" >/dev/null 2>&1; then
    printf "%s" "$tool"
  elif command -v pixi >/dev/null 2>&1; then
    printf "pixi run %s" "$tool"
  else
    printf "%s" "$tool"
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

metadata_dir_has_baseline() {
  local dir="$1"
  [[ -r "$dir/oracle_baseline_paths.env" && -r "$dir/oracle_metadata.json" ]] || return 1
  grep -Eq '^ORACLE_SORTED_PAIRSAM=.' "$dir/oracle_baseline_paths.env"
}

metadata_dir_has_parse_matrix() {
  local dir="$1"
  [[ -r "$dir/oracle_baseline_paths.env" && -r "$dir/oracle_metadata.json" ]] || return 1
  [[ -s "$dir/oracle_parse_matrix_summary.tsv" ]] || return 1
  grep -q 'PT02_PARSE_MATRIX' "$dir/oracle_parse_matrix_summary.tsv"
}

extract_metadata_tarball() {
  local tarball="$1" dest="$2"
  [[ -r "$tarball" ]] || die "metadata tarball is not readable: $tarball"
  mkdir -p "$dest"
  tar -xzf "$tarball" -C "$dest"
  find "$dest" -maxdepth 2 -type f -name oracle_metadata.json -printf '%h\n' | head -1
}

resolve_oracle_metadata_dir() {
  local extracted candidate
  if [[ -n "$ORACLE_METADATA_TARBALL" ]]; then
    extracted="$(extract_metadata_tarball "$ORACLE_METADATA_TARBALL" "$TMPROOT/oracle_metadata_tar")"
    [[ -n "$extracted" ]] || die "metadata tarball did not contain oracle_metadata.json: $ORACLE_METADATA_TARBALL"
    ORACLE_METADATA_DIR="$extracted"
  fi

  local candidates=()
  [[ -n "$ORACLE_METADATA_DIR" ]] && candidates+=("$ORACLE_METADATA_DIR")
  candidates+=(
    "$TEST_DATA_DIR/Test_ou1.txt"
    "$TEST_DATA_DIR/metadata"
    "$REPO_ROOT/Test_ou1.txt"
    "$REPO_ROOT/metadata"
  )

  for candidate in "${candidates[@]}"; do
    if [[ -d "$candidate" ]] && metadata_dir_has_baseline "$candidate"; then
      ORACLE_METADATA_DIR="$candidate"
      return 0
    fi
  done

  for candidate in "$TEST_DATA_DIR/Test_ou1.txt.tar.gz" "$REPO_ROOT/Test_ou1.txt.tar.gz"; do
    if [[ -r "$candidate" ]]; then
      extracted="$(extract_metadata_tarball "$candidate" "$TMPROOT/oracle_metadata_tar_auto")"
      if [[ -n "$extracted" ]] && metadata_dir_has_baseline "$extracted"; then
        ORACLE_METADATA_DIR="$extracted"
        return 0
      fi
    fi
  done

  die "PT01 oracle metadata bundle not found; set ORACLE_METADATA_DIR or ORACLE_METADATA_TARBALL"
}

resolve_parse_matrix_metadata_dir() {
  local extracted candidate
  if [[ -n "$PARSE_MATRIX_METADATA_TARBALL" ]]; then
    extracted="$(extract_metadata_tarball "$PARSE_MATRIX_METADATA_TARBALL" "$TMPROOT/parse_matrix_metadata_tar")"
    [[ -n "$extracted" ]] || die "parse-matrix metadata tarball did not contain oracle_metadata.json: $PARSE_MATRIX_METADATA_TARBALL"
    PARSE_MATRIX_METADATA_DIR="$extracted"
  fi

  local candidates=()
  [[ -n "$PARSE_MATRIX_METADATA_DIR" ]] && candidates+=("$PARSE_MATRIX_METADATA_DIR")
  candidates+=(
    "$TEST_DATA_DIR/Test_out2.txt"
    "$TEST_DATA_DIR/Test_ou2.txt"
    "$REPO_ROOT/Test_out2.txt"
    "$REPO_ROOT/Test_ou2.txt"
  )

  for candidate in "${candidates[@]}"; do
    if [[ -d "$candidate" ]] && metadata_dir_has_parse_matrix "$candidate"; then
      PARSE_MATRIX_METADATA_DIR="$candidate"
      return 0
    fi
  done

  for candidate in \
    "$TEST_DATA_DIR/Test_out2.txt.tar.gz" \
    "$TEST_DATA_DIR/Test_ou2.txt.tar.gz" \
    "$REPO_ROOT/Test_out2.txt.tar.gz" \
    "$REPO_ROOT/Test_ou2.txt.tar.gz"; do
    if [[ -r "$candidate" ]]; then
      extracted="$(extract_metadata_tarball "$candidate" "$TMPROOT/parse_matrix_metadata_tar_auto")"
      if [[ -n "$extracted" ]] && metadata_dir_has_parse_matrix "$extracted"; then
        PARSE_MATRIX_METADATA_DIR="$extracted"
        return 0
      fi
    fi
  done

  PARSE_MATRIX_METADATA_DIR=""
}

load_env_assignments() {
  local file="$1" key value
  [[ -r "$file" ]] || die "metadata env file is not readable: $file"
  while IFS='=' read -r key value; do
    [[ "$key" =~ ^ORACLE_[A-Z0-9_]+$ ]] || continue
    printf -v "$key" '%s' "$value"
  done < "$file"
}

print_metadata_summary() {
  local dir="$1" label="$2"
  python3 - "$dir/oracle_metadata.json" "$label" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
label = sys.argv[2]
data = json.loads(path.read_text(encoding="utf-8"))
print(f"{label} metadata:", file=sys.stderr)
print(f"  metadata_dir: {path.parent}", file=sys.stderr)
print(f"  created_at: {data.get('created_at', 'UNKNOWN')}", file=sys.stderr)
print(f"  hostname: {data.get('hostname', 'UNKNOWN')}", file=sys.stderr)
print(f"  roots: {', '.join(data.get('roots', [])) or 'UNKNOWN'}", file=sys.stderr)
print(f"  file_count: {data.get('file_count', 'UNKNOWN')}", file=sys.stderr)
print(f"  total_bytes: {data.get('total_bytes', 'UNKNOWN')}", file=sys.stderr)
baseline = data.get("baseline_paths", {})
if baseline:
    print("  baseline paths:", file=sys.stderr)
    for key in sorted(baseline):
        print(f"    {key}={baseline[key]}", file=sys.stderr)
else:
    print("  baseline paths: none", file=sys.stderr)
PY
}

print_parse_matrix_summary() {
  [[ -n "$PARSE_MATRIX_METADATA_DIR" ]] || {
    log ""
    log "Parse matrix metadata: not found; this is optional for M161 and reserved for later parse/parse2 diagnostics."
    return 0
  }
  print_metadata_summary "$PARSE_MATRIX_METADATA_DIR" "Parse matrix"
  python3 - "$PARSE_MATRIX_METADATA_DIR/oracle_parse_matrix_summary.tsv" <<'PY'
import csv
import sys
from pathlib import Path

path = Path(sys.argv[1])
variants = []
with path.open(newline="", encoding="utf-8") as handle:
    reader = csv.DictReader(handle, delimiter="\t")
    for row in reader:
        if row.get("family") == "PT02_PARSE_MATRIX":
            variants.append(row.get("variant", ""))
print("  parse matrix variants:", file=sys.stderr)
for variant in variants[:40]:
    print(f"    - {variant}", file=sys.stderr)
if len(variants) > 40:
    print(f"    ... {len(variants) - 40} more", file=sys.stderr)
print("  note: parse matrix metadata is not used to pass M161; keep it for later parse/parse2 option-surface diagnostics.", file=sys.stderr)
PY
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

run_available_stage_comparisons() {
  if [[ "${RUN_AVAILABLE_STAGE_COMPARISONS:-0}" != "1" ]]; then
    log ""
    log "Available stage artifact checks: skipped."
    log "  Set RUN_AVAILABLE_STAGE_COMPARISONS=1 to compare the non-canonical files currently present in TEST_DATA_DIR."
    return 0
  fi

  log ""
  log "Available stage artifact checks:"
  if ! python3 - "$TEST_DATA_DIR" <<'PY'
import gzip
import math
import os
import sys
from pathlib import Path

root = Path(sys.argv[1])
failures = []

artifacts = {
    "pairtools parse stats": root / "parse_stats_STANDARD_s01_pairtools.txt",
    "pairs-rs parse stats": root / "s01.RS.parse.stats.txt",
    "pairs-rs alternate parse stats": root / "parse_RS.stats.txt",
    "pairtools dedup stats": root / "merged.dedup.s01.pairtoolsDEF.stats.txt",
    "pairs-rs dedup stats": root / "s01.RS.merged.dedup.stats.txt",
    "pairtools nodups pairsam": root / "nodups.parse_standard_s01.sorted.pairsam",
    "pairs-rs nodups pairsam.gz": root / "s01.RS.merged.nodups.pairsam.gz",
    "pairtools dups pairsam.gz": root / "merged.dups.pairsam.s01.pairtoolsDEF.gz",
    "pairs-rs dups pairsam.gz": root / "s01.RS.merged.dups.pairsam.gz",
    "pairtools unmapped pairsam.gz": root / "merged.unmapped.pairsam.s01.pairtoolsDEF.gz",
    "pairs-rs unmapped pairsam.gz": root / "s01.RS.merged.unmapped.pairsam.gz",
    "pairs-rs selected valid pairsam.gz": root / "s01.RS.merged.valid.pairsam.gz",
    "pairs-rs split valid pairs": root / "rs_s01.outpairs.split.pairs",
    "pairs-rs split valid SAM": root / "rs_s01_split_out.sam",
    "pairs-rs valid stats": root / "rs_s01.merged.valid.stats.txt",
}

for label, path in artifacts.items():
    status = "present" if path.exists() else "missing"
    print(f"  {label}: {status}: {path}", file=sys.stderr)

def read_stats(path):
    data = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) >= 2:
            data[parts[0]] = parts[1:]
    return data

def compare_parse_stats(candidate, oracle, label):
    if not candidate.exists() or not oracle.exists():
        print(f"  {label}: skipped; missing input", file=sys.stderr)
        return
    c = read_stats(candidate)
    o = read_stats(oracle)
    diffs = []
    for key in sorted(set(c) | set(o)):
        if c.get(key) == o.get(key):
            continue
        if key == "summary/complexity_naive":
            c_val = c.get(key, [""])[0]
            o_val = o.get(key, [""])[0]
            if {c_val, o_val} <= {"nan", "inf", "-inf"}:
                continue
            try:
                if math.isclose(float(c_val), float(o_val), rel_tol=1e-9, abs_tol=1e-9):
                    continue
            except ValueError:
                pass
        diffs.append((key, o.get(key), c.get(key)))
    if diffs:
        print(f"  {label}: FAIL", file=sys.stderr)
        for key, oracle_value, candidate_value in diffs[:20]:
            print(
                f"    {key}: oracle={oracle_value} candidate={candidate_value}",
                file=sys.stderr,
            )
        failures.append(f"{label} has {len(diffs)} differing stats fields")
    else:
        print(f"  {label}: PASS", file=sys.stderr)

def first_number(stats, key, default=None):
    if key not in stats:
        return default
    try:
        return int(float(stats[key][0]))
    except (ValueError, IndexError):
        return default

def compare_dedup_stats(candidate, oracle):
    if not candidate.exists() or not oracle.exists():
        print("  dedup stats core comparison: skipped; missing input", file=sys.stderr)
        return
    c = read_stats(candidate)
    o = read_stats(oracle)
    expected = {
        "total": first_number(o, "total"),
        "total_mapped": first_number(o, "total_mapped"),
        "total_unmapped": (
            first_number(o, "total_unmapped", 0)
            + first_number(o, "total_single_sided_mapped", 0)
        ),
        "total_dups": first_number(o, "total_dups"),
        "total_nodups": first_number(o, "total_nodups"),
    }
    observed = {key: first_number(c, key) for key in expected}
    diffs = [
        (key, expected[key], observed[key])
        for key in expected
        if expected[key] != observed[key]
    ]
    if diffs:
        print("  dedup stats core comparison: FAIL", file=sys.stderr)
        for key, expected_value, observed_value in diffs:
            print(
                f"    {key}: pairtools_expected={expected_value} pairs_rs={observed_value}",
                file=sys.stderr,
            )
        failures.append("dedup stats core counts differ for available real-data artifacts")
    else:
        print("  dedup stats core comparison: PASS", file=sys.stderr)

def read_body_read_ids(path):
    ids = []
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line or line.startswith("#"):
                continue
            ids.append(line.split("\t", 1)[0])
    return ids

def compare_duplicate_read_ids(candidate, oracle):
    if not candidate.exists() or not oracle.exists():
        print("  duplicate readID routing comparison: skipped; missing input", file=sys.stderr)
        return
    c = set(read_body_read_ids(candidate))
    o = set(read_body_read_ids(oracle))
    only_oracle = sorted(o - c)
    only_candidate = sorted(c - o)
    if only_oracle or only_candidate:
        print("  duplicate readID routing comparison: FAIL", file=sys.stderr)
        print(
            f"    pairtools_duplicate_readIDs={len(o)} pairs_rs_duplicate_readIDs={len(c)}",
            file=sys.stderr,
        )
        print(
            f"    only_pairtools={len(only_oracle)} only_pairs_rs={len(only_candidate)}",
            file=sys.stderr,
        )
        for label, values in (("only_pairtools", only_oracle), ("only_pairs_rs", only_candidate)):
            for value in values[:20]:
                print(f"    {label}: {value}", file=sys.stderr)
        failures.append("duplicate readID routing differs for available real-data artifacts")
    else:
        print("  duplicate readID routing comparison: PASS", file=sys.stderr)

def validate_gzip(path):
    with gzip.open(path, "rb") as handle:
        while handle.read(1024 * 1024):
            pass

if os.environ.get("RUN_AVAILABLE_STAGE_GZIP_TESTS") == "1":
    for label, path in artifacts.items():
        if path.suffix != ".gz" or not path.exists():
            continue
        try:
            validate_gzip(path)
            print(f"  gzip integrity {label}: PASS", file=sys.stderr)
        except Exception as exc:
            print(f"  gzip integrity {label}: FAIL: {exc}", file=sys.stderr)
            failures.append(f"gzip validation failed for {path}")
else:
    print(
        "  gzip integrity checks: skipped; set RUN_AVAILABLE_STAGE_GZIP_TESTS=1 to read all compressed artifacts",
        file=sys.stderr,
    )

compare_parse_stats(
    artifacts["pairs-rs parse stats"],
    artifacts["pairtools parse stats"],
    "parse stats pairs-rs vs pairtools",
)
compare_parse_stats(
    artifacts["pairs-rs alternate parse stats"],
    artifacts["pairtools parse stats"],
    "alternate parse stats pairs-rs vs pairtools",
)
compare_dedup_stats(
    artifacts["pairs-rs dedup stats"],
    artifacts["pairtools dedup stats"],
)
compare_duplicate_read_ids(
    artifacts["pairs-rs dups pairsam.gz"],
    artifacts["pairtools dups pairsam.gz"],
)

if artifacts["pairs-rs split valid pairs"].exists():
    print(
        "  split pairs artifact: rs_s01.outpairs.split.pairs is present as pairs text; treat it as the available split pairs table regardless of production .pairs.gz naming.",
        file=sys.stderr,
    )

print(
    "  note: available artifact checks do not complete M161; canonical merged.* pairtools oracle files are still required.",
    file=sys.stderr,
)

if failures:
    print("  available artifact blockers:", file=sys.stderr)
    for failure in failures:
        print(f"    - {failure}", file=sys.stderr)
    raise SystemExit(1)
PY
  then
    log "Available stage artifact checks reported blockers."
  fi
}

discover() {
  [[ -d "$TEST_DATA_DIR" ]] || die "TEST_DATA_DIR does not exist: $TEST_DATA_DIR"
  resolve_oracle_metadata_dir
  resolve_parse_matrix_metadata_dir
  load_env_assignments "$ORACLE_METADATA_DIR/oracle_baseline_paths.env"

  log "Discovering external real-data files under $TEST_DATA_DIR"
  find "$TEST_DATA_DIR" -maxdepth 3 -type f | sort >&2
  log ""
  print_metadata_summary "$ORACLE_METADATA_DIR" "PT01 baseline"
  print_parse_matrix_summary

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

  REQUIRED_ORACLE_KEYS=(
    ORACLE_SORTED_PAIRSAM
    ORACLE_NODUPS_PAIRSAM
    ORACLE_DUPS_PAIRSAM
    ORACLE_UNMAPPED_PAIRSAM
    ORACLE_VALID_PAIRSAM
    ORACLE_VALID_PAIRS
    ORACLE_VALID_STATS
    ORACLE_DEDUP_STATS
    ORACLE_PARSE_STATS
    ORACLE_VALID_BAM
    ORACLE_VALID_BAI
  )
  REQUIRED_ORACLES=(
    "${ORACLE_SORTED_PAIRSAM:-}"
    "${ORACLE_NODUPS_PAIRSAM:-}"
    "${ORACLE_DUPS_PAIRSAM:-}"
    "${ORACLE_UNMAPPED_PAIRSAM:-}"
    "${ORACLE_VALID_PAIRSAM:-}"
    "${ORACLE_VALID_PAIRS:-}"
    "${ORACLE_VALID_STATS:-}"
    "${ORACLE_DEDUP_STATS:-}"
    "${ORACLE_PARSE_STATS:-}"
    "${ORACLE_VALID_BAM:-}"
    "${ORACLE_VALID_BAI:-}"
  )
  CANDIDATE_OUTPUTS=(
    "$CANDIDATE_PREFIX.sorted.pairsam.gz"
    "$CANDIDATE_PREFIX.parse.stats.txt"
    "$CANDIDATE_DIR/merged.sorted.pairsam.gz"
    "$CANDIDATE_DIR/merged.nodups.pairsam.gz"
    "$CANDIDATE_DIR/merged.dups.pairsam.gz"
    "$CANDIDATE_DIR/merged.unmapped.pairsam.gz"
    "$CANDIDATE_DIR/merged.dedup.stats.txt"
    "$CANDIDATE_DIR/merged.valid.pairsam.gz"
    "$CANDIDATE_DIR/merged.valid.pairs.gz"
    "$CANDIDATE_DIR/merged.valid.coord.bam"
    "$CANDIDATE_DIR/merged.valid.coord.bam.bai"
    "$CANDIDATE_DIR/merged.valid.stats.txt"
  )
}

print_expected_layout() {
  log ""
  log "M161 expected external input directory:"
  log "  $TEST_DATA_DIR"
  log "PT01 oracle metadata directory:"
  log "  $ORACLE_METADATA_DIR"
  if [[ -n "$PARSE_MATRIX_METADATA_DIR" ]]; then
    log "Parse matrix metadata directory (diagnostic only):"
    log "  $PARSE_MATRIX_METADATA_DIR"
  fi
  log ""
  log "Discovered required inputs:"
  log "  R1: $R1"
  log "  R2: $R2"
  log "  CHROMS: $CHROMS"
  log "  ASM: $ASM"
  log "  MAPQ: $MAPQ"
  log "  BWA_INDEX: ${BWA_INDEX:-unset}"
  log ""
  log "Expected pairtools oracle files:"
  local idx key
  for idx in "${!REQUIRED_ORACLE_KEYS[@]}"; do
    key="${REQUIRED_ORACLE_KEYS[$idx]}"
    printf '  - %s=%s\n' "$key" "${REQUIRED_ORACLES[$idx]}" >&2
  done
  log ""
  log "Expected all-Rust candidate output paths:"
  printf '  - %s\n' "${CANDIDATE_OUTPUTS[@]}" >&2
}

print_oracle_generation_command() {
  local pairtools_cmd bwa_cmd samtools_cmd oracle_prefix tmp_for_oracle bwa_index_for_print
  pairtools_cmd="$(display_command PAIRTOOLS pairtools)"
  bwa_cmd="$(display_command BWA_MEM2 bwa-mem2)"
  samtools_cmd="$(display_command SAMTOOLS samtools)"
  oracle_prefix="$TEST_DATA_DIR/PT01_BASELINE_$(date -u +%Y%m%dT%H%M%S)"
  tmp_for_oracle="${ORACLE_TMPDIR:-$TEST_DATA_DIR/tmp_pairtools_oracle}"
  bwa_index_for_print="${BWA_INDEX:-SET_BWA_INDEX_PREFIX}"

  split_command "$pairtools_cmd" ORACLE_PAIRTOOLS_CMD
  split_command "$bwa_cmd" ORACLE_BWA_CMD
  split_command "$samtools_cmd" ORACLE_SAMTOOLS_CMD

  log ""
  log "Command to generate missing pairtools oracle outputs without requiring merged.* symlinks:"
  log "  # Run from the repository or shell environment where pairtools, bwa-mem2, and samtools are available."
  if [[ "$bwa_index_for_print" == "SET_BWA_INDEX_PREFIX" ]]; then
    log "  # Replace SET_BWA_INDEX_PREFIX with the real BWA-MEM2 index prefix before running."
  fi
  printf '  cd %q\n' "$TEST_DATA_DIR" >&2
  printf '  mkdir -p %q\n' "$tmp_for_oracle" >&2
  printf '  %s | \\\n' "$(quote_cmd "${ORACLE_BWA_CMD[@]}" mem -5SPM -T 30 -t "$THREADS" "$bwa_index_for_print" "$R1" "$R2")" >&2
  printf '    %s | \\\n' "$(quote_cmd "${ORACLE_PAIRTOOLS_CMD[@]}" parse --chroms-path "$CHROMS" --assembly "$ASM" --min-mapq "$MAPQ" --walks-policy 5unique --max-inter-align-gap 30 --report-alignment-end 5 --add-columns mapq,pos5,pos3,cigar,read_len --output-stats "$oracle_prefix.parse.stats.txt")" >&2
  printf '    %s\n' "$(quote_cmd "${ORACLE_PAIRTOOLS_CMD[@]}" sort --nproc "$SORT_THREADS" --tmpdir "$tmp_for_oracle" -o "$oracle_prefix.sorted.pairsam.gz")" >&2
  printf '  %s\n' "$(quote_cmd "${ORACLE_PAIRTOOLS_CMD[@]}" dedup --mark-dups --output-stats "$oracle_prefix.merged.dedup.stats.txt" --output-dups "$oracle_prefix.merged.dups.pairsam.gz" --output-unmapped "$oracle_prefix.merged.unmapped.pairsam.gz" -o "$oracle_prefix.merged.nodups.pairsam.gz" "$oracle_prefix.sorted.pairsam.gz")" >&2
  printf '  %s\n' "$(quote_cmd "${ORACLE_PAIRTOOLS_CMD[@]}" select '(pair_type == "UU")' -o "$oracle_prefix.merged.valid.pairsam.gz" "$oracle_prefix.merged.nodups.pairsam.gz")" >&2
  printf '  %s | \\\n' "$(quote_cmd "${ORACLE_PAIRTOOLS_CMD[@]}" split --output-pairs "$oracle_prefix.merged.valid.pairs.gz" --output-sam - "$oracle_prefix.merged.valid.pairsam.gz")" >&2
  printf '    %s | \\\n' "$(quote_cmd "${ORACLE_SAMTOOLS_CMD[@]}" view -@ "$SORT_THREADS" -b -)" >&2
  printf '    %s\n' "$(quote_cmd "${ORACLE_SAMTOOLS_CMD[@]}" sort -@ "$SORT_THREADS" -o "$oracle_prefix.merged.valid.coord.bam" -)" >&2
  printf '  %s\n' "$(quote_cmd "${ORACLE_SAMTOOLS_CMD[@]}" index "$oracle_prefix.merged.valid.coord.bam")" >&2
  printf '  %s\n' "$(quote_cmd "${ORACLE_PAIRTOOLS_CMD[@]}" stats --with-chromsizes -o "$oracle_prefix.merged.valid.stats.txt" "$oracle_prefix.merged.valid.pairs.gz")" >&2
  log "  # After generation, point ORACLE_METADATA_DIR at a refreshed metadata bundle containing these PT01 paths."
}

print_candidate_command() {
  local pairs_rs_for_print bwa_cmd samtools_cmd bgzip_cmd bwa_index_for_print
  pairs_rs_for_print="${PAIRS_RS:-${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}/debug/pairs-rs}"
  bwa_cmd="$(display_command BWA_MEM2 bwa-mem2)"
  samtools_cmd="$(display_command SAMTOOLS samtools)"
  bgzip_cmd="$(display_command BGZIP bgzip)"
  bwa_index_for_print="${BWA_INDEX:-SET_BWA_INDEX_PREFIX}"

  log ""
  log "Command to run the all-Rust candidate pipeline:"
  if [[ "$bwa_index_for_print" == "SET_BWA_INDEX_PREFIX" ]]; then
    log "  # Replace SET_BWA_INDEX_PREFIX with the real BWA-MEM2 index prefix before running."
  fi
  printf '  cd %q\n' "$REPO_ROOT" >&2
  print_env_line THREADS "$THREADS"
  print_env_line SORT_THREADS "$SORT_THREADS"
  print_env_line MAPQ "$MAPQ"
  print_env_line BWA_INDEX "$bwa_index_for_print"
  print_env_line CHROMS "$CHROMS"
  print_env_line ASM "$ASM"
  print_env_line PREFIX "$CANDIDATE_PREFIX"
  print_env_line TMPDIR "$TMPROOT/tmp"
  print_env_line R1 "$R1"
  print_env_line R2 "$R2"
  print_env_line PAIRS_RS "$pairs_rs_for_print"
  print_env_line BWA_MEM2 "$bwa_cmd"
  print_env_line SAMTOOLS "$samtools_cmd"
  print_env_line BGZIP "$bgzip_cmd"
  printf '  bash scripts/run_hic_all_rust_pairs_rs_pipeline.sh\n' >&2
}

report_missing_oracles() {
  local missing=()
  local idx key path
  for idx in "${!REQUIRED_ORACLES[@]}"; do
    key="${REQUIRED_ORACLE_KEYS[$idx]}"
    path="${REQUIRED_ORACLES[$idx]}"
    if [[ -z "$path" ]]; then
      missing+=("$key is empty in $ORACLE_METADATA_DIR/oracle_baseline_paths.env")
    elif [[ ! -r "$path" ]]; then
      missing+=("$key is not readable: $path")
    fi
  done
  if [[ -z "$BWA_INDEX" ]]; then
    missing+=("BWA_INDEX prefix with index files")
  elif [[ ! -e "$BWA_INDEX" ]] && ! compgen -G "${BWA_INDEX}*" >/dev/null 2>&1; then
    missing+=("BWA_INDEX prefix with index files")
  fi

  if (( ${#missing[@]} > 0 )); then
    log "M161 blocker: required exact all-Rust pipeline oracle inputs are missing."
    log "The harness reads explicit PT01 paths from: $ORACLE_METADATA_DIR/oracle_baseline_paths.env"
    log "Missing:"
    printf '  - %s\n' "${missing[@]}" >&2
    die "external real-data oracle set is incomplete"
  fi
}

run_candidate_pipeline() {
  local outdir="$CANDIDATE_DIR"
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
      PREFIX="$CANDIDATE_PREFIX" \
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
  compare_gz_text "$CANDIDATE_DIR/merged.sorted.pairsam.gz" "$ORACLE_SORTED_PAIRSAM" "merged.sorted.pairsam"
  compare_stats "$CANDIDATE_PREFIX.parse.stats.txt" "$ORACLE_PARSE_STATS"
  compare_gz_text "$CANDIDATE_DIR/merged.nodups.pairsam.gz" "$ORACLE_NODUPS_PAIRSAM" "merged.nodups.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.dups.pairsam.gz" "$ORACLE_DUPS_PAIRSAM" "merged.dups.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.unmapped.pairsam.gz" "$ORACLE_UNMAPPED_PAIRSAM" "merged.unmapped.pairsam"
  compare_stats "$CANDIDATE_DIR/merged.dedup.stats.txt" "$ORACLE_DEDUP_STATS"
  compare_gz_text "$CANDIDATE_DIR/merged.valid.pairsam.gz" "$ORACLE_VALID_PAIRSAM" "merged.valid.pairsam"
  compare_gz_text "$CANDIDATE_DIR/merged.valid.pairs.gz" "$ORACLE_VALID_PAIRS" "merged.valid.pairs"
  compare_stats "$CANDIDATE_DIR/merged.valid.stats.txt" "$ORACLE_VALID_STATS"

  if [[ -r "$ORACLE_VALID_BAM" ]]; then
    split_command "${SAMTOOLS:-$(command_or_pixi samtools)}" SAMTOOLS_CMD
    "${SAMTOOLS_CMD[@]}" quickcheck "$CANDIDATE_DIR/merged.valid.coord.bam"
    "${SAMTOOLS_CMD[@]}" quickcheck "$ORACLE_VALID_BAM"
    diff -u <("${SAMTOOLS_CMD[@]}" flagstat "$ORACLE_VALID_BAM") <("${SAMTOOLS_CMD[@]}" flagstat "$CANDIDATE_DIR/merged.valid.coord.bam")
  fi
}

discover
print_expected_layout
print_oracle_generation_command
print_candidate_command
run_available_stage_comparisons
report_missing_oracles
run_candidate_pipeline
compare_outputs
log "M161 all-Rust real-data oracle validation passed"
