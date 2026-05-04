# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M140: split core.

M131 and M132 are complete. They extended `pairs-rs stats` from stable count fields to pairtools-style report parity for the committed small stats fixture, plus stats merge, YAML output, and HTSlib BGZF threaded `.gz` stats I/O. M140 is active again for split core work.

## Current branch

`master`

## Current commit

`uncommitted` during M131/M132 implementation. The final task response must report the committed SHA.

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
- M150 Dedup core:
  - `pairs-rs dedup` streams sorted `.pairs`/`.pairsam` input and routes parent/nodup, duplicate, and unmapped records.
  - `-o/--output`, `--output-dups`, `--output-unmapped`, and `--output-stats` are implemented for the scoped pipeline contract.
  - Plain and `.gz` input/output are supported through HTSlib BGZF helpers.
  - Duplicate detection supports `--method max` and `--method sum` with `--max-mismatch`.
  - `--mark-dups` marks duplicate pair records as `pair_type` `DD`; pairsam `sam1`/`sam2` fields have duplicate flag `0x400` set and `Yt:Z:DD` updated where feasible.
  - A committed fixture compares read routing against installed Python pairtools.
- M130 Stats core:
  - `pairs-rs stats` computes stable pairtools-compatible count fields on small `.pairs`/`.pairsam` inputs.
  - Oracle tests compare total, mapped/unmapped/single-sided, duplicate/nodup, cis/trans, pair-type, cis-threshold, fraction, chromosome-frequency, and `--with-chromsizes` fields against installed Python pairtools.
  - `-o/--output` writes plain stats output, and `.gz` output validates as BGZF.
  - Plain and `.gz` stats input is supported through HTSlib BGZF helpers.
  - Unsupported stats options fail loudly with `not implemented`.
- M131 Stats report parity:
  - `pairs-rs stats` now emits the full pairtools-style TSV report for the committed small stats fixture, including distance-frequency bins, convergence summary fields, chromosome sizes by default, and library complexity.
  - Oracle tests compare normalized full report output against installed Python pairtools for default output, `--no-chromsizes`, and `--n-dist-bins-decade 1`.
  - `summary/complexity_naive` is compared numerically with tolerance because the Rust implementation uses a local Lambert W solver instead of SciPy.
- M132 Stats I/O and merge:
  - `pairs-rs stats --merge` merges TSV stats files and is oracle-tested against pairtools for committed small stats outputs.
  - `pairs-rs stats --yaml` emits pairtools-style YAML for the committed fixture and is oracle-tested after normalizing only the complexity representation.
  - `--nproc-in` and `--nproc-out` control HTSlib BGZF threading for `.gz` stats input and output.
  - `--cmd-in`, `--cmd-out`, `--merge --yaml`, filters, by-tile stats, chrom subsets, type casts, and custom shell compression remain loud non-goals.

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- `select` supports only exact `pair_type == "VALUE"` predicates. The broader pairtools expression language is not implemented.
- `merge` supports small sorted inputs only. Broad pairtools merge options such as `--nproc`, `--tmpdir`, `--memory`, `--compress-program`, `--keep-first-header`, and `--concatenate` remain explicitly unsupported.
- `dedup` does not yet implement full pairtools stats, by-tile stats, alternate backends, parent IDs, extra-column duplicate matching, filtering, YAML output, chrom subsets, type casts, or custom input/output shell commands.
- `stats` does not yet implement YAML merge mode, expression filters, chrom subsets, by-tile duplicate statistics, type casts, custom compression shell commands, or broad uncommitted-fixture parity beyond the tested report surface.
- Rust split and other downstream commands remain unimplemented until their command-specific milestones land.
- Compressed parse output and compressed parse stats output are not implemented.
- No benchmark or speedup is claimed by M056.

## Validation performed

Validation commands for M131/M132:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M131
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
python3 scripts/check_milestone_schema.py
python3 scripts/milestone_gate.py pre --milestone M132
python3 scripts/check_no_runtime_pairtools.py --milestone M132
python3 scripts/check_no_noop_flags.py --milestone M132
python3 scripts/check_parse_lite_drift.py --milestone M132
python3 scripts/check_cargo_needed.py --milestone M132
python3 scripts/milestone_gate.py post --milestone M131 --allow-nonactive
python3 scripts/milestone_gate.py post --milestone M132 --allow-nonactive
python3 scripts/codex_report.py --milestone M132
git diff --check
```

`scripts/cargo_guard.sh test` passed 40 compatibility tests and the walk oracle test after adding stats report, merge, YAML, and threaded BGZF coverage.

## Validation not performed and why

- Benchmarks were not run because M131/M132 are correctness milestones.
- Exact byte-for-byte `summary/complexity_naive` string parity is not claimed because Rust uses a local Lambert W implementation while pairtools uses SciPy; tests compare it numerically within tolerance.
- YAML merge mode was not run because `pairs-rs stats --merge --yaml` remains explicitly unsupported.

## Cargo required

Yes. M131/M132 changed Rust source and tests. `scripts/cargo_guard.sh check` and `scripts/cargo_guard.sh test` both passed through Pixi/WSL with `CARGO_TARGET_DIR=$HOME/pairtools_RS_target_codex`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M140: implement and oracle-test scoped `pairs-rs split` output for pairs and SAM stream handoff.
