# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M080: exact hybrid pipeline.

## Current branch

`master`

## Current commit

`uncommitted` during M080 validation. The final task response must report the committed SHA.

## Goal

Reproduce the known pairtools Hi-C pipeline with `pairs-rs parse | pairs-rs sort` replacing `pairtools parse | pairtools sort` only. Downstream `pairtools merge`, `dedup`, `select`, `split`, and `stats` remain intentional shell-pipeline dependencies because those commands are not implemented in Rust yet.

## Implemented behavior

- Added `scripts/run_hic_exact_pairs_rs_pipeline.sh`, an exact production shell pipeline for M080.
- Single-lane mode writes `${PREFIX}.sorted.pairsam.gz`, `${PREFIX}.parse.stats.txt`, and symlinks `merged.sorted.pairsam.gz` beside `PREFIX`.
- Multi-lane mode writes `${PREFIX}.laneNN.sorted.pairsam.gz` and `${PREFIX}.laneNN.parse.stats.txt`, then uses `pairtools merge` to create `merged.sorted.pairsam.gz`.
- Downstream outputs are exactly:
  - `merged.nodups.pairsam.gz`
  - `merged.dups.pairsam.gz`
  - `merged.unmapped.pairsam.gz`
  - `merged.dedup.stats.txt`
  - `merged.valid.pairsam.gz`
  - `merged.valid.pairs.gz`
  - `merged.valid.coord.bam`
  - `merged.valid.coord.bam.bai`
  - `merged.valid.stats.txt`
- Added dry-run shell tests for one-lane and two-lane planning.
- Added validation shell tests for missing chrom sizes, mismatched lane counts, `SORT_THREADS=0`, `MAPQ=999`, and missing BWA index prefix.
- Added external real-data oracle harness for `/mnt/d/pairtools_RS_test`.

## Exact pipeline reproduced

The script runs:

```bash
bwa-mem2 mem -5SPM -T 30 -t "$THREADS" "$BWA_INDEX" "$R1" "$R2" \
  | pairs-rs parse \
      --chroms-path "$CHROMS" \
      --assembly "$ASM" \
      --min-mapq "$MAPQ" \
      --walks-policy 5unique \
      --max-inter-align-gap 30 \
      --report-alignment-end 5 \
      --add-columns mapq,pos5,pos3,cigar,read_len \
      --output-stats "${PREFIX}.parse.stats.txt" \
  | pairs-rs sort \
      --nproc "$SORT_THREADS" \
      --tmpdir "$TMPDIR" \
      -o "${PREFIX}.sorted.pairsam.gz"
```

Then it runs the exact downstream `pairtools dedup`, `pairtools select`, `pairtools split`, `samtools view/sort/index`, and `pairtools stats` commands documented in the M080 task.

## Intentionally unsupported behavior

- No Rust implementation of merge, dedup, select, split, stats, parse2, header, restrict, phase, sample, scaling, or filterbycov was added.
- Rust runtime still must not call pairtools, samtools, bgzip, or gzip.
- This milestone does not claim an all-Rust pairtools replacement.
- This milestone does not benchmark or claim speedups.
- `src/` was not changed.

## External real-data oracle status

The external directory `/mnt/d/pairtools_RS_test` was discovered.

Discovered files include:

- `/mnt/d/pairtools_RS_test/BWAMEM2_R1R2_s01.bam`
- `/mnt/d/pairtools_RS_test/Hop282H1.chrom.sizes`
- `/mnt/d/pairtools_RS_test/pairtools_1.sh`
- `/mnt/d/pairtools_RS_test/p3.commands`
- `/mnt/d/pairtools_RS_test/out_s01.PAIRTOOLSDEF.sorted.pairs`
- `/mnt/d/pairtools_RS_test/hic.parse.stats.txt`
- `/mnt/d/pairtools_RS_test/out_s01.pairtools.parse.stats`

The available sorted pairtools oracle is `.pairs`, not the exact M080 `.pairsam.gz` output. `p3.commands` documents a different command using `--drop-sam` and `--min-mapq 1`, so that sorted file is classified as a legacy/incompatible oracle for exact M080 output. The real-data harness records this uncertainty and does not claim full real-data parity unless an exact oracle is present and passes comparison.

M080 real-data harness result:

- `bash tests/scripts/test_hic_exact_pipeline_real_oracle.sh` passed.
- The harness ran `pairs-rs parse | pairs-rs sort` on `/mnt/d/pairtools_RS_test/BWAMEM2_R1R2_s01.bam`.
- The candidate sorted pairsam contained 11,359,961 rows and passed the expected sort-key order check.
- Sorted pairsam semantic comparison was not run because no exact `*.sorted.pairsam.gz` pairtools oracle was present.
- Stats comparisons were skipped with explicit reasons:
  - `hic.parse.stats.txt` reported `total=1.13558e+09`, greater than the aligned input line count, so it is not a compatible parse-stat oracle for this fixture.
  - `out_s01.pairtools.parse.stats` comes from `p3.commands`, which used `--drop-sam` and `--min-mapq 1`, not the exact M080 target flags.

## Validation performed

M080 validation commands:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M080
bash -n scripts/run_hic_exact_pairs_rs_pipeline.sh
bash tests/scripts/test_hic_exact_pipeline_dry_run.sh
bash tests/scripts/test_hic_exact_pipeline_validation.sh
bash tests/scripts/test_hic_exact_pipeline_real_oracle.sh
python3 scripts/check_no_runtime_pairtools.py --milestone M080
python3 scripts/check_no_noop_flags.py --milestone M080
python3 scripts/check_parse_lite_drift.py --milestone M080
python3 scripts/check_cargo_needed.py --milestone M080
python3 scripts/milestone_gate.py post --milestone M080
python3 scripts/codex_report.py --milestone M080
git diff --check
```

## Validation not performed and why

- Cargo is not expected for M080 if only shell scripts, docs, and shell tests change.
- `RUN_REAL_DOWNSTREAM=1` was not run because exact downstream oracle files are not present. The discovered external directory currently does not include `merged.nodups.pairsam.gz`, `merged.valid.pairs.gz`, `merged.valid.coord.bam`, or `merged.valid.stats.txt`.
- Benchmarks are not run because performance is out of scope.

## Cargo required

Expected: no, unless a Rust/Cargo/Pixi file changes.

## Next recommended milestone

M090 benchmarking only after the exact hybrid dry-run and a real small end-to-end test pass. If real-data parse/sort parity gaps appear, return to M020/M030/M040 as appropriate.
