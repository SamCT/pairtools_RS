# pairs-rs Status

Last reconciled: 2026-05-05

## Active milestone

M180: select expression engine.

M161 is deferred as nearly validated but not complete: parse stats match on available real-data artifacts, while dedup routing still differs and canonical `merged.*` oracle outputs/BWA index files are still missing. M162 is complete and recorded in `milestone_results/M162.json`.

## Current branch

`master`

## Current commit

`uncommitted` M180 select expression engine changes are in progress after M171 completion commit `d233d2d877e5b14f6ecae2b333545b2c5b07e60a` and M180 activation commit `bdd0ec21a579f662750dd9bd7975289aa4b788a4`.

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
- M180 Select expression engine:
  - `pairs-rs select` now supports a bounded safe expression subset: column references, string equality/inequality, numeric comparisons, `and`, `or`, `not`, and parentheses.
  - `--output-rest` writes rejected rows to a separate output while selected rows go to stdout or `-o`.
  - Oracle tests compare selected and rest outputs against Python pairtools on committed `.pairs` and `.pairsam` fixtures.
  - Python-specific expression features, startup code, chrom subsets, type casts, remove-columns, threaded/custom I/O options, and `.lz4` remain loud non-goals.
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
- M000 governance/bootstrap update:
  - Added planned milestone JSON files for M005, M006, M007, M141, M160, M161, and M300.
  - Added `make codex-next` through `scripts/codex_next.py` as a conservative active-milestone runner that lists required tests and fails until they are recorded.
  - Added `milestone_results/` ledger scaffolding for machine-readable milestone completion evidence.
  - Set `milestones/ACTIVE_MILESTONE` to M007 so registry sync is the next active milestone before M140 split core resumes.
- M007 registry sync:
  - `scripts/check_docs_sync.py` now checks that every `milestones/M*.json` file is listed in `milestones/README.md`.
  - The same check also verifies that the registry README contains the rule requiring milestone JSON files and registry docs to stay synchronized.
  - `milestone_results/M007.json` records the validation commands and points to commit `60759c2816a1655ce583e9bd9f62167fbaa1536d`.
  - This is governance-only and does not change Rust runtime behavior.
- M005 autonomous runner:
  - `scripts/codex_next.py` now has explicit `--status`, `--run-required-tests`, and `--chain` modes.
  - `make codex-next` still runs preflight, prints the milestone summary, checks recorded required tests, runs postflight and the report, and fails clearly if required validations are missing.
  - The runner refuses recursive required-test execution instead of fabricating test results.
  - `milestone_results/M005.json` records the validation commands and points to commit `aa12ad4d1c028c08f3bd1b69424d20ec6ca9a23a`.
- M006 result ledger:
  - Added `scripts/check_milestone_results.py` to validate `milestone_results/*.json` required fields, command records, pass/blocker consistency, and registered milestone IDs.
  - `scripts/milestone_gate.py` now runs the result-ledger validator during postflight.
  - `scripts/codex_report.py` now reports whether `milestone_results/<MILESTONE>.json` exists.
  - `milestone_results/M006.json` records the validation commands and points to commit `8e0ecfaca12a665a9dd3917b3f6600d1acefa7c3`.
- M140 Split core:
  - `pairs-rs split` now supports scoped pairsam splitting with `--output-pairs`, `--output-sam`, optional input path/stdin, and file/stdout routing.
  - Pairs output preserves all non-`sam1`/`sam2` columns and writes a pairs header with updated `#columns`.
  - SAM output restores `sam1` and `sam2` records from pairsam unit separators into tab-delimited SAM records.
  - Pairs `.gz` output and `.gz` input use HTSlib BGZF helpers; `.lz4`, custom compression commands, nproc I/O controls, and BAM output fail loudly.
  - Oracle tests compare split pairs and SAM output against Python pairtools on a committed small pairsam fixture after normalizing volatile split `@PG` command text.
  - `milestone_results/M140.json` records the validation commands and points to commit `ae4a341d2a0413a6c2408b73717c1f96403a0ce6`.
- M141 Split production validation:
  - `tests/scripts/test_split_pipeline_command_shape.sh` validates a production-shaped split command using gzipped pairsam input, gzipped pairs output, and SAM stream output.
  - The script verifies candidate command exit, pairs output existence and body rows, SAM stream body rows, optional samtools parsing, exact normalized pairs/SAM content against Python pairtools split, and readID routing.
  - The validation uses a small pipeline-style temporary fixture derived from `tests/data/mock.pairsam` with CIGAR strings adjusted to match toy sequence lengths for samtools parsing.
  - `milestone_results/M141.json` records the validation commands and points to commit `800c056311e53baa1b1349365adc8513ec147f77`.
