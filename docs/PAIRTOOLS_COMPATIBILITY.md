# Pairtools Compatibility Inventory

Oracle source: installed Pixi `pairtools, version 1.1.3`, generated from:

```bash
pixi run pairtools --version
pixi run pairtools --help
pixi run pairtools <command> --help
```

Policy: pairtools is permitted only as a test oracle. The Rust binary must not call pairtools at runtime. Every accepted option must either match pairtools 1.1.3 semantics or fail loudly with `not implemented`.

Compatibility claims in this file are controlled by milestone-gated oracle tests. M000 added governance automation only. M010 adds CLI inventory and loud-failure tests only; it does not expand parse/sort behavior. Any stale or uncertain claim must be reconciled in a future milestone before it can support performance claims.

## Current Binary Classification

The current binary is a partial pairtools-compatible `parse`/`sort` implementation. It is not the older parse-lite prototype. Legacy parse-lite scripts remain in `scripts/` as historical artifacts and are not authoritative for current binary behavior.

Runtime code uses `rust-htslib`/HTSlib for SAM/BAM/CRAM input and BGZF output. The Rust runtime does not shell out to `pairtools`, `samtools`, `bgzip`, or `gzip`; those commands are used only in tests, benchmarks, and shell pipeline scripts.

## M000 Governance Note

M000 adds repository-enforced milestone automation only. It does not change Rust parse, sort, or downstream pairtools behavior. Parse/sort oracle parity was not rerun in M000, so this file records the previously reconciled compatibility baseline rather than new behavioral evidence from this milestone.

The M000 governance/bootstrap update added planned milestone JSON files for registry sync, autonomous runner work, result ledgers, split production validation, all-Rust pipeline orchestration, real-data oracle validation, and full-pipeline benchmarking. It also added `make codex-next` scaffolding and `milestone_results/` ledger scaffolding. These are governance changes only; no Rust runtime behavior or compatibility claim changed.

## M007 Registry Sync Note

M007 makes registry synchronization executable by extending `scripts/check_docs_sync.py` to fail when any `milestones/M*.json` file is missing from `milestones/README.md`, or when the README omits the rule that registry docs must stay synchronized with milestone JSON files. This is governance-only; it does not change Rust runtime behavior or pairtools compatibility status.

## M005 Autonomous Runner Note

M005 updates `scripts/codex_next.py` and `make codex-next` only. The runner now has explicit status, required-test execution, and chain-summary modes, and it refuses to fabricate validation results. This is governance-only; it does not change Rust runtime behavior or pairtools compatibility status.

## M006 Result Ledger Note

M006 adds `scripts/check_milestone_results.py`, wires result-ledger validation into milestone postflight, and makes `scripts/codex_report.py` report ledger presence. This is governance-only; it does not change Rust runtime behavior or pairtools compatibility status.

## M010 CLI Inventory Note

M010 verifies that the Rust CLI exposes the current command inventory in help text, that `parse --help` and `sort --help` expose the inventoried options, and that unsupported global options and unsupported commands fail loudly with `not implemented`. M010 does not implement downstream command behavior and does not modify parse or sort runtime semantics.

## M100 Downstream Planning Note

M100 defines the downstream command roadmap in `docs/DOWNSTREAM_MILESTONES.md` and activates M110 for a scoped `select` implementation. M100 does not implement merge, dedup, select, split, stats, or any other downstream Rust command behavior.

## M110 Select Note

M110 implements a scoped `select` command for exact `pair_type == "VALUE"` predicates. The oracle tests cover `pairs-rs select '(pair_type == "UU")'` on small `.pairs` and `.pairsam` fixtures, `-o/--output`, BGZF `.gz` output, and loud failures for unsupported predicates/options. M110 does not implement the full pairtools expression engine.

## M120 Merge Note

