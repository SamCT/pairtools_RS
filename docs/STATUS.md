# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M090: benchmarking.

M020, M030, M040, M050, M060, and M070 are marked complete. M070 was closed after the guarded test suite passed with compression, `nproc`, and tempfile coverage.

## Current branch

`master`

## Current commit

`uncommitted` during M070 closure. The final task response must report the committed SHA.

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

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Full complex-walk parity is not claimed beyond the scoped M050 fixtures.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- Rust downstream commands remain unimplemented.
- M070 does not claim measured compression speedup or CPU utilization; performance reporting is M090.
- No benchmarks or speedups are claimed.

## Validation performed

Validation commands for M070 closure:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M070
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
```

`scripts/cargo_guard.sh test` passed 21 integration tests, including the M070 gzip, BGZF, `--nproc`, unsupported compression option, and tmpdir checks.

## Validation not performed and why

- Benchmarks were not run because M070 validates behavior only. M090 is the active benchmark milestone.
- CPU-utilization proof was not added in M070; the current claim is functional BGZF/thread-count wiring and decompressed output parity, not measured speedup.

## Cargo required

Yes. M070 changed tests and requires `scripts/cargo_guard.sh check` plus `scripts/cargo_guard.sh test`; both passed.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M090: benchmarking, with parity as the prerequisite for any performance claims.
