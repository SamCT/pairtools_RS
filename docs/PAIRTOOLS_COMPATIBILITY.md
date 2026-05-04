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

## M010 CLI Inventory Note

M010 verifies that the Rust CLI exposes the current command inventory in help text, that `parse --help` and `sort --help` expose the inventoried options, and that unsupported global options and unsupported commands fail loudly with `not implemented`. M010 does not implement downstream command behavior and does not modify parse or sort runtime semantics.

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
| `dedup` | explicitly not implemented |
| `filterbycov` | explicitly not implemented |
| `flip` | explicitly not implemented |
| `header` | explicitly not implemented |
| `markasdup` | explicitly not implemented |
| `merge` | explicitly not implemented |
| `parse2` | explicitly not implemented |
| `phase` | explicitly not implemented |
| `restrict` | explicitly not implemented |
| `sample` | explicitly not implemented |
| `scaling` | explicitly not implemented |
| `select` | explicitly not implemented |
| `split` | explicitly not implemented |
| `stats` | explicitly not implemented |

## `parse`

Arguments: optional `SAM_PATH`.

| Option | Rust status |
|---|---|
| `-c`, `--chroms-path` | tested oracle parity |
| `-o`, `--output` | implemented for uncompressed output; compressed `.gz` and `.lz4` explicitly not implemented |
| `--assembly` | implemented |
| `--min-mapq` | tested oracle parity |
| `--max-molecule-size` | explicitly not implemented |
| `--drop-readid` | explicitly not implemented |
| `--drop-seq` | explicitly not implemented |
| `--drop-sam` | tested oracle parity; pairsam output is also supported when omitted |
| `--add-pair-index` | explicitly not implemented |
| `--add-columns` | tested oracle parity only for `mapq,pos5,pos3,cigar,read_len`; all other values explicitly not implemented |
| `--output-parsed-alignments` | explicitly not implemented |
| `--output-stats` | tested oracle parity for parse-time PairCounter TSV output |
| `--report-alignment-end` | tested oracle parity for `5` and `3` |
| `--max-inter-align-gap` | tested oracle parity for supported `5unique` walks |
| `--walks-policy` | tested oracle parity only for default `5unique`; all other values explicitly not implemented |
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
- `--walks-policy` support is limited to `5unique`; other walk policies fail loudly.
- `--max-molecule-size` is not configurable yet; rescue logic currently uses the built-in pairtools default of 750 bp.
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

Previously recorded sort oracle coverage includes default column sorting, parse-generated `.pairsam` with `sam1`, `sam2`, and supported parse extra columns, header preservation with `#sorted: chr1-chr2-pos1-pos2` insertion, stable ordering of equal keys across spilled chunks, identical `--nproc 1` and `--nproc 8` output, BGZF-compatible `.gz` output validated by `gzip -dc` and `bgzip -t`, and equivalent decompressed `.gz` output for `--nproc 1` and `--nproc 8`. These checks were not rerun in M000. `scripts/benchmark_sort_threads.sh` is a previously added harness; M000 does not run benchmarks or add performance evidence.

## Other Command Inventories

These commands are present so they fail loudly instead of looking absent. Their pairtools 1.1.3 options are inventoried here, but the Rust implementation currently rejects the command as explicitly not implemented.

| Command | Arguments | Options |
|---|---|---|
| `dedup` | optional `PAIRS_PATH` | `-o`/`--output`, `--output-dups`, `--output-unmapped`, `--output-stats`, `--output-bytile-stats`, `--max-mismatch`, `--method`, `--backend`, `--chunksize`, `--carryover`, `-p`/`--n-proc`, `--mark-dups`/`--no-mark-dups`, `--keep-parent-id`, `--extra-col-pair`, `--sep`, `--send-header-to`, `--c1`, `--c2`, `--p1`, `--p2`, `--s1`, `--s2`, `--unmapped-chrom`, `--yaml`/`--no-yaml`, `--filter`, `--engine`, `--chrom-subset`, `--startup-code`, `-t`/`--type-cast`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `filterbycov` | optional `PAIRS_PATH` | `-o`/`--output`, `--output-highcov`, `--output-unmapped`, `--output-stats`, `--max-cov`, `--max-dist`, `--method`, `--sep`, `--comment-char`, `--send-header-to`, `--c1`, `--c2`, `--p1`, `--p2`, `--s1`, `--s2`, `--unmapped-chrom`, `--mark-multi`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `flip` | optional `PAIRS_PATH` | `-c`/`--chroms-path`, `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `markasdup` | optional `PAIRSAM_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `merge` | zero or more `PAIRS_PATH` values | `-o`/`--output`, `--max-nmerge`, `--tmpdir`, `--memory`, `--compress-program`, `--nproc`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `--keep-first-header`/`--no-keep-first-header`, `--concatenate`/`--no-concatenate` |
| `parse2` | optional `SAM_PATH` | `-c`/`--chroms-path`, `-o`/`--output`, `--report-position`, `--report-orientation`, `--assembly`, `--min-mapq`, `--max-inter-align-gap`, `--max-insert-size`, `--dedup-max-mismatch`, `--single-end`, `--expand`/`--no-expand`, `--max-expansion-depth`, `--add-pair-index`, `--flip`/`--no-flip`, `--add-columns`, `--drop-readid`/`--keep-readid`, `--readid-transform`, `--drop-seq`/`--keep-seq`, `--drop-sam`/`--keep-sam`, `--output-parsed-alignments`, `--output-stats`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `phase` | optional `PAIRS_PATH` | `-o`/`--output`, `--phase-suffixes`, `--clean-output`, `--tag-mode`, `--report-scores`/`--no-report-scores`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `restrict` | optional `PAIRS_PATH` | `-f`/`--frags`, `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `sample` | required `FRACTION`, optional `PAIRS_PATH` | `-o`/`--output`, `-s`/`--seed`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `scaling` | zero or more `INPUT_PATH` values | `-o`/`--output`, `--view`/`--regions`, `--chunksize`, `--dist-range`, `--n-dist-bins-decade`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `select` | required `CONDITION`, optional `PAIRS_PATH` | `-o`/`--output`, `--output-rest`, `--chrom-subset`, `--startup-code`, `-t`/`--type-cast`, `-r`/`--remove-columns`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `split` | optional `PAIRSAM_PATH` | `--output-pairs`, `--output-sam`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |
| `stats` | zero or more `INPUT_PATH` values | `-o`/`--output`, `--merge`, `--n-dist-bins-decade`, `--with-chromsizes`/`--no-chromsizes`, `--yaml`/`--no-yaml`, `--bytile-dups`/`--no-bytile-dups`, `--output-bytile-stats`, `--filter`, `--engine`, `--chrom-subset`, `--startup-code`, `-t`/`--type-cast`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out` |

## `header` Subcommands

The `header` command is explicitly not implemented. Pairtools 1.1.3 exposes these subcommands and options:

| Header subcommand | Arguments | Options |
|---|---|---|
| `header generate` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `--chroms-path`, `--sam-path`, `--columns`, `--extra-columns`, `--assembly`, `--no-flip`, `--pairs`/`--pairsam` |
| `header transfer` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `-r`/`--reference-file` |
| `header set-columns` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `-c`/`--columns` |
| `header validate-columns` | optional `PAIRS_PATH` | `-o`/`--output`, `--nproc-in`, `--nproc-out`, `--cmd-in`, `--cmd-out`, `-r`/`--reference-file`, `-c`/`--reference-columns` |
