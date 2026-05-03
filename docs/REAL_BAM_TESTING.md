# Real BAM Parity And Benchmark Harness

Use this for large, external test data such as `BWAMEM2_R1R2_s01.bam`. Keep the BAM and full pairtools outputs outside git.

Recommended Windows location:

```text
D:\pairtools_RS_testdata\hop_s01\
```

The same folder in WSL:

```bash
/mnt/d/pairtools_RS_testdata/hop_s01
```

## Expected Layout

```text
D:\pairtools_RS_testdata\hop_s01\
  BWAMEM2_R1R2_s01.bam
  Hop282H1.chrom.sizes
  out_s01.PAIRTOOLSDEF.sorted.pairs        # optional saved output from the first command
  pipeline.sh                              # optional full pairtools pipeline for reference
  hic.sorted.pairsam.gz                    # optional full-mode output
  merged.sorted.pairsam.gz                 # optional full-mode output
  merged.nodups.pairsam.gz                 # optional full-mode output
  merged.valid.pairs.gz                    # optional full-mode output
  merged.valid.stats.txt                   # optional full-mode output
```

The harness still benchmarks this original supported subset:

```bash
pairtools parse \
  -c Hop282H1.chrom.sizes \
  --drop-sam \
  --min-mapq 1 \
  --walks-policy 5unique \
  --report-alignment-end 5 \
  BWAMEM2_R1R2_s01.bam \
| pairtools sort
```

The harness now also compares parse-only pairsam output for the current target parse surface:

```bash
pairtools parse \
  -c Hop282H1.chrom.sizes \
  --assembly "$ASM" \
  --min-mapq "$FULL_MAPQ" \
  --walks-policy 5unique \
  --max-inter-align-gap "$FULL_MAX_INTER_ALIGN_GAP" \
  --report-alignment-end 5 \
  --add-columns mapq,pos5,pos3,cigar,read_len \
  BWAMEM2_R1R2_s01.bam
```

For the parse-only comparison, the script removes `#samheader:` lines before diffing to avoid irrelevant BAM header provenance differences while preserving pair rows, pair columns, and parse metadata.

The front-half target now includes pairsam parse, multithreaded sort, and BGZF-compatible `.gz` sort output. The rest of the full pipeline remains a future Rust parity target because it uses downstream commands that are not implemented in `pairs-rs` yet:

- downstream `merge`, `dedup`, `select`, `split`, and `stats`

The harness checks that still-unsupported parse flags fail loudly instead of being accepted as no-ops.

## Build First

Build outside the benchmark timing:

```bash
cd /mnt/d/pairtools_RS
export CARGO_TARGET_DIR="$HOME/pairtools_RS_target_codex"
pixi run cargo build --release
```

If the binary is in the external target directory, pass it explicitly:

```bash
export PAIRS_RS_BIN="$HOME/pairtools_RS_target_codex/release/pairs-rs"
```

## Exact Comparison

This compares live `pairtools 1.1.3` output to `pairs-rs` output. It runs both the original parse+sort drop-sam comparison and the parse-only normalized pairsam comparison. If `out_s01.PAIRTOOLSDEF.sorted.pairs` exists, it is also compared to the live pairtools parse+sort output.

```bash
cd /mnt/d/pairtools_RS
export PAIRTOOLS_RS_TESTDATA=/mnt/d/pairtools_RS_testdata/hop_s01
export PAIRS_RS_BIN="$HOME/pairtools_RS_target_codex/release/pairs-rs"
pixi run bash scripts/real_bam_compare.sh --compare
```

Equivalent Pixi task:

```bash
pixi run real-bam-compare
```

To preserve generated outputs for inspection:

```bash
KEEP_COMPARE_WORKDIR=1 \
COMPARE_WORKDIR=/mnt/d/pairtools_RS_testdata/hop_s01/compare_outputs \
pixi run bash scripts/real_bam_compare.sh --compare
```

## Unsupported-Option Gate

This verifies that currently unsupported parse options still fail with a `not implemented` error.

```bash
pixi run bash scripts/real_bam_compare.sh --full-gate
```

Equivalent Pixi task:

```bash
pixi run real-bam-full-gate
```

## Benchmark

Benchmarking runs only after exact supported-subset parity passes:

```bash
pixi run bash scripts/real_bam_compare.sh --benchmark
```

Equivalent Pixi task:

```bash
pixi run real-bam-benchmark
```

Optional knobs:

```bash
MAPQ=1
REPORT_ALIGNMENT_END=5
BENCHMARK_RUNS=5
REAL_BAM=BWAMEM2_R1R2_s01.bam
REAL_CHROMS=Hop282H1.chrom.sizes
EXPECTED_SORTED_PAIRS=/mnt/d/pairtools_RS_testdata/hop_s01/out_s01.PAIRTOOLSDEF.sorted.pairs
```

Do not interpret benchmark output as a project performance claim until the corresponding parity comparison has passed for that run.
