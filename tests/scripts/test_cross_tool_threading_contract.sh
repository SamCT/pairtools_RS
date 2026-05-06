#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

split_command() {
  local text="$1"
  local -n out_ref="$2"
  # Repo-local defaults do not contain shell quoting. Use a wrapper path if needed.
  # shellcheck disable=SC2206
  out_ref=($text)
}

PAIRS_RS="${PAIRS_RS:-${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}/debug/pairs-rs}"
split_command "$PAIRS_RS" PAIRS_RS_CMD
if [[ ! -x "${PAIRS_RS_CMD[0]}" ]]; then
  echo "missing pairs-rs binary: ${PAIRS_RS_CMD[*]}" >&2
  echo "run scripts/cargo_guard.sh build before this test, or set PAIRS_RS" >&2
  exit 1
fi

TMPROOT="$(mktemp -d)"
trap 'rm -rf "$TMPROOT"' EXIT

assert_contains() {
  local path="$1" needle="$2"
  grep -F -- "$needle" "$path" >/dev/null || {
    echo "missing expected text: $needle" >&2
    cat "$path" >&2
    exit 1
  }
}

assert_not_contains() {
  local path="$1" needle="$2"
  if grep -F -- "$needle" "$path" >/dev/null; then
    echo "unexpected text: $needle" >&2
    cat "$path" >&2
    exit 1
  fi
}

expect_fail_contains() {
  local needle="$1"
  shift
  local output="$TMPROOT/expect-fail.$RANDOM.txt"
  set +e
  "$@" >"$output" 2>&1
  local status=$?
  set -e
  if (( status == 0 )); then
    echo "command unexpectedly succeeded: $*" >&2
    cat "$output" >&2
    exit 1
  fi
  assert_contains "$output" "$needle"
}

decompress_gzip_text() {
  local input="$1" output="$2"
  python3 - "$input" "$output" <<'PY'
import gzip
import sys

source, dest = sys.argv[1:3]
with gzip.open(source, "rt", encoding="utf-8", errors="replace") as handle, open(dest, "w", encoding="utf-8") as out:
    for line in handle:
        out.write(line)
PY
}

make_dummy_exe() {
  local name="$1"
  cat >"$TMPROOT/bin/$name" <<'SH'
#!/usr/bin/env bash
echo "dummy $0 $*" >&2
SH
  chmod +x "$TMPROOT/bin/$name"
}

fixture="$TMPROOT/threading.unsorted.pairsam"
chroms="$TMPROOT/threading.chrom.sizes"
sam="$TMPROOT/threading.sam"

cat >"$chroms" <<'EOF_CHROMS'
chr1	1000000
chr2	1000000
EOF_CHROMS

cat >"$sam" <<'EOF_SAM'
@HD	VN:1.6	SO:queryname
@SQ	SN:chr1	LN:1000000
r001	65	chr1	100	60	10M	=	200	0	AAAAAAAAAA	IIIIIIIIII
r001	129	chr1	200	60	10M	=	100	0	TTTTTTTTTT	IIIIIIIIII
EOF_SAM

python3 - "$fixture" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
rows = []
for i in range(12000):
    # Reverse-ish order plus repeated keys: enough rows to exercise sort chunking
    # and stable equal-key handling without making the shell test large.
    chrom1 = "chr2" if i % 7 == 0 else "chr1"
    chrom2 = "chr2" if i % 5 == 0 else "chr1"
    pos1 = 50000 - (i % 5000)
    pos2 = 70000 - ((i * 3) % 7000)
    if i % 11 == 0:
        pos1 = 12345
        pos2 = 54321
    rows.append(
        [
            f"read{i:05d}",
            chrom1,
            str(pos1),
            chrom2,
            str(pos2),
            "+",
            "-",
            "UU",
            "60",
            "60",
        ]
    )

text = """## pairs format v1.0.0
#shape: upper triangle
#genome_assembly: threading_test
#chromosomes: chr1 chr2
#chromsize: chr1 1000000
#chromsize: chr2 1000000
#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type mapq1 mapq2
"""
text += "\n".join("\t".join(row) for row in rows) + "\n"
path.write_text(text, encoding="utf-8")
PY

mkdir -p "$TMPROOT/sort1" "$TMPROOT/sort4"
"${PAIRS_RS_CMD[@]}" sort --nproc 1 --tmpdir "$TMPROOT/sort1" -o "$TMPROOT/sorted.nproc1.pairsam.gz" "$fixture"
"${PAIRS_RS_CMD[@]}" sort --nproc 4 --tmpdir "$TMPROOT/sort4" -o "$TMPROOT/sorted.nproc4.pairsam.gz" "$fixture"

decompress_gzip_text "$TMPROOT/sorted.nproc1.pairsam.gz" "$TMPROOT/sorted.nproc1.pairsam"
decompress_gzip_text "$TMPROOT/sorted.nproc4.pairsam.gz" "$TMPROOT/sorted.nproc4.pairsam"
cmp "$TMPROOT/sorted.nproc1.pairsam" "$TMPROOT/sorted.nproc4.pairsam"

