#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  PAIRTOOLS_RS_TESTDATA=/mnt/d/pairtools_RS_testdata/hop_s01 \
    scripts/real_bam_compare.sh [--compare] [--benchmark] [--full-gate]

Purpose:
  External-data parity and benchmark harness for a real alignment BAM.
  The script is opt-in and is not run by default cargo tests.

Required input files:
  $PAIRTOOLS_RS_TESTDATA/BWAMEM2_R1R2_s01.bam
  $PAIRTOOLS_RS_TESTDATA/Hop282H1.chrom.sizes

Optional golden file:
  $PAIRTOOLS_RS_TESTDATA/out_s01.PAIRTOOLSDEF.sorted.pairs

Environment knobs:
  PAIRTOOLS_RS_TESTDATA      Directory containing the real test files.
  PAIRS_RS_BIN              pairs-rs binary. Defaults to target/release/pairs-rs,
                            then target/debug/pairs-rs.
  REAL_BAM                  BAM basename. Default: BWAMEM2_R1R2_s01.bam
  REAL_CHROMS               chrom.sizes basename. Default: Hop282H1.chrom.sizes
  EXPECTED_SORTED_PAIRS     Optional exact pairtools parse+sort output.
  MAPQ                      Supported-subset MAPQ. Default: 1
  REPORT_ALIGNMENT_END      Supported-subset reported alignment end. Default: 5
  BENCHMARK_RUNS            hyperfine runs. Default: 3
  COMPARE_WORKDIR           Working/output directory. Default: mktemp dir.
  KEEP_COMPARE_WORKDIR      If set to 1, do not delete the workdir.
  ASM                       Parse-only pairsam assembly. Default: HopH1_282
  FULL_MAPQ                 Parse-only pairsam MAPQ. Default: 10
  FULL_MAX_INTER_ALIGN_GAP  Parse-only pairsam gap. Default: 30

Modes:
  --compare     Run exact supported-subset comparison. Default if no mode is set.
  --benchmark   Run hyperfine only after exact comparison passes.
  --full-gate   Verify currently unsupported parse options still fail loudly.
  -h, --help    Show this help.

Notes:
  The benchmark still covers only the original drop-sam supported subset:
    parse --drop-sam --min-mapq MAPQ --walks-policy 5unique --report-alignment-end 5 | sort
  --compare also runs a parse-only pairsam comparison with assembly, max-inter-align-gap,
  and add-columns mapq,pos5,pos3,cigar,read_len, then diffs normalized output.
USAGE
}

run_compare=0
run_benchmark=0
run_full_gate=0

