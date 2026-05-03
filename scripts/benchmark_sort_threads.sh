#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Benchmark pairs-rs sort threading on parse-generated or compression-heavy .pairsam input.

Required input for default mode, choose one:
  --pairsam PATH              Existing parse-generated .pairsam
  --sam PATH --chroms PATH    SAM/BAM/CRAM to parse before benchmarking

Compression-dominates mode:
  --compression-dominates     Generate presorted wide .pairsam so output compression dominates
  --compression-rows N        Rows for compression-dominates mode (default: 50000)
  --payload-bytes N           Extra payload bytes per side per row (default: 2048)
  --require-parallel-compression
                              Exit non-zero unless nproc=8 reports >100% CPU utilization

Options:
  --workdir PATH              Working directory for generated files
  --bin PATH                  pairs-rs binary to benchmark
  --asm NAME                  Assembly passed to parse (default: unknown)
  --mapq INT                  --min-mapq passed to parse (default: 1)
  --require-speedup           Exit non-zero unless nproc=8 is faster than nproc=1

The script never builds Rust. Set PAIRS_RS_BIN or pass --bin to choose a binary.
USAGE
}

pairsam=""
sam=""
chroms=""
workdir=""
bin="${PAIRS_RS_BIN:-target/release/pairs-rs}"
asm="${ASM:-unknown}"
mapq="${MAPQ:-1}"
require_speedup=0
compression_dominates=0
compression_rows=50000
payload_bytes=2048
require_parallel_compression=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --pairsam)
            pairsam="$2"
            shift 2
            ;;
        --sam)
            sam="$2"
            shift 2
            ;;
        --chroms|--chroms-path)
            chroms="$2"
            shift 2
            ;;
        --workdir)
            workdir="$2"
            shift 2
            ;;
        --bin)
            bin="$2"
            shift 2
            ;;
        --asm|--assembly)
            asm="$2"
            shift 2
            ;;
        --mapq|--min-mapq)
            mapq="$2"
            shift 2
            ;;
        --compression-dominates)
            compression_dominates=1
            shift
            ;;
        --compression-rows)
            compression_rows="$2"
            shift 2
            ;;
        --payload-bytes)
            payload_bytes="$2"
            shift 2
            ;;
        --require-speedup)
            require_speedup=1
            shift
            ;;
        --require-parallel-compression)
            require_parallel_compression=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -z "$workdir" ]]; then
    workdir="$(mktemp -d "${TMPDIR:-/tmp}/pairs-rs-sort-bench.XXXXXX")"
else
    mkdir -p "$workdir"
fi

if [[ ! -x "$bin" ]]; then
    if command -v "$bin" >/dev/null 2>&1; then
        bin="$(command -v "$bin")"
    else
        echo "pairs-rs binary is not executable: $bin" >&2
        echo "Pass --bin or set PAIRS_RS_BIN. This benchmark does not build Rust." >&2
        exit 2
    fi
fi

make_compression_dominates_pairsam() {
    local out="$1"
    python - "$out" "$compression_rows" "$payload_bytes" <<'PY'
import hashlib
import sys
from pathlib import Path

out = Path(sys.argv[1])
rows = int(sys.argv[2])
payload_bytes = int(sys.argv[3])

def payload_for(idx: int, side: int) -> str:
    chunks = []
    counter = 0
    while sum(len(chunk) for chunk in chunks) < payload_bytes:
        chunks.append(hashlib.sha256(f"{idx}:{side}:{counter}".encode()).hexdigest())
        counter += 1
    return "".join(chunks)[:payload_bytes]

with out.open("w", encoding="utf-8") as f:
    f.write("## pairs format v1.0.0\n")
    f.write("#sorted: chr1-chr2-pos1-pos2\n")
    f.write("#chromsize: chr1 1000000\n")
    f.write("#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type sam1 sam2 payload1 payload2\n")
    for idx in range(rows):
        payload1 = payload_for(idx, 1)
        payload2 = payload_for(idx, 2)
        # Already sorted by chrom1, chrom2, pos1, pos2, pair_type; wide payload makes compression dominate.
        f.write(
            f"r{idx:09d}\tchr1\t1\tchr1\t2\t+\t-\tUU\t"
            f"sam{idx}:1:60:100M\t"
            f"sam{idx}:2:60:100M\t"
            f"{payload1}\t{payload2}\n"
        )
PY
}

if [[ "$compression_dominates" -eq 1 ]]; then
    pairsam="$workdir/compression-dominates.pairsam"
    make_compression_dominates_pairsam "$pairsam"
elif [[ -z "$pairsam" ]]; then
    if [[ -z "$sam" || -z "$chroms" ]]; then
        echo "provide either --pairsam, --compression-dominates, or both --sam and --chroms" >&2
        usage >&2
        exit 2
    fi
    pairsam="$workdir/parse-generated.pairsam"
    "$bin" parse \
        --chroms-path "$chroms" \
        --assembly "$asm" \
        --min-mapq "$mapq" \
        --walks-policy 5unique \
        --max-inter-align-gap 30 \
        --report-alignment-end 5 \
        --add-columns mapq,pos5,pos3,cigar,read_len \
        --output-stats "$workdir/parse.stats.txt" \
        "$sam" > "$pairsam"
