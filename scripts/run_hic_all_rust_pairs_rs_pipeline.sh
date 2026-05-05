#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Run an all-Rust pairtools-equivalent Hi-C pipeline.

This script uses pairs-rs for parse, sort, merge, dedup, select, split, and
stats. It still uses bwa-mem2 for alignment and samtools for BAM conversion,
coordinate sorting, quickcheck, and indexing.

Required environment:
  THREADS       bwa-mem2 thread count
  SORT_THREADS  pairs-rs sort and samtools thread count
  MAPQ          pairs-rs parse --min-mapq value
  BWA_INDEX     bwa-mem2 index prefix
  CHROMS        chrom sizes file
  ASM           assembly name for pairs-rs parse
  PREFIX        per-sample output prefix
  TMPDIR        temporary directory for sort and samtools
  R1            R1 FASTQ, or comma-separated R1 lane list
  R2            R2 FASTQ, or comma-separated R2 lane list

Optional environment:
  PAIRS_RS      pairs-rs command. Default: pairs-rs
  BWA_MEM2      bwa-mem2 command. Default: bwa-mem2
  SAMTOOLS      samtools command. Default: samtools
  BGZIP         bgzip command used only for output validation. Default: bgzip
  DRY_RUN       1 prints the planned commands without creating outputs

Examples:
  DRY_RUN=1 THREADS=64 SORT_THREADS=32 MAPQ=10 BWA_INDEX=H1_s3 \
    CHROMS=Hop282H1.chrom.sizes ASM=HopH1_282 PREFIX=hic TMPDIR=/scratch \
    R1=Plant_R1.fastq.gz R2=Plant_R2.fastq.gz \
    scripts/run_hic_all_rust_pairs_rs_pipeline.sh
USAGE
}

die() {
  echo "error: $*" >&2
  exit 2
}

log() {
  echo "$*" >&2
}

is_dry_run() {
  [[ "${DRY_RUN:-0}" == "1" || "${DRY_RUN:-0}" == "true" || "${DRY_RUN:-0}" == "yes" ]]
}

