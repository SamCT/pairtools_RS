#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

split_command() {
  local text="$1"
  local -n out_ref="$2"
  # Intentionally simple: repo-local defaults do not contain shell quoting.
  # Callers that need spaces can provide a wrapper script path via the env var.
  # shellcheck disable=SC2206
  out_ref=($text)
}

PAIRS_RS="${PAIRS_RS:-${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}/debug/pairs-rs}"
split_command "$PAIRS_RS" PAIRS_RS_CMD
if [[ -n "${PAIRTOOLS:-}" ]]; then
  split_command "$PAIRTOOLS" PAIRTOOLS_CMD
else
  PAIRTOOLS_CMD=(pixi run --manifest-path "$ROOT/pixi.toml" pairtools)
fi
if [[ -n "${BGZIP:-}" ]]; then
  split_command "$BGZIP" BGZIP_CMD
else
  BGZIP_CMD=(pixi run --manifest-path "$ROOT/pixi.toml" bgzip)
fi
if [[ -n "${GZIP:-}" ]]; then
  split_command "$GZIP" GZIP_CMD
else
  GZIP_CMD=(pixi run --manifest-path "$ROOT/pixi.toml" gzip)
fi

if [[ ! -x "${PAIRS_RS_CMD[0]}" ]]; then
  echo "missing pairs-rs binary: ${PAIRS_RS_CMD[*]}" >&2
  echo "run scripts/cargo_guard.sh build before this test, or set PAIRS_RS" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

input="$tmpdir/H1_ALL_parse_RS_1.sorted_2.pairsam"

python3 - "$input" <<'PY'
from pathlib import Path
import sys

sep = "\x19"

def sam(read_id, flag, chrom, pos, mate_chrom, mate_pos, yt):
    return sep.join(
        [
            read_id,
            str(flag),
            chrom,
            str(pos),
            "60",
            "10M",
            mate_chrom,
            str(mate_pos),
            "0",
            "AAAAAAAAAA",
            "IIIIIIIIII",
            "XS:i:0",
            f"Yt:Z:{yt}",
        ]
    )

rows = [
    ("r_unmapped", "!", "0", "chr1", "50", "-", "+", "NU", ".", ".", "0", "60"),
    (
        "r_parent",
        "chr1",
        "100",
        "chr1",
        "200",
        "+",
        "-",
        "UU",
        sam("r_parent", 65, "chr1", 100, "chr1", 200, "UU"),
        sam("r_parent", 129, "chr1", 200, "chr1", 100, "UU"),
        "60",
        "60",
    ),
    (
        "r_far",
        "chr1",
        "100",
        "chr1",
        "210",
        "+",
        "-",
        "UU",
        sam("r_far", 65, "chr1", 100, "chr1", 210, "UU"),
        sam("r_far", 129, "chr1", 210, "chr1", 100, "UU"),
        "60",
        "60",
    ),
    (
        "r_dup1",
        "chr1",
        "101",
        "chr1",
        "202",
        "+",
        "-",
        "UU",
        sam("r_dup1", 65, "chr1", 101, "chr1", 202, "UU"),
        sam("r_dup1", 129, "chr1", 202, "chr1", 101, "UU"),
        "60",
        "60",
    ),
    (
        "r_dup2",
        "chr1",
        "103",
        "chr1",
        "203",
        "+",
        "-",
        "UU",
        sam("r_dup2", 65, "chr1", 103, "chr1", 203, "UU"),
        sam("r_dup2", 129, "chr1", 203, "chr1", 103, "UU"),
        "60",
        "60",
    ),
    (
        "r_unique",
        "chr1",
        "200",
        "chr2",
        "10",
        "+",
        "+",
        "UU",
        sam("r_unique", 65, "chr1", 200, "chr2", 10, "UU"),
        sam("r_unique", 129, "chr2", 10, "chr1", 200, "UU"),
        "60",
        "60",
    ),
]

text = """## pairs format v1.0.0
#sorted: chr1-chr2-pos1-pos2
#shape: upper triangle
#genome_assembly: dedup_pipeline_test
#chromosomes: chr1 chr2
#chromsize: chr1 1000
#chromsize: chr2 1000
#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type sam1 sam2 mapq1 mapq2
"""
text += "\n".join("\t".join(row) for row in rows) + "\n"
Path(sys.argv[1]).write_text(text, encoding="utf-8")
PY

