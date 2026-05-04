#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

chroms="tests/fixtures/walks/walks.chrom.sizes"
outdir="tests/oracle/walks"
mkdir -p "$outdir"

policies=(mask 5any 5unique 3any 3unique all)
cases=(
  simple_non_walk
  r1_chimera_rescue_cis_convergent_near
  r2_chimera_rescue_cis_convergent_near
  chimera_no_rescue_trans
  chimera_no_rescue_wrong_orientation
  chimera_no_rescue_too_far
  chimera_inner_multi_rescue
  chimera_inner_null_rescue
  both_sides_chimeric_2x2
  multi_alignment_with_long_gap
  no_unique_available_for_5unique
  no_unique_available_for_3unique
  all_policy_simple_three_alignments
)

run_oracle() {
  local case_name="$1"
  local policy="$2"
  local gap="$3"
  local suffix="$case_name.$policy"
  if [[ "$case_name" == "multi_alignment_with_long_gap" && "$gap" != "30" ]]; then
    suffix="$case_name.gap${gap}.$policy"
  fi

  pixi run pairtools parse \
    --chroms-path "$chroms" \
    --assembly walks_test \
    --min-mapq 10 \
    --walks-policy "$policy" \
    --max-inter-align-gap "$gap" \
    --max-molecule-size 750 \
    --report-alignment-end 5 \
    --add-columns mapq,pos5,pos3,cigar,read_len \
    --output-stats "$outdir/$suffix.stats.txt" \
    "tests/fixtures/walks/$case_name.sam" \
    > "$outdir/$suffix.pairsam"
}

for case_name in "${cases[@]}"; do
  for policy in "${policies[@]}"; do
    run_oracle "$case_name" "$policy" 30
  done
done

for policy in "${policies[@]}"; do
  run_oracle multi_alignment_with_long_gap "$policy" 300
done

echo "generated walk oracles in $outdir"