M120 implements a scoped `merge` command for small already sorted `.pairs` and `.pairsam` inputs. The oracle tests cover `pairs-rs merge` on committed sorted fixtures, compatible header handling for a small pairsam fixture, `-o/--output`, BGZF `.gz` output, and loud failures for unsupported merge options. M120 does not implement pairtools merge parallelism, temporary intermediate merging, concatenation mode, custom compression commands, or broad header edge cases.

## M150 Dedup Note

M150 implements a scoped `dedup` command for sorted `.pairs` and `.pairsam` inputs. The oracle tests compare read routing for nodups, duplicates, and unmapped records against installed Python pairtools on a committed fixture. M150 also tests `.gz` duplicate/unmapped output, `pair_type` `DD` marking, feasible pairsam SAM duplicate flag/Yt tag updates, simple stats output, and loud failures for unsupported dedup options. Full pairtools dedup stats, backend behavior, parent ID handling, extra-column duplicate matching, by-tile stats, filtering, YAML output, and custom input/output shell commands are not claimed.

## M151 Dedup Production Command Validation Note

M151 adds `tests/scripts/test_dedup_pipeline_command_shape.sh`, a production-shaped shell validation that runs the exact class of command used by the real pipeline:

```bash
pairs-rs dedup \
  --mark-dups \
  --output-stats merged.dedup.s01.RS.stats.txt \
  --output-dups merged.dups.pairsam.s01.RS.gz \
  --output-unmapped merged.unmapped.pairsam.s01.RS.gz \
  -o nodups.parse_RS_s01.sorted.pairsam \
  H1_ALL_parse_RS_1.sorted_2.pairsam
```

The script uses a small pipeline-style sorted pairsam fixture with the production input basename, validates the expected output files, validates compressed duplicate/unmapped streams, checks required simple stats fields, checks duplicate `pair_type` `DD`, checks pairsam SAM duplicate flag `0x400` and `Yt:Z:DD` where SAM columns exist, and compares readID routing against Python pairtools dedup on the same fixture. This supports a scoped sorted-input dedup routing claim only; it is not a full pairtools dedup parity or optimization claim.

## M130 Stats Note

M130 implements a scoped `stats` command for stable small-fixture counts. The oracle tests compare total, mapped/unmapped/single-sided, duplicate/nodup, cis/trans, pair-type, cis-threshold, fraction, chromosome-frequency, and `--with-chromsizes` fields against installed Python pairtools. M130 also tests `-o/--output`, BGZF `.gz` stats output, and loud failures for unsupported stats options. Full pairtools stats merge mode, distance-frequency sections, YAML output, filters, chrom subsets, by-tile duplicate statistics, type casts, custom input/output shell commands, and every derived summary field are not claimed.

## M131-M132 Stats Report, Merge, and I/O Notes

M131 extends `pairs-rs stats` to the pairtools-style report surface for committed small fixtures. The oracle tests compare the full normalized TSV report against installed Python pairtools for default output, `--no-chromsizes`, and `--n-dist-bins-decade 1`. The report includes distance-frequency bins, convergence summary fields, chromosome sizes by default, chromosome frequencies, pair-type counts, duplicate counts, and derived fractions. `summary/complexity_naive` is compared numerically within tolerance because Rust uses a local Lambert W implementation while pairtools uses SciPy.

M132 adds TSV stats merge, YAML output, and HTSlib BGZF threaded `.gz` stats input/output. `--nproc-in` and `--nproc-out` are implemented for BGZF streams through `rust-htslib`/HTSlib and do not shell out. `--cmd-in` and `--cmd-out` remain explicitly unsupported because Rust runtime shell compression is not allowed. `--merge --yaml` is also explicitly unsupported.

## M140 Split Note