"${PAIRS_RS_CMD[@]}" stats --nproc-in 1 --nproc-out 1 -o "$TMPROOT/stats.nproc1.txt.gz" "$TMPROOT/sorted.nproc1.pairsam.gz"
"${PAIRS_RS_CMD[@]}" stats --nproc-in 4 --nproc-out 4 -o "$TMPROOT/stats.nproc4.txt.gz" "$TMPROOT/sorted.nproc1.pairsam.gz"
decompress_gzip_text "$TMPROOT/stats.nproc1.txt.gz" "$TMPROOT/stats.nproc1.txt"
decompress_gzip_text "$TMPROOT/stats.nproc4.txt.gz" "$TMPROOT/stats.nproc4.txt"
cmp "$TMPROOT/stats.nproc1.txt" "$TMPROOT/stats.nproc4.txt"

expect_fail_contains "pairtools sort --nproc must be greater than zero" \
  "${PAIRS_RS_CMD[@]}" sort --nproc 0 "$fixture"
expect_fail_contains "pairtools stats --nproc-in must be greater than zero" \
  "${PAIRS_RS_CMD[@]}" stats --nproc-in 0 "$TMPROOT/sorted.nproc1.pairsam.gz"
expect_fail_contains "not implemented: pairtools parse --nproc-in" \
  "${PAIRS_RS_CMD[@]}" parse --nproc-in 2 --chroms-path "$chroms" "$sam"
expect_fail_contains "not implemented: pairtools select --nproc-in" \
  "${PAIRS_RS_CMD[@]}" select --nproc-in 2 '(pair_type == "UU")' "$TMPROOT/sorted.nproc1.pairsam.gz"
expect_fail_contains "not implemented: pairtools merge --nproc" \
  "${PAIRS_RS_CMD[@]}" merge --nproc 2 "$TMPROOT/sorted.nproc1.pairsam.gz" "$TMPROOT/sorted.nproc4.pairsam.gz"
expect_fail_contains "not implemented: pairtools dedup --nproc-in" \
  "${PAIRS_RS_CMD[@]}" dedup --nproc-in 2 "$TMPROOT/sorted.nproc1.pairsam.gz"
expect_fail_contains "not implemented: pairtools split --nproc-out" \
  "${PAIRS_RS_CMD[@]}" split --nproc-out 2 --output-pairs "$TMPROOT/split.pairs" --output-sam "$TMPROOT/split.sam" "$TMPROOT/sorted.nproc1.pairsam.gz"

mkdir -p "$TMPROOT/bin" "$TMPROOT/pipeline" "$TMPROOT/pipeline/tmp"
for exe in pairs-rs bwa-mem2 samtools bgzip; do
  make_dummy_exe "$exe"
done
touch "$TMPROOT/pipeline/H1_s3.0123" "$TMPROOT/pipeline/Hop282H1.chrom.sizes"
touch "$TMPROOT/pipeline/lane1_R1.fastq.gz" "$TMPROOT/pipeline/lane1_R2.fastq.gz"

plan="$TMPROOT/all-rust-thread-plan.txt"
env \
  "PATH=$TMPROOT/bin:$PATH" \
  "THREADS=8" \
  "SORT_THREADS=6" \
  "MAPQ=10" \
  "BWA_INDEX=$TMPROOT/pipeline/H1_s3" \
  "CHROMS=$TMPROOT/pipeline/Hop282H1.chrom.sizes" \
  "ASM=threading_test" \
  "PREFIX=$TMPROOT/pipeline/hic" \
  "TMPDIR=$TMPROOT/pipeline/tmp" \
  "R1=$TMPROOT/pipeline/lane1_R1.fastq.gz" \
  "R2=$TMPROOT/pipeline/lane1_R2.fastq.gz" \
  "PAIRS_RS=pairs-rs" \
  "BWA_MEM2=bwa-mem2" \
  "SAMTOOLS=samtools" \
  "BGZIP=bgzip" \
  "DRY_RUN=1" \
  bash scripts/run_hic_all_rust_pairs_rs_pipeline.sh >"$plan" 2>&1

assert_contains "$plan" "bwa-mem2 mem -5SPM -T 30 -t 8"
assert_contains "$plan" "pairs-rs sort --nproc 6"
assert_contains "$plan" "pairs-rs dedup"
assert_contains "$plan" "pairs-rs select"
assert_contains "$plan" "pairs-rs split"
assert_contains "$plan" "pairs-rs stats"
assert_contains "$plan" "samtools view -@ 6 -b -"
assert_contains "$plan" "samtools sort -@ 6 -o merged.valid.coord.bam -"
assert_not_contains "$plan" "pairtools"

echo "cross-tool threading contract validation passed"
