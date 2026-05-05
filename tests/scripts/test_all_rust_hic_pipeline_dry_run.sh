#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/scripts/run_hic_all_rust_pairs_rs_pipeline.sh"
TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

make_exe() {
  local name="$1"
  cat > "$TMPROOT/bin/$name" <<'SH'
#!/usr/bin/env bash
echo "dummy $0 $*" >&2
SH
  chmod +x "$TMPROOT/bin/$name"
}

assert_contains() {
  local haystack="$1" needle="$2"
  grep -F -- "$needle" "$haystack" >/dev/null || {
    echo "missing expected text: $needle" >&2
    cat "$haystack" >&2
    exit 1
  }
}

assert_not_contains() {
  local haystack="$1" needle="$2"
  if grep -F -- "$needle" "$haystack" >/dev/null; then
    echo "unexpected text: $needle" >&2
    cat "$haystack" >&2
    exit 1
  fi
}

mkdir -p "$TMPROOT/bin" "$TMPROOT/out" "$TMPROOT/tmp"
for exe in pairs-rs bwa-mem2 samtools bgzip; do
  make_exe "$exe"
done

touch "$TMPROOT/H1_s3.0123" "$TMPROOT/Hop282H1.chrom.sizes"
touch "$TMPROOT/lane1_R1.fastq.gz" "$TMPROOT/lane1_R2.fastq.gz"
touch "$TMPROOT/lane2_R1.fastq.gz" "$TMPROOT/lane2_R2.fastq.gz"

COMMON_ENV=(
  "PATH=$TMPROOT/bin:$PATH"
  "THREADS=8"
  "SORT_THREADS=4"
  "MAPQ=10"
  "BWA_INDEX=$TMPROOT/H1_s3"
  "CHROMS=$TMPROOT/Hop282H1.chrom.sizes"
  "ASM=HopH1_282"
  "TMPDIR=$TMPROOT/tmp"
  "PAIRS_RS=pairs-rs"
  "BWA_MEM2=bwa-mem2"
  "SAMTOOLS=samtools"
  "BGZIP=bgzip"
  "DRY_RUN=1"
)

one_lane_plan="$TMPROOT/one-lane.plan"
env "${COMMON_ENV[@]}" \
  "PREFIX=$TMPROOT/out/hic" \
  "R1=$TMPROOT/lane1_R1.fastq.gz" \
  "R2=$TMPROOT/lane1_R2.fastq.gz" \
  bash "$SCRIPT" >"$one_lane_plan" 2>&1

assert_contains "$one_lane_plan" "bwa-mem2 mem"
assert_contains "$one_lane_plan" "pairs-rs parse"
assert_contains "$one_lane_plan" "pairs-rs sort"
assert_contains "$one_lane_plan" "$TMPROOT/out/hic.sorted.pairsam.gz"
assert_contains "$one_lane_plan" "$TMPROOT/out/hic.parse.stats.txt"
assert_contains "$one_lane_plan" "ln -s"
assert_contains "$one_lane_plan" "merged.sorted.pairsam.gz"
assert_not_contains "$one_lane_plan" "pairs-rs merge"
assert_contains "$one_lane_plan" "pairs-rs dedup"
assert_contains "$one_lane_plan" "--mark-dups"
assert_contains "$one_lane_plan" "merged.nodups.pairsam.gz"
assert_contains "$one_lane_plan" "merged.dups.pairsam.gz"
assert_contains "$one_lane_plan" "merged.unmapped.pairsam.gz"
assert_contains "$one_lane_plan" "merged.dedup.stats.txt"
assert_contains "$one_lane_plan" "pairs-rs select"
assert_contains "$one_lane_plan" "pair_type"
assert_contains "$one_lane_plan" "UU"
assert_contains "$one_lane_plan" "merged.valid.pairsam.gz"
assert_contains "$one_lane_plan" "pairs-rs split"
assert_contains "$one_lane_plan" "merged.valid.pairs.gz"
assert_contains "$one_lane_plan" "samtools view -@ 4 -b -"
assert_contains "$one_lane_plan" "samtools sort -@ 4 -o merged.valid.coord.bam -"
assert_contains "$one_lane_plan" "samtools index merged.valid.coord.bam"
assert_contains "$one_lane_plan" "samtools quickcheck merged.valid.coord.bam"
assert_contains "$one_lane_plan" "pairs-rs stats"
assert_contains "$one_lane_plan" "--with-chromsizes"
assert_contains "$one_lane_plan" "merged.valid.stats.txt"
assert_not_contains "$one_lane_plan" "pairtools"

two_lane_plan="$TMPROOT/two-lane.plan"
env "${COMMON_ENV[@]}" \
  "PREFIX=$TMPROOT/out/hic" \
  "R1=$TMPROOT/lane1_R1.fastq.gz,$TMPROOT/lane2_R1.fastq.gz" \
  "R2=$TMPROOT/lane1_R2.fastq.gz,$TMPROOT/lane2_R2.fastq.gz" \
  bash "$SCRIPT" >"$two_lane_plan" 2>&1

assert_contains "$two_lane_plan" "$TMPROOT/out/hic.lane01.sorted.pairsam.gz"
assert_contains "$two_lane_plan" "$TMPROOT/out/hic.lane01.parse.stats.txt"
assert_contains "$two_lane_plan" "$TMPROOT/out/hic.lane02.sorted.pairsam.gz"
assert_contains "$two_lane_plan" "$TMPROOT/out/hic.lane02.parse.stats.txt"
assert_contains "$two_lane_plan" "pairs-rs merge"
assert_contains "$two_lane_plan" "-o merged.sorted.pairsam.gz"
assert_contains "$two_lane_plan" "merged.nodups.pairsam.gz"
assert_contains "$two_lane_plan" "merged.valid.pairs.gz"
assert_contains "$two_lane_plan" "merged.valid.coord.bam"
assert_contains "$two_lane_plan" "merged.valid.stats.txt"
assert_not_contains "$two_lane_plan" "pairtools"

echo "all-Rust Hi-C pipeline dry-run validation passed"