M140 implements scoped `pairs-rs split` for small pairsam inputs. The tested surface supports `--output-pairs`, `--output-sam`, optional input path or stdin, file/stdout routing, `.gz` pairs output through HTSlib BGZF, and SAM reconstruction from `sam1`/`sam2` pairsam fields. Oracle tests compare pairs and SAM output against Python pairtools on `tests/data/mock.pairsam` after normalizing volatile split `@PG` command text. `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `.lz4`, and BAM `--output-sam` remain explicitly unsupported.

## M141 Split Production Validation Note

M141 adds `tests/scripts/test_split_pipeline_command_shape.sh`, which validates the production-shaped split contract on a small pairsam fixture. The script feeds gzipped pairsam into `pairs-rs split`, writes `merged.valid.pairs.gz`, captures `--output-sam -`, validates body rows, checks SAM parseability with samtools when available, and compares normalized pairs/SAM content and readID routing against Python pairtools split on the same fixture. This is validation of the scoped M140 split surface, not a broader split parity claim.

## M160 All-Rust Pipeline Orchestration Note

M160 adds `scripts/run_hic_all_rust_pairs_rs_pipeline.sh`, a shell orchestration script that uses `pairs-rs` for every pairtools-equivalent stage that currently has a scoped Rust implementation: parse, sort, merge, dedup, select, split, and stats. The script still uses `bwa-mem2` for alignment and `samtools` for BAM conversion, coordinate sorting, quickcheck, and indexing.

`tests/scripts/test_all_rust_hic_pipeline_dry_run.sh` validates the one-lane and two-lane dry-run command graph and confirms that Python pairtools is not present in the planned commands. This is dry-run orchestration coverage only. Production parity and final-output equivalence remain blocked on M161 real-data oracle validation.


## M162 Cross-Tool Threading Validation Note

M162 adds `tests/scripts/test_cross_tool_threading_contract.sh`, a behavioral validation of the current threading option contract. The test checks that `pairs-rs sort --nproc 1` and `--nproc 4` produce identical decompressed BGZF output on a generated fixture large enough to exercise chunk sorting, that `pairs-rs stats --nproc-in/--nproc-out` produces identical decompressed output for single-threaded and threaded BGZF paths, and that the all-Rust pipeline dry-run propagates `SORT_THREADS` to `pairs-rs sort` and `samtools` command lines. It also checks that unsupported threaded options for parse, merge, dedup, select, and split fail loudly. M162 does not add runtime behavior and does not claim CPU utilization, throughput, or speedup.

## M161 Real-Data Oracle Status

M161 adds `tests/scripts/test_all_rust_pipeline_real_oracle.sh` as the all-Rust real-data validation harness. The harness discovers the external fixture directory, required FASTQs, chrom sizes, assembly/MAPQ provenance, BWA index prefix, and exact pairtools-generated `merged.*` oracle outputs before running the all-Rust pipeline. When the oracle set is incomplete, it prints expected inputs, expected oracle files, expected candidate outputs, a pairtools oracle-generation command, and an all-Rust candidate command before exiting nonzero.

The current external directory `/mnt/d/pairtools_RS_test` is not sufficient for M161 validation. It is missing the exact `merged.sorted.pairsam.gz`, `merged.nodups.pairsam.gz`, `merged.dups.pairsam.gz`, `merged.unmapped.pairsam.gz`, `merged.valid.pairsam.gz`, `merged.valid.pairs.gz`, `merged.valid.stats.txt`, and BWA index prefix required for final-output comparison. No all-Rust real-data parity claim is made.

The same directory now contains useful non-canonical stage artifacts. Running the M161 harness with `RUN_AVAILABLE_STAGE_COMPARISONS=1` validates those files diagnostically:

- `s01.RS.parse.stats.txt` and `parse_RS.stats.txt` match `parse_stats_STANDARD_s01_pairtools.txt` after allowing only the known `summary/complexity_naive` `nan`/`inf` representation difference.
- The available dedup stats expose a blocker: pairtools reports `total_dups=29706` and `total_nodups=5733319`, while the available pairs-rs stats report `total_dups=29690` and `total_nodups=5733335`.

These stage-artifact checks do not complete M161. Canonical `merged.*` pairtools oracle files and a full semantic all-Rust output comparison remain required.

## M020 Parse I/O Note

M020 adds tests for parse input and writer plumbing without changing pair formation semantics. The tested parse I/O baseline is:

- SAM input from a path and the same SAM bytes from stdin produce identical output.
- A BAM file generated from the SAM fixture through rust-htslib produces identical output to the SAM path.
- `-o` writes pairs output to a file and leaves stdout empty.
- `--output-stats` writes parse stats to a file.
- Compressed parse output and compressed parse stats output fail loudly with `not implemented`.

CRAM-specific reference handling is not claimed by M020 tests. The runtime input path remains rust-htslib/HTSlib rather than shelling out to samtools.

## M030-M050 Parse Milestone Status

M030, M040, and M050 are marked complete based on the existing guarded oracle suite.

M030 core-pair coverage includes simple UU, unmapped, low-MAPQ, reverse-strand 5'/3' coordinate reporting, interchromosomal flipping, and same-chromosome position flipping.

M040 pairsam coverage includes scoped pairsam SAM columns, supported `--add-columns mapq,pos5,pos3,cigar,read_len`, parse stats output, and loud rejection of unsupported add-columns.

M050 walks/chimeric-limit coverage includes secondary and supplementary alignment fixtures, BWA-MEM2-style leading soft-clipped split behavior under `--max-inter-align-gap`, and loud rejection of unsupported walk policies. Full complex-walk or `parse2` parity is not claimed.

M055 expands `pairtools parse --walks-policy` support for deterministic walk-resolution fixtures. The supported non-`all` policies are `mask`, `5any`, `5unique`, `3any`, and `3unique`. The oracle suite compares pairsam rows, pair-type counts, and parse stats against pairtools-generated outputs for simple non-walk pairs, single-side chimeras with and without rescue, both-side chimeric walks, long-gap null insertion, and no-unique fallback cases.

M056 adds `--walks-policy all` support for the committed walk oracle fixtures. The suite now covers all six supported walk policies across 14 case/threshold combinations, including adjacent internal walk-edge emission, terminal R1/R2 bridge emission, a three-alignment R1 walk, inserted null segments, multi-mapping segments, and both-side 2x2 chimeric walks. This is oracle parity for the committed deterministic fixtures, not a claim that every possible pairtools complex-walk corner case has been exhausted.

## Supported Hybrid Pipeline Contract

M080 supports an exact shell-orchestrated hybrid pipeline in `scripts/run_hic_exact_pairs_rs_pipeline.sh`. The supported contract is:

- `bwa-mem2 mem -5SPM -T 30` produces the alignment stream.
- `pairs-rs parse` replaces `pairtools parse` with the target flags: `--chroms-path`, `--assembly`, `--min-mapq`, `--walks-policy 5unique`, `--max-inter-align-gap 30`, `--report-alignment-end 5`, `--add-columns mapq,pos5,pos3,cigar,read_len`, and `--output-stats`.
- `pairs-rs sort` replaces `pairtools sort` with `--nproc`, `--tmpdir`, and `-o *.sorted.pairsam.gz`.
- For one lane, the sorted pairsam is symlinked to `merged.sorted.pairsam.gz`.
- For multiple lanes, `pairtools merge` creates `merged.sorted.pairsam.gz`.
- Downstream `pairtools dedup`, `pairtools select`, `pairtools split`, `samtools view/sort/index`, and `pairtools stats` produce the requested `merged.*` outputs.

This is not an all-Rust pipeline. The M080 script still intentionally calls pairtools for downstream shell steps. Later scoped Rust milestones have added `select`, `merge`, `dedup`, and `stats`, but the exact M080 production script has not been rewritten to use those Rust downstream commands.

## External Real-Data Oracle Status

External data was discovered in `/mnt/d/pairtools_RS_test`. The directory contains a BWA-MEM2 aligned input, chrom sizes, command provenance, stats files, and a legacy pairtools sorted `.pairs` output.

The discovered sorted oracle is not an exact M080 `.pairsam.gz` oracle. The documented legacy command in `p3.commands` used `--drop-sam` and `--min-mapq 1`, while M080 targets `.pairsam.gz` with SAM columns, extra columns, and MAPQ 10 from `pairtools_1.sh`. Therefore full real-data parity is not claimed from that file.

`bash tests/scripts/test_hic_exact_pipeline_real_oracle.sh` passed on the external aligned input by generating candidate `pairs-rs parse | pairs-rs sort` output and verifying 11,359,961 sorted pairsam rows in pairtools-compatible key order. Exact semantic comparison against pairtools `.sorted.pairsam.gz` and final downstream output comparison were not run because those exact oracle files were not present.

If an exact pairtools `.sorted.pairsam.gz` oracle and downstream `merged.*` outputs are added to `/mnt/d/pairtools_RS_test`, `tests/scripts/test_hic_exact_pipeline_real_oracle.sh` can compare semantic decompressed pairsam content and optional downstream outputs. Until then, M080 claims only the exact shell pipeline contract and dry-run/validation coverage, not final all-output oracle parity.


## Planned Compatibility Expansion Milestones

The following milestones are planned only; they do not expand current compatibility claims until implemented and oracle-tested:

| Milestone | Planned scope |
|---|---|
| M170 | `flip` core |
| M171 | `markasdup` core |
| M180 | broader safe `select` expression engine |
| M190 | advanced `merge` options and header/temp behavior |
| M191 | dedup parent IDs, extra-column matching, and richer stats |
| M192 | stats filters, by-tile reports, type casts, and chrom subsets |
| M193 | sort custom columns and memory semantics |
| M194 | cross-command threaded BGZF I/O expansion |
| M200 | `filterbycov` core |
| M210 | `restrict` core |
| M220 | `sample` core |
| M230 | `header` subcommands |
| M240 | `parse2` core |
| M250 | `phase` core |
| M260 | `scaling` core |

These entries are roadmap items, not implemented behavior. Pairtools remains an oracle-only dependency for the corresponding future tests.

## Top-Level Options

| Option | Rust status |
|---|---|
| `--post-mortem` | explicitly not implemented |
| `--output-profile` | explicitly not implemented |
| `-v`, `--verbose` | explicitly not implemented |
| `-d`, `--debug` | explicitly not implemented |
| `--version` | implemented |
| `-h`, `--help` | implemented by CLI parser |

## Commands

| Command | Rust status |
|---|---|
| `parse` | partial, oracle-gated subset |
| `sort` | partial, oracle-gated multithreaded default sort |
| `dedup` | partial, oracle-gated sorted input routing |
| `filterbycov` | explicitly not implemented |
| `flip` | explicitly not implemented |
| `header` | explicitly not implemented |
| `markasdup` | explicitly not implemented |
| `merge` | partial, oracle-gated small sorted input merge |
| `parse2` | explicitly not implemented |
| `phase` | explicitly not implemented |
| `restrict` | explicitly not implemented |
| `sample` | explicitly not implemented |
| `scaling` | explicitly not implemented |
| `select` | partial, oracle-gated exact `pair_type` equality |
| `split` | partial, oracle-gated pairsam split |
| `stats` | partial, oracle-gated report, merge, YAML, and BGZF I/O |

## `parse`

Arguments: optional `SAM_PATH`.

| Option | Rust status |
|---|---|
| `-c`, `--chroms-path` | tested oracle parity |
| `-o`, `--output` | implemented for uncompressed output; compressed `.gz` and `.lz4` explicitly not implemented |
| `--assembly` | implemented |
| `--min-mapq` | tested oracle parity |
| `--max-molecule-size` | tested oracle parity for M055 single-ligation rescue fixtures |
| `--drop-readid` | explicitly not implemented |
| `--drop-seq` | explicitly not implemented |
| `--drop-sam` | tested oracle parity; pairsam output is also supported when omitted |
| `--add-pair-index` | explicitly not implemented |
| `--add-columns` | tested oracle parity only for `mapq,pos5,pos3,cigar,read_len`; all other values explicitly not implemented |
| `--output-parsed-alignments` | explicitly not implemented |
| `--output-stats` | tested oracle parity for parse-time PairCounter TSV output |
| `--report-alignment-end` | tested oracle parity for `5` and `3` |
| `--max-inter-align-gap` | tested oracle parity for supported parse walk fixtures |
| `--walks-policy` | tested oracle parity for `mask`, `5any`, `5unique`, `3any`, `3unique`, and `all` on committed walk fixtures |
| `--readid-transform` | explicitly not implemented |
| `--flip` | implemented as default flipping behavior |
| `--no-flip` | explicitly not implemented |
| `--nproc-in` | explicitly not implemented |
| `--nproc-out` | explicitly not implemented |
| `--cmd-in` | explicitly not implemented |
| `--cmd-out` | explicitly not implemented |

Previously recorded parse oracle fixtures cover small SAM inputs for UU pairs, unmapped and low-MAPQ mates, reverse-strand 5'/3' coordinates, soft/hard clipping, indel reference span, interchromosomal and same-chromosome flipping, secondary alignments, supplementary alignments, pairsam SAM columns, supported extra columns, parse-time stats, a BWA-MEM2-style leading soft-clipped split affected by `--max-inter-align-gap`, and loud rejection of non-adjacent repeated read names. These fixtures were not rerun in M000.

Known correctness limitations:
- Input must be query-name grouped: all SAM/BAM/CRAM records for a read name must be adjacent. Non-adjacent repeated read names fail loudly with `not implemented`.
- Compressed parse output and compressed parse stats output are not implemented.
- Only `mapq,pos5,pos3,cigar,read_len` are accepted for `--add-columns`.

## `sort`

Arguments: optional `PAIRS_PATH`.

| Option | Rust status |
|---|---|
| `-o`, `--output` | implemented for uncompressed output and `.gz`; `.lz4` explicitly not implemented |
| `--c1` | explicitly not implemented |
| `--c2` | explicitly not implemented |
| `--p1` | explicitly not implemented |
| `--p2` | explicitly not implemented |
| `--pt` | explicitly not implemented |
| `--extra-col` | explicitly not implemented |
| `--nproc` | implemented as the Rayon chunk-sort pool size and BGZF compression thread count for `.gz` output; `0` is rejected |
| `--tmpdir` | implemented |
| `--memory` | explicitly not implemented |
| `--compress-program` | explicitly not implemented |
| `--nproc-in` | explicitly not implemented |
| `--nproc-out` | explicitly not implemented |
| `--cmd-in` | explicitly not implemented |
| `--cmd-out` | explicitly not implemented |

M060 reran the guarded oracle suite and closed sort core coverage for default column sorting, parse-generated `.pairsam` with `sam1`, `sam2`, and supported parse extra columns, header preservation with `#sorted: chr1-chr2-pos1-pos2` insertion, stable ordering of equal keys across spilled chunks, identical `--nproc 1` and `--nproc 8` output, and loud rejection of unsupported sort options.