- M160 All-Rust Hi-C pipeline orchestration:
  - `scripts/run_hic_all_rust_pairs_rs_pipeline.sh` orchestrates `bwa-mem2 mem` followed by `pairs-rs parse`, `pairs-rs sort`, `pairs-rs merge` for multiple lanes, `pairs-rs dedup`, `pairs-rs select`, `pairs-rs split`, `samtools view/sort/index/quickcheck`, and `pairs-rs stats`.
  - The script preserves the established production output names: per-lane sorted pairsam and parse stats, `merged.sorted.pairsam.gz`, `merged.nodups.pairsam.gz`, `merged.dups.pairsam.gz`, `merged.unmapped.pairsam.gz`, `merged.dedup.stats.txt`, `merged.valid.pairsam.gz`, `merged.valid.pairs.gz`, `merged.valid.coord.bam`, `merged.valid.coord.bam.bai`, and `merged.valid.stats.txt`.
  - `tests/scripts/test_all_rust_hic_pipeline_dry_run.sh` validates the full dry-run command graph for one-lane and two-lane inputs and checks that pairtools-equivalent stages use `pairs-rs` rather than Python pairtools.
  - M160 does not run real data and does not claim production parity.
- M161 Real-data oracle setup:
  - `tests/scripts/test_all_rust_pipeline_real_oracle.sh` deterministically discovers the external real-data directory, FASTQs, chrom sizes, assembly, MAPQ, BWA index prefix, and exact `merged.*` oracle files needed for all-Rust pipeline validation.
  - The harness now prints the expected external input directory, expected pairtools oracle files, expected all-Rust candidate output paths, a copy-pasteable command block to generate missing pairtools oracle outputs, and a copy-pasteable all-Rust candidate command block.
  - The harness can also inspect available non-canonical stage artifacts with `RUN_AVAILABLE_STAGE_COMPARISONS=1` without treating them as a full M161 pass.
  - The current external directory `/mnt/d/pairtools_RS_test` is incomplete for M161. It contains FASTQs, an aligned BAM, chrom sizes, provenance files, pairtools parse/dedup artifacts, and pairs-rs dedup/select/split/stats artifacts, but it is missing the exact pairtools-generated `merged.*` oracle outputs and a usable BWA index prefix.
  - Available artifact validation found that pairs-rs parse stats match pairtools parse stats after allowing only the known `summary/complexity_naive` `nan`/`inf` representation difference.
  - Available artifact validation found a real dedup count blocker: pairtools reports `total_dups=29706` and `total_nodups=5733319`, while the available pairs-rs dedup stats report `total_dups=29690` and `total_nodups=5733335`.
  - Available duplicate-output readID routing also differs: 6,953 duplicate readIDs are only in the pairtools duplicate output and 6,937 duplicate readIDs are only in the pairs-rs duplicate output.
  - The available split output is `rs_s01.outpairs.split.pairs`, a plain pairs text table. Treat it as the available split pairs artifact for M161 diagnostics; the `.pairs.gz` production name denotes the same semantic pairs table under compression and is still required for canonical final-output comparison.
  - M161 remains active and blocked; no all-Rust real-data parity claim is made.

- M170 Flip core:
  - `pairs-rs flip` implements scoped upper-triangle normalization for `.pairs`/`.pairsam` streams using `-c/--chroms-path`.
  - Oracle tests compare the committed `tests/data/mock.4flip.pairs` fixture against Python pairtools, including listed chromosomes, unannotated chromosomes, unmapped `!`, same-chromosome position flips, strand swaps, and pair-type reversal.
  - stdin/path input, `-o/--output`, and `.gz` BGZF output are tested.
  - `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, and `.lz4` remain loud non-goals.
- M171 Markasdup core:
  - `pairs-rs markasdup` marks every `.pairs`/`.pairsam` body row as duplicate by setting `pair_type` to `DD`.
  - Pairsam `sam1` and `sam2` columns are updated where present: SAM duplicate flag `0x400` is set and `Yt:Z:DD` is added or replaced.
  - stdin/path input, `-o`/`--output`, plain output, and `.gz` BGZF output are tested.
  - Oracle tests compare normalized output against Python pairtools on committed `.pairs` and `.pairsam` fixtures.
  - `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, and `.lz4` remain loud non-goals.
- M162 Cross-tool threading validation:
  - `tests/scripts/test_cross_tool_threading_contract.sh` validates the current thread-option contract across implemented tools without benchmarking.
  - Sort is checked for identical decompressed output with `--nproc 1` and `--nproc 4` on a generated fixture large enough to exercise chunk sorting.
  - Stats is checked for identical decompressed output with single-threaded and threaded BGZF input/output settings.
  - The all-Rust pipeline dry-run is checked for `SORT_THREADS` propagation into `pairs-rs sort` and `samtools` commands.
  - Parse, merge, dedup, select, split, sort, and stats unsupported or invalid threaded options are checked for loud failure rather than silent acceptance.
  - M162 makes no CPU utilization, throughput, or speedup claim.
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


