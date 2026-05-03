# Pairtools Compatibility Inventory

## Target oracle
- Requested oracle: installed Pixi `pairtools` binary.
- Current environment status (2026-05-03): `pairtools` and `pixi` executables are not available in PATH, and `python -m pairtools` fails due to missing compiled extension (`dedup_cython`).
- Therefore this initial inventory is derived from repository CLI sources (`pairtools/cli/*.py`) and must be revalidated against runtime `--help` output once oracle binary is available.

## Command inventory
parse, parse2, sort, dedup, flip, merge, split, select, stats, restrict, filterbycov, phase, markasdup, plus header/scaling/sample in Python tool.

## First-PR implementation status
- parse: implemented scaffold in Rust; parity not proven.
- sort: implemented scaffold in Rust; parity not proven.
- parse2/dedup/flip/merge/split/select/stats/restrict/filterbycov/phase/markasdup: recognized by CLI and fail loudly as unsupported.

## Option inventory (first-pass)
Detailed option extraction from runtime `pairtools <cmd> --help` is pending oracle availability. The first Rust scaffold currently supports:
- parse: `-c/--chroms-path`, `-o/--output`, `--threads`/`-@`, `--min-mapq`, `--report-alignment-end`, `--drop-readid`, input path/stdin.
- sort: `-o/--output`, `--nproc`/`--threads`/`-@`, input path/stdin.

All other pairtools options are currently **not implemented** in Rust and must fail loudly.

## Known differences
- Runtime pairtools help snapshot has not yet been captured in this environment.
- Parse/sort behavior currently only covers starter fixtures and is not yet exact parity.

## Fixture coverage (committed)
- parse simple UU.
- parse reverse-strand coordinate reporting switch.
- parse chrom.sizes-based flipping.
- sort simple ordering.

## Next steps
1. Capture actual `pairtools --help` surface from runnable oracle binary.
2. Expand compatibility table to per-option statuses (exact / parity-unproven / missing / intentionally unsupported).
3. Grow fixtures to full requested matrix and wire oracle-update mode.
