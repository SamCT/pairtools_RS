# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M056: parse `--walks-policy all`.

M055 is complete. It added oracle-driven walk-resolution parity for the non-`all` `pairtools parse --walks-policy` values and split the broader `all` behavior into M056.

## Current branch

`master`

## Current commit

`uncommitted` during M055 closure. The final task response must report the committed SHA.

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
- M055 Walk-resolution parity:
  - `--walks-policy mask`, `5any`, `5unique`, `3any`, and `3unique` are accepted and compared to pairtools oracle fixtures.
  - Alignment choice is ordered by 5' distance along the read, independent of input order.
  - `--max-inter-align-gap` inserts null alignments for long read-span gaps and is tested at small and large thresholds.
  - Single-ligation rescue and unrescuable walk policy selection are covered for deterministic SAM fixtures.
  - `--max-molecule-size` is accepted for parse rescue decisions.
  - Pairsam rows, pair-type counts, and parse stats are compared against pairtools-generated walk oracles.
- M060 Sort core:
  - Pairtools-compatible default sort order is covered by oracle tests.
  - Parse-generated `.pairsam` with `sam1`, `sam2`, and supported extra columns is covered.
  - Equal-key order is deterministic across spilled chunks and identical for `--nproc 1` and `--nproc 8`.
- M070 Sort compression and tempfiles:
  - `.gz` sort output is written through HTSlib BGZF and validates with `gzip -dc` and `bgzip -t` in tests.
  - Decompressed `.gz` output is identical for `--nproc 1` and `--nproc 8`.
  - `--tmpdir` is covered by a test that fails if the requested spill directory is ignored.
- M090 Benchmarking:
  - `scripts/benchmark_sort_threads.sh` records wall time, CPU utilization, max RSS, temp disk usage, compressed and uncompressed output sizes, and compression throughput when run.
  - M090 did not run a benchmark and does not add a performance claim.

## Intentionally unsupported behavior

- `pairtools parse --walks-policy all` remains explicitly not implemented and is split into M056.
- Full pairtools `parse2` behavior is not implemented.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- Rust downstream commands remain unimplemented.
- Compressed parse output and compressed parse stats output are not implemented.
- No benchmark or speedup is claimed by M055.

## Validation performed

Validation commands for M055:

```bash
git status --short --branch
cat milestones/ACTIVE_MILESTONE
python3 scripts/milestone_gate.py pre --milestone M055
python3 scripts/check_cargo_needed.py --milestone M055
bash tests/scripts/generate_walk_oracles.sh
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
```

`scripts/cargo_guard.sh test` passed 23 Rust integration tests after adding the walk oracle suite. `bash tests/scripts/generate_walk_oracles.sh` generated 130 pairtools oracle files for 13 case/threshold combinations across five non-`all` policies.

## Validation not performed and why

- `--walks-policy all` oracle parity was not implemented or claimed in M055. It is the active M056 follow-up.
- Benchmarks were not run because M055 is a correctness milestone.

## Cargo required

Yes. M055 changed Rust source and tests. `scripts/cargo_guard.sh check` and `scripts/cargo_guard.sh test` both passed through Pixi/WSL with `CARGO_TARGET_DIR=$HOME/pairtools_RS_target_codex`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M056: implement and oracle-test `pairtools parse --walks-policy all`.