## Planned implementation milestones

Additional planned milestones now cover the remaining pairtools command surface and parity expansions without changing runtime behavior in this planning task:

```text
M170 flip core
M171 markasdup core
M180 select expression engine
M190 advanced merge
M191 dedup parity expansion
M192 stats filters/bytile/chrom subsets
M193 sort custom columns/memory semantics
M194 cross-command threaded I/O
M200 filterbycov core
M210 restrict core
M220 sample core
M230 header subcommands
M240 parse2 core
M250 phase core
M260 scaling core
```

M171 is complete for committed oracle fixtures and M180 is now the active command milestone. M161 remains deferred with blocker notes in `milestone_results/M161.json`; M300 benchmarking remains blocked until real-data validation passes.

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- `select` supports a bounded safe subset: column references, string equality/inequality, numeric comparisons, `and`, `or`, `not`, and parentheses. Arbitrary Python expression execution and Python-specific method calls are not implemented.
- `merge` supports small sorted inputs only. Broad pairtools merge options such as `--nproc`, `--tmpdir`, `--memory`, `--compress-program`, `--keep-first-header`, and `--concatenate` remain explicitly unsupported.
- `dedup` does not yet implement full pairtools stats, by-tile stats, alternate backends, parent IDs, extra-column duplicate matching, filtering, YAML output, chrom subsets, type casts, custom input/output shell commands, optimization claims, or full pairtools parity beyond scoped sorted-input routing.
- `stats` does not yet implement YAML merge mode, expression filters, chrom subsets, by-tile duplicate statistics, type casts, custom compression shell commands, or broad uncommitted-fixture parity beyond the tested report surface.
- `markasdup` does not yet implement threaded input/output or custom shell compression commands.
- Remaining downstream commands such as `filterbycov`, `restrict`, `sample`, `header`, `parse2`, `phase`, and `scaling` remain unimplemented until their command-specific milestones land.
- The all-Rust pipeline is dry-run validated only until M161 real-data oracle validation passes.
- Compressed parse output and compressed parse stats output are not implemented.
- No benchmark or speedup is claimed by M056.

## Validation performed

Validation commands for M007:

```bash
git status -sb
git log --oneline -n 5
cat milestones/ACTIVE_MILESTONE
python3 scripts/milestone_gate.py pre --milestone M007
python3 scripts/check_milestone_schema.py
python3 scripts/check_docs_sync.py --milestone M007
python3 scripts/milestone_gate.py post --milestone M007
python3 scripts/codex_report.py --milestone M007
git diff --check
```

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

Validation commands for M160:

```bash
python3 scripts/milestone_gate.py pre --milestone M160
bash tests/scripts/test_all_rust_hic_pipeline_dry_run.sh
```

M160 dry-run validation passed and reported `all-Rust Hi-C pipeline dry-run validation passed`.

Validation commands for M162:

```bash
python3 scripts/milestone_gate.py pre --milestone M162
scripts/cargo_guard.sh build
bash tests/scripts/test_cross_tool_threading_contract.sh
```

M162 threading contract validation passed locally with `cross-tool threading contract validation passed`.

Validation commands for M161 setup:

```bash
python3 scripts/milestone_gate.py pre --milestone M161
bash tests/scripts/test_all_rust_pipeline_real_oracle.sh
RUN_AVAILABLE_STAGE_COMPARISONS=1 bash tests/scripts/test_all_rust_pipeline_real_oracle.sh
```

The M161 real-data oracle harness stopped before running the all-Rust pipeline because required external oracle files are missing:

- `/mnt/d/pairtools_RS_test/merged.sorted.pairsam.gz`
- `/mnt/d/pairtools_RS_test/merged.nodups.pairsam.gz`
- `/mnt/d/pairtools_RS_test/merged.dups.pairsam.gz`
- `/mnt/d/pairtools_RS_test/merged.unmapped.pairsam.gz`
- `/mnt/d/pairtools_RS_test/merged.valid.pairsam.gz`
- `/mnt/d/pairtools_RS_test/merged.valid.pairs.gz`
- `/mnt/d/pairtools_RS_test/merged.valid.stats.txt`
- a BWA index prefix with index files

The harness prints the exact `pairtools` oracle-generation command and the exact all-Rust candidate command before exiting nonzero.