M070 reran the guarded suite with BGZF-compatible `.gz` output checks using `gzip -dc` and `bgzip -t`, equivalent decompressed `.gz` output for `--nproc 1` and `--nproc 8`, and a direct `--tmpdir` tripwire test that fails if sort spill files ignore the requested temporary directory. The runtime uses HTSlib `bgzf_mt` when `--nproc > 1` for `.gz` output. M070 does not claim measured compression throughput or CPU utilization; `scripts/benchmark_sort_threads.sh` is a harness for active milestone M090.

M090 validates the benchmark harness shape only. `scripts/benchmark_sort_threads.sh` reports wall time, CPU utilization, max RSS, temp disk usage, compressed and uncompressed output sizes, and compression throughput when run, and includes a compression-dominates mode for compression-heavy output. No benchmark was run in M090, so this document makes no speedup claim.

## `dedup`

Arguments: optional `PAIRS_PATH`.

| Option | Rust status |
|---|---|
| `-o`, `--output` | implemented for nodup output to stdout, plain files, and `.gz` BGZF output |
| `--output-dups` | implemented for duplicate output to plain files and `.gz` BGZF output |
| `--output-unmapped` | implemented for unmapped output to plain files and `.gz` BGZF output |
| `--output-stats` | implemented as simple append-only TSV counts: total, mapped, unmapped, duplicates, nodups, and fraction duplicates |
| `--output-bytile-stats` | explicitly not implemented |
| `--max-mismatch` | implemented for non-negative integer mismatch thresholds |
| `--method` | implemented for `max` and `sum`; other values fail loudly |
| `--backend` | explicitly not implemented |
| `--chunksize` | explicitly not implemented |
| `--carryover` | explicitly not implemented |
| `-p`, `--n-proc` | explicitly not implemented |
| `--mark-dups` | implemented; duplicates written to `--output-dups` are marked `DD` |
| `--no-mark-dups` | implemented |
| `--keep-parent-id` | explicitly not implemented |
| `--extra-col-pair` | explicitly not implemented |
| `--sep` | tab separator implemented; non-tab and multi-character separators fail loudly |
| `--send-header-to` | implemented for `dups`, `dedup`, `both`, and `none` |
| `--c1`, `--c2`, `--p1`, `--p2` | implemented for column names or numeric indices |
| `--s1`, `--s2` | explicitly not implemented |
| `--unmapped-chrom` | implemented |
| `--yaml`, `--no-yaml` | explicitly not implemented |
| `--filter`, `--engine`, `--chrom-subset`, `--startup-code`, `-t`/`--type-cast` | explicitly not implemented |
| `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` | explicitly not implemented |

