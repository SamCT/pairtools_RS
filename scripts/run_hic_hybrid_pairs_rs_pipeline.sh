#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Run a hybrid Hi-C pipeline where pairs-rs accelerates parse+sort only.

Configuration environment variables:
  THREADS       Threads for bwa-mem2 and samtools. Default: 16
  SORT_THREADS  Threads for pairs-rs sort and pairtools compression/decompression.
                Default: THREADS
  MAPQ          Minimum MAPQ for pairs-rs parse. Default: 10
  REF           Reference FASTA, validated before running.
  BWA_INDEX     bwa-mem2 index prefix. Defaults to REF when unset.
  CHROMS        Chrom sizes file.
  ASM           Assembly name for pairs-rs parse headers.
  PREFIX        Per-lane output prefix. Single-lane sort writes PREFIX.sorted.pairsam.gz.
  TMPDIR        Temporary directory for sort/merge/samtools.
  R1            R1 FASTQ path, or comma-separated paths for multiple lanes.
  R2            R2 FASTQ path, or comma-separated paths for multiple lanes.
  PAIRS_RS      pairs-rs command or path. Default: pairs-rs
  PAIRTOOLS     pairtools command or wrapper. Default: pairtools

Optional environment variables:
  BWA_MEM2      bwa-mem2 command or wrapper. Default: bwa-mem2
  SAMTOOLS      samtools command or wrapper. Default: samtools
  BGZIP         bgzip command or wrapper. Default: bgzip
  DRY_RUN       Set to 1 to print commands without executing them.

Commands may be wrappers such as "pixi run pairtools".

Examples:
  DRY_RUN=1 THREADS=32 SORT_THREADS=16 MAPQ=10 REF=ref.fa BWA_INDEX=ref.fa \
    CHROMS=genome.chrom.sizes ASM=asm PREFIX=sample TMPDIR=/scratch/tmp \
    R1=lane1_R1.fastq.gz R2=lane1_R2.fastq.gz \
    PAIRS_RS=./target/release/pairs-rs PAIRTOOLS="pixi run pairtools" \
    scripts/run_hic_hybrid_pairs_rs_pipeline.sh

Options:
  --dry-run     Same as DRY_RUN=1.
  -h, --help    Show this help.
USAGE
}

die() {
    echo "error: $*" >&2
    exit 2
}

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >&2
}

is_dry_run() {
    [[ "${DRY_RUN:-0}" == "1" || "${DRY_RUN:-0}" == "true" || "${DRY_RUN:-0}" == "yes" ]]
}

quote_cmd() {
    local out="" q=""
    for arg in "$@"; do
        printf -v q "%q" "$arg"
        out+="${q} "
    done
    printf "%s" "${out% }"
}

run_cmd() {
    log "+ $(quote_cmd "$@")"
    if ! is_dry_run; then
        "$@"
    fi
}

require_readable_file() {
    local path="$1"
    local name="$2"
    [[ -n "$path" ]] || die "$name is empty"
    [[ -r "$path" ]] || die "$name is not readable: $path"
}