fi

if [[ ! -r "$pairsam" ]]; then
    echo "pairsam input is not readable: $pairsam" >&2
    exit 2
fi

metric_value() {
    local file="$1"
    local pattern="$2"
    awk -F': ' -v pat="$pattern" '$0 ~ pat { print $2; exit }' "$file"
}

cpu_percent_number() {
    printf "%s" "$1" | tr -d '%' | awk '{ print $1 + 0 }'
}

throughput_mb_s() {
    python - "$1" "$2" <<'PY'
import sys
bytes_uncompressed = int(sys.argv[1])
seconds = float(sys.argv[2])
print(f"{bytes_uncompressed / seconds / 1_000_000:.6f}" if seconds > 0 else "inf")
PY
}

run_sort() {
    local nproc="$1"
    local tmp="$workdir/tmp-nproc-$nproc"
    local out="$workdir/sorted.nproc$nproc.pairsam.gz"
    local plain="$workdir/sorted.nproc$nproc.pairsam"
    local time_log="$workdir/time.nproc$nproc.txt"
    local temp_log="$workdir/temp-usage.nproc$nproc.txt"

    rm -rf "$tmp"
    mkdir -p "$tmp"
    : > "$temp_log"

    local start_ns
    start_ns="$(date +%s%N)"
    /usr/bin/time -v -o "$time_log" \
        "$bin" sort --nproc "$nproc" --tmpdir "$tmp" -o "$out" "$pairsam" &
    local sort_pid=$!

    while kill -0 "$sort_pid" 2>/dev/null; do
        du -sb "$tmp" 2>/dev/null | awk '{print $1}' >> "$temp_log" || true
        sleep 0.2
    done
    wait "$sort_pid"

    local end_ns
    end_ns="$(date +%s%N)"
    du -sb "$tmp" 2>/dev/null | awk '{print $1}' >> "$temp_log" || true
    gzip -cd "$out" > "$plain"
    bgzip -t "$out"

    local wall_seconds
    wall_seconds="$(python - "$start_ns" "$end_ns" <<'PY'
import sys
start = int(sys.argv[1])
end = int(sys.argv[2])
print(f"{(end - start) / 1_000_000_000:.6f}")
PY
)"
    local cpu_percent
    cpu_percent="$(metric_value "$time_log" 'Percent of CPU')"
    local max_rss_kb
    max_rss_kb="$(metric_value "$time_log" 'Maximum resident set size')"
    local max_tmp_bytes
    max_tmp_bytes="$(awk 'max < $1 { max = $1 } END { print max + 0 }' "$temp_log")"
    local compressed_bytes
    compressed_bytes="$(stat -c%s "$out")"
    local uncompressed_bytes
    uncompressed_bytes="$(stat -c%s "$plain")"
    local throughput
    throughput="$(throughput_mb_s "$uncompressed_bytes" "$wall_seconds")"

    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
        "$nproc" "$wall_seconds" "$cpu_percent" "$max_rss_kb" \
        "$max_tmp_bytes" "$compressed_bytes" "$uncompressed_bytes" \
        "$throughput" "$out"
}

results="$workdir/sort-thread-benchmark.tsv"
printf "nproc\twall_seconds\tcpu_utilization\tmax_rss_kb\tmax_temp_disk_bytes\tcompressed_output_bytes\tuncompressed_output_bytes\tcompression_throughput_mb_s\toutput\n" > "$results"
run_sort 1 >> "$results"
run_sort 8 >> "$results"

cmp "$workdir/sorted.nproc1.pairsam" "$workdir/sorted.nproc8.pairsam"

wall1="$(awk -F'\t' '$1 == "1" { print $2 }' "$results")"
wall8="$(awk -F'\t' '$1 == "8" { print $2 }' "$results")"
cpu8="$(awk -F'\t' '$1 == "8" { print $3 }' "$results")"
cpu8_number="$(cpu_percent_number "$cpu8")"
faster="$(python - "$wall1" "$wall8" <<'PY'
import sys
wall1 = float(sys.argv[1])
wall8 = float(sys.argv[2])
print("yes" if wall8 < wall1 else "no")
PY
)"
parallel_compression="$(python - "$cpu8_number" <<'PY'
import sys
cpu = float(sys.argv[1])
print("yes" if cpu > 100.0 else "no")
PY
)"

cat "$results"
printf "nproc8_faster_than_nproc1\t%s\n" "$faster"
printf "nproc8_cpu_gt_100_percent\t%s\n" "$parallel_compression"
printf "input_uncompressed_bytes\t%s\n" "$(stat -c%s "$pairsam")"
printf "mode\t%s\n" "$([[ "$compression_dominates" -eq 1 ]] && echo compression-dominates || echo parse-generated)"
printf "workdir\t%s\n" "$workdir"

if [[ "$require_speedup" -eq 1 && "$faster" != "yes" ]]; then
    exit 1
fi
if [[ "$require_parallel_compression" -eq 1 && "$parallel_compression" != "yes" ]]; then
    exit 1
fi