Known correctness limitations:
- Input must already be sorted by pairtools sort semantics.
- Duplicate clusters keep the first retained mapped record as the parent.
- Duplicate detection is based on `chrom1`, `chrom2`, `pos1`, and `pos2`; extra-column duplicate matching is not implemented.
- Stats are intentionally simple and are not full pairtools stats parity.

## `stats`

Arguments: zero or more `INPUT_PATH` values.

| Option | Rust status |
|---|---|
| `-o`, `--output` | implemented for stdout, plain files, and `.gz` BGZF output |
| `--merge` | implemented for TSV stats files; `--merge --yaml` explicitly not implemented |
| `--n-dist-bins-decade` | implemented for tested values, including `1` and default `8` |
| `--with-chromsizes` | implemented from `#chromsize:` header lines and matches pairtools default behavior when chromsizes are present |
| `--no-chromsizes` | implemented |
| `--yaml`, `--no-yaml` | YAML output implemented for normal stats reports; YAML merge explicitly not implemented |
| `--bytile-dups` | explicitly not implemented |
| `--no-bytile-dups` | accepted as the default no-bytile mode; by-tile output itself is not implemented |
| `--output-bytile-stats` | explicitly not implemented |
| `--filter`, `--engine`, `--chrom-subset`, `--startup-code`, `-t`/`--type-cast` | explicitly not implemented |
| `--nproc-in`, `--nproc-out` | implemented for HTSlib BGZF `.gz` input/output; `0` is rejected |
| `--cmd-in`, `--cmd-out` | explicitly not implemented |