require_command() {
    local cmd="$1"
    local name="$2"
    [[ -n "$cmd" ]] || die "$name command is empty"
    if [[ "$cmd" == */* ]]; then
        [[ -x "$cmd" ]] || die "$name is not executable: $cmd"
    else
        command -v "$cmd" >/dev/null 2>&1 || die "$name is not on PATH: $cmd"
    fi
}

validate_bwa_index() {
    local prefix="$1"
    [[ -n "$prefix" ]] || die "BWA_INDEX is empty"
    if [[ -e "$prefix" ]]; then
        return
    fi
    if compgen -G "${prefix}*" >/dev/null; then
        return
    fi
    die "BWA_INDEX prefix has no matching files: $prefix"
}

ensure_dir() {
    local dir="$1"
    [[ -n "$dir" ]] || die "empty directory path"
    if is_dry_run; then
        log "+ mkdir -p $(quote_cmd "$dir")"
    else
        mkdir -p "$dir"
    fi
}

validate_bgzip_file() {
    local path="$1"
    if ! is_dry_run; then
        [[ -s "$path" ]] || die "expected non-empty gzip output: $path"
    fi
    run_cmd "${BGZIP_CMD[@]}" -t "$path"
}

split_csv_paths() {
    local value="$1"
    local -n out_ref="$2"
    IFS=',' read -r -a out_ref <<< "$value"
    for i in "${!out_ref[@]}"; do
        out_ref[$i]="${out_ref[$i]#"${out_ref[$i]%%[![:space:]]*}"}"
        out_ref[$i]="${out_ref[$i]%"${out_ref[$i]##*[![:space:]]}"}"
    done
}

lane_prefix() {
    local lane_count="$1"
    local lane_index="$2"
    if [[ "$lane_count" -eq 1 ]]; then
        printf "%s" "$PREFIX"
    else
        printf "%s.lane%02d" "$PREFIX" "$lane_index"
    fi
}

run_parse_sort_lane() {
    local lane_index="$1"
    local lane_count="$2"
    local r1="$3"
    local r2="$4"
    local prefix sorted stats

    prefix="$(lane_prefix "$lane_count" "$lane_index")"
    sorted="${prefix}.sorted.pairsam.gz"
    if [[ "$lane_count" -eq 1 ]]; then
        stats="${PREFIX}.parse.stats.txt"
    else
        stats="${prefix}.parse.stats.txt"
    fi

    log "Lane ${lane_index}/${lane_count}: bwa-mem2 mem | pairs-rs parse | pairs-rs sort"
    if is_dry_run; then
        log "+ $(quote_cmd "${BWA_MEM2_CMD[@]}" mem -5SPM -T 30 -t "$THREADS" "$BWA_INDEX" "$r1" "$r2") | \\"
        log "  $(quote_cmd "${PAIRS_RS_CMD[@]}" parse --chroms-path "$CHROMS" --assembly "$ASM" --min-mapq "$MAPQ" --walks-policy 5unique --max-inter-align-gap 30 --report-alignment-end 5 --add-columns mapq,pos5,pos3,cigar,read_len --output-stats "$stats") | \\"
        log "  $(quote_cmd "${PAIRS_RS_CMD[@]}" sort --nproc "$SORT_THREADS" --tmpdir "$TMPDIR" -o "$sorted")"
    else
        "${BWA_MEM2_CMD[@]}" mem -5SPM -T 30 -t "$THREADS" "$BWA_INDEX" "$r1" "$r2" \
            | "${PAIRS_RS_CMD[@]}" parse \
                --chroms-path "$CHROMS" \
                --assembly "$ASM" \
                --min-mapq "$MAPQ" \
                --walks-policy 5unique \
                --max-inter-align-gap 30 \
                --report-alignment-end 5 \
                --add-columns mapq,pos5,pos3,cigar,read_len \
                --output-stats "$stats" \
            | "${PAIRS_RS_CMD[@]}" sort \
                --nproc "$SORT_THREADS" \
                --tmpdir "$TMPDIR" \
                -o "$sorted"
    fi
    validate_bgzip_file "$sorted"
    SORTED_LANES+=("$sorted")
}

link_single_lane_as_merged() {
    local sorted="$1"
    local merged="$2"
    local target

    log "Single sorted pairsam detected; linking it as $(basename "$merged")"
    if is_dry_run; then
        log "+ rm -f $(quote_cmd "$merged")"
        log "+ ln -s $(quote_cmd "$sorted") $(quote_cmd "$merged")"
        return
    fi

    rm -f "$merged"
    if [[ "$(dirname "$sorted")" == "$(dirname "$merged")" ]]; then
        target="$(basename "$sorted")"
    else
        target="$(realpath "$sorted")"
    fi
    ln -s "$target" "$merged"
}

merge_sorted_lanes() {
    local merged="$1"
    if [[ "${#SORTED_LANES[@]}" -eq 1 ]]; then
        link_single_lane_as_merged "${SORTED_LANES[0]}" "$merged"
    else
        log "Merging ${#SORTED_LANES[@]} sorted lanes with pairtools merge"
        run_cmd "${PAIRTOOLS_CMD[@]}" merge \
            --nproc "$SORT_THREADS" \
            --nproc-in "$SORT_THREADS" \
            --nproc-out "$SORT_THREADS" \
            --tmpdir "$TMPDIR" \
            -o "$merged" \
            "${SORTED_LANES[@]}"
    fi
    validate_bgzip_file "$merged"
}

dedup_pairsam() {
    local input="$1"
    local output="$2"
    local stats="$3"
    log "Running pairtools dedup for downstream duplicate removal"
    run_cmd "${PAIRTOOLS_CMD[@]}" dedup \
        --nproc-in "$SORT_THREADS" \
        --nproc-out "$SORT_THREADS" \
        --output "$output" \
        --output-stats "$stats" \
        "$input"
    validate_bgzip_file "$output"
}

select_valid_pairsam() {
    local input="$1"
    local output="$2"
    log "Selecting unique-unique pairs with pairtools select"
    run_cmd "${PAIRTOOLS_CMD[@]}" select \
        --nproc-in "$SORT_THREADS" \
        --nproc-out "$SORT_THREADS" \
        -o "$output" \
        '(pair_type == "UU")' \
        "$input"
    validate_bgzip_file "$output"
}

split_pairsam_to_pairs_and_bam() {
    local input="$1"
    local pairs_out="$2"
    local bam_out="$3"
    local bam_tmp_prefix="$TMPDIR/$(basename "${bam_out%.bam}").samtools"

    log "Splitting pairsam with pairtools and sorting SAM stream with samtools"
    if is_dry_run; then
        log "+ $(quote_cmd "${PAIRTOOLS_CMD[@]}" split --nproc-in "$SORT_THREADS" --nproc-out "$SORT_THREADS" --output-pairs "$pairs_out" --output-sam - "$input") | \\"
        log "  $(quote_cmd "${SAMTOOLS_CMD[@]}" view -@ "$THREADS" -bS -) | \\"
        log "  $(quote_cmd "${SAMTOOLS_CMD[@]}" sort -@ "$THREADS" -T "$bam_tmp_prefix" -o "$bam_out" -)"
        log "+ $(quote_cmd "${SAMTOOLS_CMD[@]}" index -@ "$THREADS" "$bam_out")"
    else
        "${PAIRTOOLS_CMD[@]}" split \
            --nproc-in "$SORT_THREADS" \
            --nproc-out "$SORT_THREADS" \
            --output-pairs "$pairs_out" \
            --output-sam - \
            "$input" \
            | "${SAMTOOLS_CMD[@]}" view -@ "$THREADS" -bS - \
            | "${SAMTOOLS_CMD[@]}" sort -@ "$THREADS" -T "$bam_tmp_prefix" -o "$bam_out" -
        "${SAMTOOLS_CMD[@]}" index -@ "$THREADS" "$bam_out"
    fi

    validate_bgzip_file "$pairs_out"
    run_cmd "${SAMTOOLS_CMD[@]}" quickcheck "$bam_out"
}

write_pairtools_stats() {
    local pairs_in="$1"
    local stats_out="$2"
    log "Writing final pairtools stats with chromosome sizes from the header"
    run_cmd "${PAIRTOOLS_CMD[@]}" stats \
        --nproc-in "$SORT_THREADS" \
        --with-chromsizes \
        -o "$stats_out" \
        "$pairs_in"
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)
                DRY_RUN=1
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                die "unknown argument: $1"
                ;;
        esac
    done
}

load_config() {
    THREADS="${THREADS:-16}"
    SORT_THREADS="${SORT_THREADS:-$THREADS}"
    MAPQ="${MAPQ:-10}"
    REF="${REF:?Set REF to the reference FASTA path}"
    BWA_INDEX="${BWA_INDEX:-$REF}"
    CHROMS="${CHROMS:?Set CHROMS to the chrom sizes path}"
    ASM="${ASM:?Set ASM to the assembly name}"
    PREFIX="${PREFIX:?Set PREFIX to the output prefix}"
    TMPDIR="${TMPDIR:-/tmp}"
    R1="${R1:?Set R1 to one FASTQ path or comma-separated lane paths}"
    R2="${R2:?Set R2 to one FASTQ path or comma-separated lane paths}"
    PAIRS_RS="${PAIRS_RS:-pairs-rs}"
    PAIRTOOLS="${PAIRTOOLS:-pairtools}"
    BWA_MEM2="${BWA_MEM2:-bwa-mem2}"
    SAMTOOLS="${SAMTOOLS:-samtools}"
    BGZIP="${BGZIP:-bgzip}"

    read -r -a PAIRS_RS_CMD <<< "$PAIRS_RS"
    read -r -a PAIRTOOLS_CMD <<< "$PAIRTOOLS"
    read -r -a BWA_MEM2_CMD <<< "$BWA_MEM2"
    read -r -a SAMTOOLS_CMD <<< "$SAMTOOLS"
    read -r -a BGZIP_CMD <<< "$BGZIP"
}

validate_config() {
    require_command "${PAIRS_RS_CMD[0]}" "PAIRS_RS"
    require_command "${PAIRTOOLS_CMD[0]}" "PAIRTOOLS"
    require_command "${BWA_MEM2_CMD[0]}" "BWA_MEM2"
    require_command "${SAMTOOLS_CMD[0]}" "SAMTOOLS"
    require_command "${BGZIP_CMD[0]}" "BGZIP"

    require_readable_file "$REF" "REF"
    require_readable_file "$CHROMS" "CHROMS"
    validate_bwa_index "$BWA_INDEX"

    split_csv_paths "$R1" LANE_R1
    split_csv_paths "$R2" LANE_R2
    [[ "${#LANE_R1[@]}" -gt 0 ]] || die "R1 contains no lanes"
    [[ "${#LANE_R1[@]}" -eq "${#LANE_R2[@]}" ]] || die "R1 and R2 lane counts differ"

    for i in "${!LANE_R1[@]}"; do
        require_readable_file "${LANE_R1[$i]}" "R1 lane $((i + 1))"
        require_readable_file "${LANE_R2[$i]}" "R2 lane $((i + 1))"
    done

    case "$SORT_THREADS" in
        ''|*[!0-9]*) die "SORT_THREADS must be a positive integer: $SORT_THREADS" ;;
    esac
    case "$THREADS" in
        ''|*[!0-9]*) die "THREADS must be a positive integer: $THREADS" ;;
    esac
    case "$MAPQ" in
        ''|*[!0-9]*) die "MAPQ must be a non-negative integer: $MAPQ" ;;
    esac
    [[ "$SORT_THREADS" -gt 0 ]] || die "SORT_THREADS must be greater than zero"
    [[ "$THREADS" -gt 0 ]] || die "THREADS must be greater than zero"
    [[ "$MAPQ" -le 255 ]] || die "MAPQ must be <= 255 for pairs-rs parse: $MAPQ"
}

main() {
    parse_args "$@"
    load_config
    validate_config

    local prefix_dir merged_prefix
    prefix_dir="$(dirname "$PREFIX")"
    ensure_dir "$prefix_dir"
    ensure_dir "$TMPDIR"

    merged_prefix="$prefix_dir/merged"
    MERGED_SORTED="${merged_prefix}.sorted.pairsam.gz"
    DEDUP_PAIRSAM="${merged_prefix}.dedup.pairsam.gz"
    DEDUP_STATS="${merged_prefix}.dedup.stats.txt"
    VALID_PAIRSAM="${merged_prefix}.valid.pairsam.gz"
    VALID_PAIRS="${merged_prefix}.valid.pairs.gz"
    VALID_BAM="${merged_prefix}.valid.bam"
    FINAL_STATS="${merged_prefix}.valid.stats.txt"

    log "Hybrid mode: pairs-rs is used only for parse+sort; pairtools handles merge/dedup/select/split/stats."

    SORTED_LANES=()
    local lane_count="${#LANE_R1[@]}"
    for i in "${!LANE_R1[@]}"; do
        run_parse_sort_lane "$((i + 1))" "$lane_count" "${LANE_R1[$i]}" "${LANE_R2[$i]}"
    done

    merge_sorted_lanes "$MERGED_SORTED"
    dedup_pairsam "$MERGED_SORTED" "$DEDUP_PAIRSAM" "$DEDUP_STATS"
    select_valid_pairsam "$DEDUP_PAIRSAM" "$VALID_PAIRSAM"
    split_pairsam_to_pairs_and_bam "$VALID_PAIRSAM" "$VALID_PAIRS" "$VALID_BAM"
    write_pairtools_stats "$VALID_PAIRS" "$FINAL_STATS"

    log "Done."
    log "Final pairs: $VALID_PAIRS"
    log "Final BAM:   $VALID_BAM"
    log "Final stats: $FINAL_STATS"
}

main "$@"
