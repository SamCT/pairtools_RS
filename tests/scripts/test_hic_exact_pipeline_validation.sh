#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/scripts/run_hic_exact_pairs_rs_pipeline.sh"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

make_exe() {
  local name="$1"
  cat > "$TMPROOT/bin/$name" <<'SH'
#!/usr/bin/env bash
exit 0
SH
  chmod +x "$TMPROOT/bin/$name"
}

expect_fail() {
  local name="$1" expected="$2"
  shift 2
  local log="$TMPROOT/$name.log"
  if "$@" >"$log" 2>&1; then
    echo "expected failure for $name" >&2
    cat "$log" >&2
    exit 1
  fi
  grep -F -- "$expected" "$log" >/dev/null || {
    echo "failure for $name did not mention: $expected" >&2
    cat "$log" >&2
    exit 1
  }
}

mkdir -p "$TMPROOT/bin" "$TMPROOT/out" "$TMPROOT/tmp"
for exe in pairs-rs pairtools bwa-mem2 samtools bgzip; do
  make_exe "$exe"
done

touch "$TMPROOT/H1_s3.0123" "$TMPROOT/Hop282H1.chrom.sizes"
touch "$TMPROOT/lane1_R1.fastq.gz" "$TMPROOT/lane1_R2.fastq.gz"
touch "$TMPROOT/lane2_R2.fastq.gz"

BASE_ENV=(
  "PATH=$TMPROOT/bin:$PATH"
  "THREADS=8"
  "SORT_THREADS=4"
  "MAPQ=10"
  "BWA_INDEX=$TMPROOT/H1_s3"
  "CHROMS=$TMPROOT/Hop282H1.chrom.sizes"
  "ASM=HopH1_282"
  "PREFIX=$TMPROOT/out/hic"
  "TMPDIR=$TMPROOT/tmp"
  "R1=$TMPROOT/lane1_R1.fastq.gz"
  "R2=$TMPROOT/lane1_R2.fastq.gz"
  "PAIRS_RS=pairs-rs"
  "PAIRTOOLS=pairtools"
  "BWA_MEM2=bwa-mem2"
  "SAMTOOLS=samtools"
  "BGZIP=bgzip"
  "DRY_RUN=1"
)

expect_fail missing_chroms "CHROMS is not readable" \
  env "${BASE_ENV[@]}" "CHROMS=$TMPROOT/missing.chrom.sizes" bash "$SCRIPT"

expect_fail mismatched_lanes "R1 and R2 lane counts differ" \
  env "${BASE_ENV[@]}" "R1=$TMPROOT/lane1_R1.fastq.gz,$TMPROOT/lane1_R1.fastq.gz" "R2=$TMPROOT/lane1_R2.fastq.gz" bash "$SCRIPT"

expect_fail sort_threads_zero "SORT_THREADS must be greater than zero" \
  env "${BASE_ENV[@]}" "SORT_THREADS=0" bash "$SCRIPT"

expect_fail mapq_too_large "MAPQ must be <= 255" \
  env "${BASE_ENV[@]}" "MAPQ=999" bash "$SCRIPT"

expect_fail missing_bwa_index "BWA_INDEX prefix has no matching files" \
  env "${BASE_ENV[@]}" "BWA_INDEX=$TMPROOT/missing_index" bash "$SCRIPT"