if (($# == 0)); then
  run_compare=1
fi

while (($#)); do
  case "$1" in
    --compare)
      run_compare=1
      ;;
    --benchmark)
      run_compare=1
      run_benchmark=1
      ;;
    --full-gate)
      run_full_gate=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data_dir="${PAIRTOOLS_RS_TESTDATA:-}"
if [[ -z "$data_dir" ]]; then
  echo "PAIRTOOLS_RS_TESTDATA is required, e.g. /mnt/d/pairtools_RS_testdata/hop_s01" >&2
  exit 2
fi

bam="$data_dir/${REAL_BAM:-BWAMEM2_R1R2_s01.bam}"
chroms="$data_dir/${REAL_CHROMS:-Hop282H1.chrom.sizes}"
expected="${EXPECTED_SORTED_PAIRS:-$data_dir/out_s01.PAIRTOOLSDEF.sorted.pairs}"
mapq="${MAPQ:-1}"
report_end="${REPORT_ALIGNMENT_END:-5}"
asm="${ASM:-HopH1_282}"
full_mapq="${FULL_MAPQ:-10}"
full_gap="${FULL_MAX_INTER_ALIGN_GAP:-30}"

if [[ ! -f "$bam" ]]; then
  echo "Missing BAM: $bam" >&2
  exit 2
fi
if [[ ! -f "$chroms" ]]; then
  echo "Missing chrom sizes: $chroms" >&2
  exit 2
fi

pairs_rs_bin="${PAIRS_RS_BIN:-}"
if [[ -z "$pairs_rs_bin" ]]; then
  if [[ -x "$repo_root/target/release/pairs-rs" ]]; then
    pairs_rs_bin="$repo_root/target/release/pairs-rs"
  elif [[ -x "$repo_root/target/debug/pairs-rs" ]]; then
    pairs_rs_bin="$repo_root/target/debug/pairs-rs"
  else
    echo "Missing pairs-rs binary. Build first, e.g.:" >&2
    echo "  export CARGO_TARGET_DIR=\"\$HOME/pairtools_RS_target\"" >&2
    echo "  pixi run cargo build --release" >&2
    echo "Then set PAIRS_RS_BIN if the binary is outside target/." >&2
    exit 2
  fi
fi

if [[ ! -x "$pairs_rs_bin" ]]; then
  echo "PAIRS_RS_BIN is not executable: $pairs_rs_bin" >&2
  exit 2
fi

workdir="${COMPARE_WORKDIR:-}"
if [[ -z "$workdir" ]]; then
  workdir="$(mktemp -d "${TMPDIR:-/tmp}/pairtools-rs-real-bam.XXXXXX")"
fi
mkdir -p "$workdir"

cleanup() {
  if [[ "${KEEP_COMPARE_WORKDIR:-0}" != "1" && -n "${workdir:-}" && -d "$workdir" ]]; then
    rm -rf "$workdir"
  fi
}
trap cleanup EXIT

pairtools_out="$workdir/pairtools.supported.sorted.pairs"
pairs_rs_out="$workdir/pairs-rs.supported.sorted.pairs"
pairtools_parse_out="$workdir/pairtools.supported.pairsam"
pairs_rs_parse_out="$workdir/pairs-rs.supported.pairsam"
pairtools_parse_norm="$workdir/pairtools.supported.normalized.pairsam"
pairs_rs_parse_norm="$workdir/pairs-rs.supported.normalized.pairsam"

run_pairtools_supported() {
  pairtools parse \
    -c "$chroms" \
    --drop-sam \
    --min-mapq "$mapq" \
    --walks-policy 5unique \
    --report-alignment-end "$report_end" \
    "$bam" \
    | pairtools sort > "$pairtools_out"
}

run_pairs_rs_supported() {
  "$pairs_rs_bin" parse \
    -c "$chroms" \
    --drop-sam \
    --min-mapq "$mapq" \
    --walks-policy 5unique \
    --report-alignment-end "$report_end" \
    "$bam" \
    | "$pairs_rs_bin" sort > "$pairs_rs_out"
}

run_pairtools_parse_only() {
  pairtools parse \
    --chroms-path "$chroms" \
    --assembly "$asm" \
    --min-mapq "$full_mapq" \
    --walks-policy 5unique \
    --max-inter-align-gap "$full_gap" \
    --report-alignment-end 5 \
    --add-columns mapq,pos5,pos3,cigar,read_len \
    "$bam" > "$pairtools_parse_out"
}

run_pairs_rs_parse_only() {
  "$pairs_rs_bin" parse \
    --chroms-path "$chroms" \
    --assembly "$asm" \
    --min-mapq "$full_mapq" \
    --walks-policy 5unique \
    --max-inter-align-gap "$full_gap" \
    --report-alignment-end 5 \
    --add-columns mapq,pos5,pos3,cigar,read_len \
    "$bam" > "$pairs_rs_parse_out"
}

normalize_parse_output() {
  local input="$1"
  local output="$2"
  sed '/^#samheader:/d' "$input" > "$output"
}

compare_outputs() {
  echo "pairtools version:"
  pairtools --version
  echo "pairs-rs binary: $pairs_rs_bin"
  echo "data dir: $data_dir"
  echo "workdir: $workdir"

  echo "Running live pairtools supported-subset oracle..."
  run_pairtools_supported

  if [[ -f "$expected" ]]; then
    echo "Comparing live pairtools output to saved expected: $expected"
    diff -u "$expected" "$pairtools_out"
  else
    echo "No saved expected sorted pairs found at: $expected"
  fi

  echo "Running pairs-rs supported-subset candidate..."
  run_pairs_rs_supported

  echo "Comparing pairs-rs output to live pairtools oracle..."
  diff -u "$pairtools_out" "$pairs_rs_out"
  echo "Exact supported-subset parity passed."

  echo "Running live pairtools parse-only pairsam oracle..."
  run_pairtools_parse_only
  normalize_parse_output "$pairtools_parse_out" "$pairtools_parse_norm"

  echo "Running pairs-rs parse-only pairsam candidate..."
  run_pairs_rs_parse_only
  normalize_parse_output "$pairs_rs_parse_out" "$pairs_rs_parse_norm"

  echo "Comparing normalized parse-only pairsam output to live pairtools oracle..."
  diff -u "$pairtools_parse_norm" "$pairs_rs_parse_norm"
  echo "Normalized parse-only pairsam parity passed."
}

full_gate() {
  local stderr="$workdir/unsupported-parse-gate.stderr"
  set +e
  "$pairs_rs_bin" parse \
    -c "$chroms" \
    --drop-sam \
    --add-columns matched_bp \
    "$bam" > /dev/null 2> "$stderr"
  local status=$?
  set -e

  if ((status == 0)); then
    echo "Expected unsupported parse option to fail loudly, but it succeeded." >&2
    exit 1
  fi
  if ! grep -q "not implemented" "$stderr"; then
    echo "Unsupported parse option failed, but not with a not-implemented gate:" >&2
    cat "$stderr" >&2
    exit 1
  fi
  echo "Unsupported parse flags are gated loudly."
}

benchmark_outputs() {
  if ! command -v hyperfine >/dev/null 2>&1; then
    echo "hyperfine is required for --benchmark" >&2
    exit 2
  fi

  local pt_runner="$workdir/run_pairtools_supported.sh"
  local rs_runner="$workdir/run_pairs_rs_supported.sh"

  cat > "$pt_runner" <<EOF
#!/usr/bin/env bash
set -euo pipefail
pairtools parse -c '$chroms' --drop-sam --min-mapq '$mapq' --walks-policy 5unique --report-alignment-end '$report_end' '$bam' | pairtools sort > '$workdir/pairtools.bench.sorted.pairs'
EOF
  cat > "$rs_runner" <<EOF
#!/usr/bin/env bash
set -euo pipefail
'$pairs_rs_bin' parse -c '$chroms' --drop-sam --min-mapq '$mapq' --walks-policy 5unique --report-alignment-end '$report_end' '$bam' | '$pairs_rs_bin' sort > '$workdir/pairs-rs.bench.sorted.pairs'
EOF
  chmod +x "$pt_runner" "$rs_runner"

  echo "Benchmarking only after exact parity has passed..."
  hyperfine \
    --runs "${BENCHMARK_RUNS:-3}" \
    --prepare "rm -f '$workdir/pairtools.bench.sorted.pairs' '$workdir/pairs-rs.bench.sorted.pairs'" \
    "$pt_runner" \
    "$rs_runner"

  echo "Verifying benchmark outputs still match..."
  diff -u "$workdir/pairtools.bench.sorted.pairs" "$workdir/pairs-rs.bench.sorted.pairs"
}

if ((run_compare)); then
  compare_outputs
fi

if ((run_full_gate)); then
  full_gate
fi

if ((run_benchmark)); then
  benchmark_outputs
fi
