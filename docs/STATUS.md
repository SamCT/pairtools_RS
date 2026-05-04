# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M070: sort compression and tempfiles.

M020, M030, M040, M050, and M060 are marked complete. M060 was closed after the guarded oracle suite passed with the existing sort-core coverage.

## Current branch

`master`

## Current commit

`uncommitted` during M060 closure. The final task response must report the committed SHA.

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

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Full complex-walk parity is not claimed beyond the scoped M050 fixtures.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- Rust downstream commands remain unimplemented.
- M060 does not claim compression throughput or temp-disk performance; that is M070/M090 territory.
- No benchmarks or speedups are claimed.

## Validation performed

Validation commands for M060 closure:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M060
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
```

`scripts/cargo_guard.sh test` passed 20 integration tests, including the M060 sort oracle, stable spill, header, gzip, and loud-failure checks.

## Validation not performed and why

- Benchmarks were not run because M060 is a correctness milestone, not a performance milestone.
- New sort implementation code was not added in this closure; the work reconciles existing oracle coverage and milestone state.

## Cargo required

Yes. M060 requires `scripts/cargo_guard.sh check` and `scripts/cargo_guard.sh test`, and both passed.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M070: sort compression and tempfiles.