With `RUN_AVAILABLE_STAGE_COMPARISONS=1`, the harness also consumed the newly provided stage artifacts. The parse stats comparisons passed against `parse_stats_STANDARD_s01_pairtools.txt` for both `s01.RS.parse.stats.txt` and `parse_RS.stats.txt`, allowing only the `summary/complexity_naive` `nan`/`inf` representation difference. The dedup stage comparison did not pass: pairtools reported 29,706 duplicate pairs and 5,733,319 nodup pairs, while pairs-rs reported 29,690 duplicate pairs and 5,733,335 nodup pairs. Duplicate-output readID routing also differs, with 6,953 readIDs only in the pairtools duplicate output and 6,937 readIDs only in the pairs-rs duplicate output.

The same artifact pass reports that `rs_s01.outpairs.split.pairs` exists as a plain pairs text table. That file should be treated as the available split pairs artifact; it is not a separate semantic format from the production `.pairs.gz` table.


Validation commands for M170:

```bash
python3 scripts/milestone_gate.py pre --milestone M170
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
```

M170 validation passed locally. The full guarded Rust test suite reported 46 `compat_oracle` tests and 1 `walks_oracle` test passing.

Validation commands for M171:

```bash
python3 scripts/milestone_gate.py pre --milestone M171
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
```

M171 validation passed locally. The full guarded Rust test suite reported 51 `compat_oracle` tests and 1 `walks_oracle` test passing.

Validation commands for the M000 transition from M171 to M180:

```bash
python3 scripts/milestone_gate.py pre --milestone M000 --allow-nonactive
python3 scripts/check_milestone_schema.py
python3 scripts/check_docs_sync.py --milestone M000
python3 scripts/check_cargo_needed.py --milestone M000
python3 scripts/milestone_gate.py post --milestone M000 --allow-nonactive
python3 scripts/codex_report.py --milestone M000
git diff --check
```

Validation commands for M180:

```bash
python3 scripts/milestone_gate.py pre --milestone M180
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
```

M180 validation passed locally. The full guarded Rust test suite reported 53 `compat_oracle` tests and 1 `walks_oracle` test passing.

## Validation not performed and why

- Benchmarks were not run because M162 is a validation-contract milestone and M161 real-data oracle validation has not passed.
- Real full-size production data was not run by this script; it validates the exact production command shape on a small pipeline-style sorted pairsam fixture and compares routing against Python pairtools.
- M161 full external validation was not run because `/mnt/d/pairtools_RS_test` is missing the exact all-Rust pipeline oracle outputs and BWA index files listed above.
- Full nodup/unmapped readID routing comparison on the full external artifacts was not run in this pass; duplicate-output readID routing already exposes a mismatch that must be investigated before claiming M161 parity.
- Benchmarks were not run for M171 because markasdup core is a correctness milestone, not a performance milestone.
- M171 was not validated on full production data; it is scoped to committed `.pairs` and `.pairsam` oracle fixtures.
- Benchmarks were not run for M180 because select expression expansion is a correctness milestone, not a performance milestone.
- M180 was not validated on full production data; it is scoped to committed `.pairs` and `.pairsam` oracle fixtures.
- The requested next-milestone planning and automation scaffolding was not added under M140 because the active milestone allows only `src/cli.rs`, `src/main.rs`, `src/split.rs`, `tests/**`, `docs/**`, `milestones/ACTIVE_MILESTONE`, and `milestones/M140-split-core.json`.
- M140 does not allow the required planning files: `milestones/README.md`, new milestone registry JSON files, `Makefile`, `milestone_results/**`, or new automation scripts. A planning/governance milestone such as M007 registry sync or M005 autonomous runner must become active before those files can be changed.
- That previous stop was correct: M140 forbids governance and automation files. The current task intentionally uses M000 with `--allow-nonactive` to bootstrap the governance files and then makes M007 active.

## Cargo required

M180 changed Rust source and tests. Cargo validation was required and run through `scripts/cargo_guard.sh check` and `scripts/cargo_guard.sh test`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

Recommended sequence after M007 completion:

```text
M005 -> M006 -> M140 -> M141 -> M160 -> M161 -> M300
```

M180 is active. Recommended implementation sequence is M180 -> M190 -> M191 -> M192 -> M193 -> M194 -> M200 -> M210 -> M220 -> M230 -> M240 -> M250 -> M260, with M300 benchmarking only after real-data validation. Add the exact pairtools-generated `merged.*` oracle outputs and BWA index prefix to `/mnt/d/pairtools_RS_test`, then rerun:

```bash
bash tests/scripts/test_all_rust_pipeline_real_oracle.sh
```

Do not claim all-Rust pipeline parity until M161 real-data oracle validation passes.

M300 full-pipeline benchmarking is blocked. The prerequisite `milestone_results/M161.json` exists, but it records `"passed": false`, and `milestones/ACTIVE_MILESTONE` remains `M161`. No benchmark was run for this reason.

Optimization remains blocked until M161 real-data oracle validation passes. Full pairtools parity is not claimed.