M131/M132 output includes the full tested pairtools-style report for the committed stats fixture. It emits total rows, unmapped and single-sided mapped rows, mapped rows, duplicates, nodups, cis/trans nodup counts, pair-type counts, cis-threshold counts, summary fractions, naive library complexity, convergence fields, chromosome-frequency counts, distance-frequency bins, and optional chromosome sizes. Compatibility beyond committed fixtures is not claimed until additional oracle cases are added.

## Other Command Inventories

These commands are present so they fail loudly instead of looking absent. Their pairtools 1.1.3 options are inventoried here. Rows for scoped partial implementations say so explicitly; otherwise the Rust implementation rejects the command as not implemented.

| Command | Arguments | Options |
|---|---|---|
| `filterbycov` | optional `PAIRS_PATH` | `-o`/`--output`, `--output-highcov`, `--output-unmapped`, `--output-stats`, `--max-cov`, `--max-dist`, `--method`, `--sep`, `--comment-char`, `--send-header-to`, `--c1`, `--c2`, `--p1`, `--p2`, `--s1`, `--s2`, `--unmapped-chrom`, `--mark-multi`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `flip` | optional `PAIRS_PATH` | `-c`/`--chroms-path`, `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `markasdup` | optional `PAIRSAM_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `merge` | zero or more `PAIRS_PATH` values | `-o`/`--output` implemented for small sorted `.pairs`/`.pairsam` inputs; `--max-nmerge`, `--tmpdir`, `--memory`, `--compress-program`, `--nproc`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `--keep-first-header`/`--no-keep-first-header`, and `--concatenate`/`--no-concatenate` explicitly not implemented |
| `parse2` | optional `SAM_PATH` | `-c`/`--chroms-path`, `-o`/`--output`, `--report-position`, `--report-orientation`, `--assembly`, `--min-mapq`, `--max-inter-align-gap`, `--max-insert-size`, `--dedup-max-mismatch`, `--single-end`, `--expand`/`--no-expand`, `--max-expansion-depth`, `--add-pair-index`, `--flip`/`--no-flip`, `--add-columns`, `--drop-readid`/`--keep-readid`, `--readid-transform`, `--drop-seq`/`--keep-seq`, `--drop-sam`/`--keep-sam`, `--output-parsed-alignments`, `--output-stats`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `phase` | optional `PAIRS_PATH` | `-o`/`--output`, `--phase-suffixes`, `--clean-output`, `--tag-mode`, `--report-scores`/`--no-report-scores`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `restrict` | optional `PAIRS_PATH` | `-f`/`--frags`, `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `sample` | required `FRACTION`, optional `PAIRS_PATH` | `-o`/`--output`, `-s`/`--seed`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `scaling` | zero or more `INPUT_PATH` values | `-o`/`--output`, `--view`/`--regions`, `--chunksize`, `--dist-range`, `--n-dist-bins-decade`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `select` | required `CONDITION`, optional `PAIRS_PATH` | `-o`/`--output` implemented for exact `pair_type == "VALUE"` predicates; `--output-rest`, `--chrom-subset`, `--startup-code`, `-t`/`--type-cast`, `-r`/`--remove-columns`, `--nproc-in`, `--nproc-out`, `--cmd-in`, and `--cmd-out` explicitly not implemented |
| `split` | optional `PAIRSAM_PATH` | `--output-pairs` and `--output-sam` implemented for scoped pairsam splitting; `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `.lz4`, and BAM output explicitly not implemented |

## `header` Subcommands

The `header` command is explicitly not implemented. Pairtools 1.1.3 exposes these subcommands and options:

| Header subcommand | Arguments | Options |
|---|---|---|
| `header generate` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `--chroms-path`, `--sam-path`, `--columns`, `--extra-columns`, `--assembly`, `--no-flip`, `--pairs`/`--pairsam` |
| `header transfer` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `-r`/`--reference-file` |
| `header set-columns` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `-c`/`--columns` |
| `header validate-columns` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `-r`/`--reference-file`, `-c`/`--reference-columns` |
