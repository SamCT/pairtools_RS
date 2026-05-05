# pairs-rs Status

Last reconciled: 2026-05-05

## Active milestone

M140: split core.

M151 is complete. It adds production-shaped dedup command validation so completion claims are backed by the exact command shape, not only small synthetic unit-style fixtures. M140 split core is active again.

## Current branch

`master`

## Current commit

`uncommitted` during M151 completion and autonomy protocol documentation. The final task response must report the committed SHA.

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
- M151 Dedup production command validation:
  - `tests/scripts/test_dedup_pipeline_command_shape.sh` runs the production-shaped command:
    `pairs-rs dedup --mark-dups --output-stats merged.dedup.s01.RS.stats.txt --output-dups merged.dups.pairsam.s01.RS.gz --output-unmapped merged.unmapped.pairsam.s01.RS.gz -o nodups.parse_RS_s01.sorted.pairsam H1_ALL_parse_RS_1.sorted_2.pairsam`.
  - The script validates command exit, nodups output existence and body rows, compressed duplicate/unmapped outputs, required stats fields, `pair_type` `DD`, pairsam SAM duplicate flag `0x400`, `Yt:Z:DD`, and readID routing against Python pairtools on the same pipeline-style fixture.
  - This is scoped sorted-input dedup routing validation. It is not an optimization claim or full pairtools dedup parity claim.
- Codex autonomous milestone protocol:
  - `docs/CODEX_AUTONOMY.md` records the repository-local autonomous milestone executor protocol.
  - The protocol requires reading the active milestone and compatibility docs, staying inside allowed paths, running required tests, running milestone gates, documenting blockers, and avoiding parity or optimization claims unless the milestone verifies them.
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
- `dedup` does not yet implement full pairtools stats, by-tile stats, alternate backends, parent IDs, extra-column duplicate matching, filtering, YAML output, chrom subsets, type casts, custom input/output shell commands, optimization claims, or full pairtools parity beyond scoped sorted-input routing.
- `stats` does not yet implement YAML merge mode, expression filters, chrom subsets, by-tile duplicate statistics, type casts, custom compression shell commands, or broad uncommitted-fixture parity beyond the tested report surface.
- Rust split and other downstream commands remain unimplemented until their command-specific milestones land.
- Compressed parse output and compressed parse stats output are not implemented.
- No benchmark or speedup is claimed by M056.

## Validation performed

Validation commands for M151:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M151
scripts/cargo_guard.sh build
bash tests/scripts/test_dedup_pipeline_command_shape.sh
python3 scripts/check_milestone_schema.py
python3 scripts/check_no_runtime_pairtools.py --milestone M151
python3 scripts/check_no_noop_flags.py --milestone M151
python3 scripts/check_parse_lite_drift.py --milestone M151
python3 scripts/check_cargo_needed.py --milestone M151
python3 scripts/milestone_gate.py post --milestone M151
python3 scripts/codex_report.py --milestone M151
git diff --check
```

M151 validation passed in the autonomy completion pass:

```bash
scripts/cargo_guard.sh build
bash tests/scripts/test_dedup_pipeline_command_shape.sh
```

The script reported `dedup production command shape validation passed`. Pairtools emitted a warning from its stats internals on the tiny fixture; the command still exited successfully and readID routing checks passed.

## Validation not performed and why

- Benchmarks were not run because M151 is a validation milestone.
- Real full-size production data was not run by this script; it validates the exact production command shape on a small pipeline-style sorted pairsam fixture and compares routing against Python pairtools.

## Cargo required

Yes. M151 changes tests, so guarded Cargo build is required to provide the candidate binary for the shell validation script.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M140: resume split core now that M151 is complete and the production-shaped dedup command validation remains part of the repository checks.
