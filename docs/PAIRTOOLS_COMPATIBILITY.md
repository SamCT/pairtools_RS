# Pairtools Compatibility Inventory

Target command surface for this rewrite:
- parse
- parse2
- sort
- dedup
- flip
- merge
- split
- select
- stats
- restrict
- filterbycov
- phase
- markasdup

## Current Rust status
- parse: implemented with rust-htslib streaming reader for SAM/BAM/CRAM input, plus core options `-c/--chroms-path`, `--drop-sam`, `--min-mapq`, `--walks-policy`, `--report-alignment-end`, `--output-stats`.
- sort: implemented external-sort skeleton with spill files and deterministic row ordering.
- all remaining commands: exposed in CLI and fail loudly with `not implemented`.

## Option parity policy
For this stage, any recognized option must be either:
1. fully implemented, or
2. rejected with `not implemented`.

No no-op options are accepted.

## Next compatibility expansion
- Fill complete per-command/per-option matrix directly from installed `pairtools --help` output.
- Validate exact output parity against pairtools oracle fixtures for each implemented command.
