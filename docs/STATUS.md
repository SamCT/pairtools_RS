# pairs-rs Status

Last reconciled: 2026-05-03

## Repository State

- Local checkout for Codex work: `/mnt/d/pairtools_RS` (Windows: `D:\pairtools_RS`).
- Current branch at reconciliation start: `master`.
- Current HEAD at reconciliation start: `b978769f2f357bda7390fdc9b87c41c68afa0d60`.
- Binary classification: partial pairtools-compatible `parse`/`sort` implementation.

## Implemented Behavior

- `pairs-rs parse` accepts SAM/BAM/CRAM input through `rust-htslib`/HTSlib.
- `pairs-rs parse` supports the currently oracle-tested subset: `--chroms-path`, `--assembly`, `--min-mapq`, default `--walks-policy 5unique`, `--report-alignment-end 5` and `3`, `--max-inter-align-gap`, `--drop-sam`, pairsam output, parse stats, and `--add-columns mapq,pos5,pos3,cigar,read_len`.
- `pairs-rs sort` supports default pairtools-compatible sort keys, parallel chunk sorting, stable equal-key output across chunks, `--nproc`, `--tmpdir`, uncompressed output, and BGZF-compatible `.gz` output through HTSlib.
- Unsupported pairtools commands are present in the CLI and fail loudly with `not implemented`.
- The Rust runtime does not call `pairtools` or `samtools`; pairtools is used only in tests, benchmarks, and shell pipeline scripts.

## Intentionally Unsupported Behavior

- `parse2`, `dedup`, `filterbycov`, `flip`, `header`, `markasdup`, `merge`, `phase`, `restrict`, `sample`, `scaling`, `select`, `split`, and `stats` are not implemented in Rust.
- Parse options outside the tested subset fail loudly rather than being accepted as no-ops.
- Sort custom column selection, compressed input, `.lz4` output, external compression commands, and memory tuning are not implemented.

## Known Correctness Limitations

- Parse input must be query-name grouped. Non-adjacent repeated read names are rejected loudly; a full name-collating parser has not been implemented.
- Parse walk handling is limited to the supported `5unique` subset and does not yet cover full pairtools `parse`/`parse2` behavior.
- The repository still contains legacy parse-lite scripts/docs under `scripts/`; they are historical and not the current binary direction.
- Compatibility and performance claims are limited to the oracle tests and benchmark scripts currently checked into the repository.

## Validation Policy

- Before Cargo commands, inspect active Cargo/Rust/native build processes.
- Use `export CARGO_TARGET_DIR="$HOME/pairtools_RS_target_codex"` for Cargo work in this repo.
- Run `cargo check` and existing tests through Pixi when no duplicate build process is active.
- Do not report speedups unless normalized oracle parity passes first.
