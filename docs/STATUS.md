# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M130: stats core.

M120 is complete. It added `pairs-rs merge` for small sorted pairs/pairsam inputs and activated M130 for stats core work.

## Current branch

`master`

## Current commit

`uncommitted` during M120 closure. The final task response must report the committed SHA.

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
- M056 Parse all-walks policy:
  - `--walks-policy all` is accepted and compared to pairtools oracle fixtures.
  - All-policy emission covers adjacent internal walk edges, the terminal R1/R2 bridge edge, 5'/3' endpoint reporting for internal edges, inserted null segments, multi-mapping segments, both-side 2x2 chimeric walks, and a three-alignment R1 walk fixture.
  - Pairsam rows, pair-type counts, and parse stats match pairtools oracle outputs for 14 case/threshold combinations across all six supported walk policies.
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
- M100 Downstream command planning:
  - `docs/DOWNSTREAM_MILESTONES.md` defines the staged downstream command sequence.
- M110 Select core:
  - `pairs-rs select '(pair_type == "UU")'` matches pairtools oracle output on small `.pairs` and `.pairsam` fixtures after normalizing volatile select `@PG` command text.
  - `-o/--output` writes selected output to plain files and `.gz` BGZF output.
  - Unsupported predicates and unsupported select options fail loudly with `not implemented`.
- M120 Merge core:
  - `pairs-rs merge` matches pairtools oracle output on a small sorted `.pairs` fixture.
  - Scoped sorted `.pairsam` merge coverage compares body output and compatible header structure after normalizing volatile merge `@PG` command text.
  - `-o/--output` writes merged output to plain files and `.gz` BGZF output.
  - Unsupported merge options fail loudly with `not implemented`.

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- `select` supports only exact `pair_type == "VALUE"` predicates. The broader pairtools expression language is not implemented.
- `merge` supports small sorted inputs only. Broad pairtools merge options such as `--nproc`, `--tmpdir`, `--memory`, `--compress-program`, `--keep-first-header`, and `--concatenate` remain explicitly unsupported.
- Rust dedup, split, stats, and other downstream commands remain unimplemented until their command-specific milestones land.
- Compressed parse output and compressed parse stats output are not implemented.
- No benchmark or speedup is claimed by M056.

## Validation performed

Validation commands for M120:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M120
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
python3 scripts/milestone_gate.py post --milestone M120 --allow-nonactive
python3 scripts/codex_report.py --milestone M120
git diff --check
```

`scripts/cargo_guard.sh test` passed 28 compatibility tests and the walk oracle test after adding merge coverage.

## Validation not performed and why

- Benchmarks were not run because M120 is a correctness milestone.

## Cargo required

Yes. M120 changed Rust source and tests. `scripts/cargo_guard.sh check` and `scripts/cargo_guard.sh test` both passed through Pixi/WSL with `CARGO_TARGET_DIR=$HOME/pairtools_RS_target_codex`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M130: implement and oracle-test scoped `pairs-rs stats` output for stable small-fixture counts.
