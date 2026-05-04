# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M100: downstream command planning.

M020, M030, M040, M050, M060, M070, and M090 are marked complete. M090 was closed after validating the benchmark harness syntax and documenting that performance claims remain gated by parity plus an explicit benchmark run.

## Current branch

`master`

## Current commit

`uncommitted` during M090 closure. The final task response must report the committed SHA.

## Implemented behavior

Completed parse milestones are covered by the guarded oracle suite:

- M020 Parse I/O:
  - SAM path and stdin SAM produce identical parse output.
  - BAM path generated through rust-htslib produces identical parse output to the SAM fixture.
  - `-o` writes parse output to a file and leaves stdout empty.
  - `--output-stats` writes parse stats to a file.
  - Compressed parse output and compressed parse stats output fail loudly.
- M030 Parse core pairs:
  - Oracle parity is covered for simple UU pairs, unmapped mates, low-MAPQ mates, reverse 5'/3' coordinate reporting, interchromosomal flip, and same-chromosome position flip.
- M040 Pairsam and extra columns:
  - Oracle parity is covered for scoped pairsam output.
  - Supported `--add-columns mapq,pos5,pos3,cigar,read_len` is covered.
  - Parse stats output is covered.
  - Unsupported add-columns fail loudly.
- M050 Walks and chimeric limits:
  - Scoped secondary and supplementary fixtures are covered.
  - BWA-MEM2-style leading soft-clipped split behavior is covered for `--max-inter-align-gap`.
  - Unsupported walk policies fail loudly.
- M060 Sort core:
  - Pairtools-compatible default sort order is covered by oracle tests.
  - Parse-generated `.pairsam` with `sam1`, `sam2`, and supported extra columns is covered.
  - Equal-key order is deterministic across spilled chunks and identical for `--nproc 1` and `--nproc 8`.
  - Header update behavior is covered, including an existing `#samheader` `@PG` chain.
  - Unsupported sort options fail loudly.
- M070 Sort compression and tempfiles:
  - `.gz` sort output is written through HTSlib BGZF and validates with `gzip -dc` and `bgzip -t` in tests.
  - Decompressed `.gz` output is identical for `--nproc 1` and `--nproc 8`.
  - `--tmpdir` is covered by a test that fails if the requested spill directory is ignored.
- M090 Benchmarking:
  - `scripts/benchmark_sort_threads.sh` records wall time, CPU utilization, max RSS, temp disk usage, compressed and uncompressed output sizes, and compression throughput when run.
  - The harness includes a compression-dominates mode and optional gates for speedup and CPU utilization.
  - M090 did not run a benchmark and does not add a performance claim.

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Full complex-walk parity is not claimed beyond the scoped M050 fixtures.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- Rust downstream commands remain unimplemented.
- M090 does not claim measured speedup or CPU utilization.
- No downstream Rust behavior is implemented.

## Validation performed

Validation commands for M090 closure:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M090
bash -n scripts/benchmark_sort_threads.sh
bash -n scripts/real_bam_compare.sh
python3 scripts/check_cargo_needed.py --milestone M090
```

## Validation not performed and why

- Benchmarks were not run in this task. M090 validated the harness and parity gate, but no input dataset or benchmark run was requested for a performance report.
- Cargo was not run because M090 changed only docs and milestone state.

## Cargo required

No. `python3 scripts/check_cargo_needed.py --milestone M090` reported `cargo_required=false`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M100: downstream command planning.