(
  cd "$tmpdir"
  "${PAIRS_RS_CMD[@]}" dedup \
    --mark-dups \
    --output-stats merged.dedup.s01.RS.stats.txt \
    --output-dups merged.dups.pairsam.s01.RS.gz \
    --output-unmapped merged.unmapped.pairsam.s01.RS.gz \
    -o nodups.parse_RS_s01.sorted.pairsam \
    H1_ALL_parse_RS_1.sorted_2.pairsam

  "${PAIRTOOLS_CMD[@]}" dedup \
    --mark-dups \
    --output-stats oracle.dedup.stats.txt \
    --output-dups oracle.dups.pairsam \
    --output-unmapped oracle.unmapped.pairsam \
    -o oracle.nodups.pairsam \
    H1_ALL_parse_RS_1.sorted_2.pairsam
)

validate_gz() {
  local path="$1"
  if "${BGZIP_CMD[@]}" -t "$path" >/dev/null 2>&1; then
    return 0
  fi
  "${GZIP_CMD[@]}" -t "$path"
}

validate_gz "$tmpdir/merged.dups.pairsam.s01.RS.gz"
validate_gz "$tmpdir/merged.unmapped.pairsam.s01.RS.gz"

python3 - "$tmpdir" <<'PY'
from pathlib import Path
import gzip
import sys

root = Path(sys.argv[1])

def read_text(name):
    path = root / name
    if name.endswith(".gz"):
        with gzip.open(path, "rt", encoding="utf-8") as handle:
            return handle.read()
    return path.read_text(encoding="utf-8")

def columns_and_rows(name):
    text = read_text(name)
    columns = None
    rows = []
    for line in text.splitlines():
        if line.startswith("#columns:"):
            columns = line.split(":", 1)[1].split()
        elif line and not line.startswith("#"):
            rows.append(line.split("\t"))
    if columns is None:
        raise AssertionError(f"{name} is missing #columns")
    return columns, rows

def read_ids(name):
    _, rows = columns_and_rows(name)
    return [row[0] for row in rows]

nodups_path = root / "nodups.parse_RS_s01.sorted.pairsam"
if not nodups_path.exists():
    raise AssertionError("nodups output was not created")
if not read_ids("nodups.parse_RS_s01.sorted.pairsam"):
    raise AssertionError("nodups output has no non-header rows")

stats = {}
for line in read_text("merged.dedup.s01.RS.stats.txt").splitlines():
    if line.strip():
        key, value = line.split("\t", 1)
        stats[key] = value
for key in [
    "total",
    "total_mapped",
    "total_unmapped",
    "total_dups",
    "total_nodups",
    "fraction_dups",
]:
    if key not in stats:
        raise AssertionError(f"stats output missing {key}")

columns, dup_rows = columns_and_rows("merged.dups.pairsam.s01.RS.gz")
idx = {name: i for i, name in enumerate(columns)}
for required in ["readID", "pair_type", "sam1", "sam2"]:
    if required not in idx:
        raise AssertionError(f"duplicate output missing {required} column")
for row in dup_rows:
    if row[idx["pair_type"]] != "DD":
        raise AssertionError(f"duplicate row {row[0]} was not marked DD")
    for sam_col in ["sam1", "sam2"]:
        sam = row[idx[sam_col]]
        if sam == ".":
            continue
        fields = sam.split("\x19")
        if len(fields) < 2:
            raise AssertionError(f"{row[0]} {sam_col} is not a pairsam SAM field")
        if not (int(fields[1]) & 0x400):
            raise AssertionError(f"{row[0]} {sam_col} missing duplicate flag 0x400")
        if "Yt:Z:DD" not in fields:
            raise AssertionError(f"{row[0]} {sam_col} missing Yt:Z:DD")

comparisons = [
    ("nodups.parse_RS_s01.sorted.pairsam", "oracle.nodups.pairsam"),
    ("merged.dups.pairsam.s01.RS.gz", "oracle.dups.pairsam"),
    ("merged.unmapped.pairsam.s01.RS.gz", "oracle.unmapped.pairsam"),
]
for candidate, oracle in comparisons:
    left = read_ids(candidate)
    right = read_ids(oracle)
    if left != right:
        raise AssertionError(f"readID routing differs for {candidate}: {left} != {right}")
PY

echo "dedup production command shape validation passed"
