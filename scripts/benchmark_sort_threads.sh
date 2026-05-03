#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Benchmark pairs-rs sort threading on parse-generated .pairsam input.

Required input, choose one:
  --pairsam PATH              Existing parse-generated .pairsam
  --sam PATH --chroms PATH    SAM/BAM/CRAM to parse before benchmarking

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
        --require-speedup)
            require_speedup=1
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

if [[ -z "$pairsam" ]]; then
    if [[ -z "$sam" || -z "$chroms" ]]; then
        echo "provide either --pairsam or both --sam and --chroms" >&2
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

run_sort() {
    local nproc="$1"
    local tmp="$workdir/tmp-nproc-$nproc"
    local out="$workdir/sorted.nproc$nproc.pairsam.gz"
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
    local output_bytes
    output_bytes="$(stat -c%s "$out")"

    printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\n" \
        "$nproc" "$wall_seconds" "$cpu_percent" "$max_rss_kb" \
        "$max_tmp_bytes" "$output_bytes" "$out"
}

results="$workdir/sort-thread-benchmark.tsv"
printf "nproc\twall_seconds\tcpu_utilization\tmax_rss_kb\tmax_temp_disk_bytes\toutput_bytes\toutput\n" > "$results"
run_sort 1 >> "$results"
run_sort 8 >> "$results"

gzip -cd "$workdir/sorted.nproc1.pairsam.gz" > "$workdir/sorted.nproc1.pairsam"
gzip -cd "$workdir/sorted.nproc8.pairsam.gz" > "$workdir/sorted.nproc8.pairsam"
cmp "$workdir/sorted.nproc1.pairsam" "$workdir/sorted.nproc8.pairsam"

wall1="$(awk -F'\t' '$1 == "1" { print $2 }' "$results")"
wall8="$(awk -F'\t' '$1 == "8" { print $2 }' "$results")"
faster="$(python - "$wall1" "$wall8" <<'PY'
import sys
wall1 = float(sys.argv[1])
wall8 = float(sys.argv[2])
print("yes" if wall8 < wall1 else "no")
PY
)"

cat "$results"
printf "nproc8_faster_than_nproc1\t%s\n" "$faster"
printf "workdir\t%s\n" "$workdir"

if [[ "$require_speedup" -eq 1 && "$faster" != "yes" ]]; then
    exit 1
fi