quote_cmd() {
  local out="" q
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

split_command() {
  local value="$1"
  local -n out_ref="$2"
  read -r -a out_ref <<< "$value"
}

split_csv_paths() {
  local value="$1"
  local -n out_ref="$2"
  IFS=',' read -r -a out_ref <<< "$value"
  local i
  for i in "${!out_ref[@]}"; do
    out_ref[$i]="${out_ref[$i]#"${out_ref[$i]%%[![:space:]]*}"}"
    out_ref[$i]="${out_ref[$i]%"${out_ref[$i]##*[![:space:]]}"}"
  done
}

require_command() {
  local cmd="$1" name="$2"
  [[ -n "$cmd" ]] || die "$name command is empty"
  if [[ "$cmd" == */* ]]; then
    [[ -x "$cmd" ]] || die "$name is not executable: $cmd"
  else
    command -v "$cmd" >/dev/null 2>&1 || die "$name is not on PATH: $cmd"
  fi
}

require_readable_file() {
  local path="$1" name="$2"
  [[ -n "$path" ]] || die "$name is empty"
  [[ -r "$path" ]] || die "$name is not readable: $path"
}

require_uint() {
  local value="$1" name="$2"
  [[ "$value" =~ ^[0-9]+$ ]] || die "$name must be numeric: $value"
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

make_dir() {
  local dir="$1"
  if is_dry_run; then
    log "+ mkdir -p $(quote_cmd "$dir")"
  else
    mkdir -p "$dir"
  fi
}

validate_gzip() {
  local path="$1"
  if is_dry_run; then
    log "+ $(quote_cmd "${BGZIP_CMD[@]}" -t "$path")"
  else
    [[ -s "$path" ]] || die "expected gzip output is missing or empty: $path"
    "${BGZIP_CMD[@]}" -t "$path"
  fi
}

lane_prefix() {
  local lane_count="$1" lane_index="$2"
  if (( lane_count == 1 )); then
    printf "%s" "$PREFIX"
  else
    printf "%s.lane%02d" "$PREFIX" "$lane_index"
  fi
}

load_config() {
  THREADS="${THREADS:?Set THREADS}"
  SORT_THREADS="${SORT_THREADS:?Set SORT_THREADS}"
  MAPQ="${MAPQ:?Set MAPQ}"
  BWA_INDEX="${BWA_INDEX:?Set BWA_INDEX}"
  CHROMS="${CHROMS:?Set CHROMS}"
  ASM="${ASM:?Set ASM}"
  PREFIX="${PREFIX:?Set PREFIX}"
  TMPDIR="${TMPDIR:?Set TMPDIR}"
  R1="${R1:?Set R1}"
  R2="${R2:?Set R2}"
  PAIRS_RS="${PAIRS_RS:-pairs-rs}"
  BWA_MEM2="${BWA_MEM2:-bwa-mem2}"
  SAMTOOLS="${SAMTOOLS:-samtools}"
  BGZIP="${BGZIP:-bgzip}"

  split_command "$PAIRS_RS" PAIRS_RS_CMD
  split_command "$BWA_MEM2" BWA_MEM2_CMD
  split_command "$SAMTOOLS" SAMTOOLS_CMD
  split_command "$BGZIP" BGZIP_CMD
}

validate_config() {
  require_uint "$THREADS" THREADS
  require_uint "$SORT_THREADS" SORT_THREADS
  require_uint "$MAPQ" MAPQ
  (( THREADS > 0 )) || die "THREADS must be greater than zero"
  (( SORT_THREADS > 0 )) || die "SORT_THREADS must be greater than zero"
  (( MAPQ <= 255 )) || die "MAPQ must be <= 255"

  require_command "${PAIRS_RS_CMD[0]}" PAIRS_RS
  require_command "${BWA_MEM2_CMD[0]}" BWA_MEM2
  require_command "${SAMTOOLS_CMD[0]}" SAMTOOLS
  require_command "${BGZIP_CMD[0]}" BGZIP
  require_readable_file "$CHROMS" CHROMS
  validate_bwa_index "$BWA_INDEX"

  split_csv_paths "$R1" LANE_R1
  split_csv_paths "$R2" LANE_R2
  (( ${#LANE_R1[@]} > 0 )) || die "R1 contains no lanes"
  (( ${#LANE_R1[@]} == ${#LANE_R2[@]} )) || die "R1 and R2 lane counts differ"

  local i
  for i in "${!LANE_R1[@]}"; do
    require_readable_file "${LANE_R1[$i]}" "R1 lane $((i + 1))"
    require_readable_file "${LANE_R2[$i]}" "R2 lane $((i + 1))"
  done
}

parse_sort_lane() {
  local lane_index="$1" lane_count="$2" r1="$3" r2="$4"
  local prefix sorted stats
  prefix="$(lane_prefix "$lane_count" "$lane_index")"
  sorted="${prefix}.sorted.pairsam.gz"
  stats="${prefix}.parse.stats.txt"

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

  validate_gzip "$sorted"
  SORTED_PAIRSAMS+=("$sorted")
}

link_or_merge_sorted_pairsams() {
  local merged="$1"
  if (( ${#SORTED_PAIRSAMS[@]} == 1 )); then
    if is_dry_run; then
      log "+ rm -f $(quote_cmd "$merged")"
      log "+ ln -s $(quote_cmd "${SORTED_PAIRSAMS[0]}") $(quote_cmd "$merged")"
    else
      rm -f "$merged"
      ln -s "${SORTED_PAIRSAMS[0]}" "$merged"
    fi
  else
    run_cmd "${PAIRS_RS_CMD[@]}" merge \
      -o "$merged" \
      "${SORTED_PAIRSAMS[@]}"
  fi
  validate_gzip "$merged"
}

run_downstream() {
  local merged_sorted="$1"

  run_cmd "${PAIRS_RS_CMD[@]}" dedup \
    --mark-dups \
    --output-stats merged.dedup.stats.txt \
    --output-dups merged.dups.pairsam.gz \
    --output-unmapped merged.unmapped.pairsam.gz \
    -o merged.nodups.pairsam.gz \
    "$merged_sorted"
  validate_gzip "merged.nodups.pairsam.gz"
  validate_gzip "merged.dups.pairsam.gz"
  validate_gzip "merged.unmapped.pairsam.gz"

  run_cmd "${PAIRS_RS_CMD[@]}" select \
    '(pair_type == "UU")' \
    -o merged.valid.pairsam.gz \
    merged.nodups.pairsam.gz
  validate_gzip "merged.valid.pairsam.gz"

  if is_dry_run; then
    log "+ $(quote_cmd "${PAIRS_RS_CMD[@]}" split --output-pairs merged.valid.pairs.gz --output-sam - merged.valid.pairsam.gz) | \\"
    log "  $(quote_cmd "${SAMTOOLS_CMD[@]}" view -@ "$SORT_THREADS" -b -) | \\"
    log "  $(quote_cmd "${SAMTOOLS_CMD[@]}" sort -@ "$SORT_THREADS" -o merged.valid.coord.bam -)"
  else
    "${PAIRS_RS_CMD[@]}" split \
      --output-pairs merged.valid.pairs.gz \
      --output-sam - \
      merged.valid.pairsam.gz \
      | "${SAMTOOLS_CMD[@]}" view -@ "$SORT_THREADS" -b - \
      | "${SAMTOOLS_CMD[@]}" sort -@ "$SORT_THREADS" -o merged.valid.coord.bam -
  fi
  validate_gzip "merged.valid.pairs.gz"

  run_cmd "${SAMTOOLS_CMD[@]}" index merged.valid.coord.bam
  if is_dry_run; then
    log "+ $(quote_cmd "${SAMTOOLS_CMD[@]}" quickcheck merged.valid.coord.bam)"
  else
    "${SAMTOOLS_CMD[@]}" quickcheck merged.valid.coord.bam
  fi

  run_cmd "${PAIRS_RS_CMD[@]}" stats \
    --with-chromsizes \
    -o merged.valid.stats.txt \
    merged.valid.pairs.gz
}

parse_args() {
  while (($#)); do
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

main() {
  parse_args "$@"
  load_config
  validate_config

  local output_dir
  output_dir="$(dirname "$PREFIX")"
  make_dir "$output_dir"
  make_dir "$TMPDIR"

  SORTED_PAIRSAMS=()
  local lane_count="${#LANE_R1[@]}"
  local i
  for i in "${!LANE_R1[@]}"; do
    parse_sort_lane "$((i + 1))" "$lane_count" "${LANE_R1[$i]}" "${LANE_R2[$i]}"
  done

  (
    if ! is_dry_run; then
      mkdir -p "$output_dir"
    fi
    cd "$output_dir"
    link_or_merge_sorted_pairsams "merged.sorted.pairsam.gz"
    run_downstream "merged.sorted.pairsam.gz"
  )
}

main "$@"
