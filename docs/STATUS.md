# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M020: parse input/output plumbing.

M080 exact hybrid pipeline is complete and pushed. M020 is now active to harden parse I/O boundaries without changing pair formation semantics.

## Current branch

`master`

## Current commit

`uncommitted` during M020 validation. The final task response must report the committed SHA.

## Implemented behavior

- Marked M080 complete and M020 active in the milestone registry.
- Added parse I/O tests showing `pairs-rs parse` reads the same SAM data from stdin as from a path.
- Added parse I/O tests showing a BAM path generated from the SAM fixture produces the same pairs output as the SAM path.
- Added parse I/O tests showing `-o` writes pairs output to a file and leaves stdout empty.
- Added parse I/O tests showing `--output-stats` writes a stats file.
- Added parse I/O tests showing compressed parse output and compressed parse stats output fail loudly.

## Intentionally unsupported behavior

- M020 does not expand pair formation semantics.
- M020 does not implement sort changes or downstream commands.
- M020 does not call samtools from Rust runtime.
- CRAM-specific reference handling remains unverified in this pass; the runtime path is still rust-htslib/HTSlib.
- Compressed parse output and compressed parse stats output remain explicitly not implemented.

## Validation performed

M020 validation commands:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M020
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
python3 scripts/check_no_runtime_pairtools.py --milestone M020
python3 scripts/check_no_noop_flags.py --milestone M020
python3 scripts/check_parse_lite_drift.py --milestone M020
python3 scripts/check_cargo_needed.py --milestone M020
python3 scripts/milestone_gate.py post --milestone M020
python3 scripts/codex_report.py --milestone M020
git diff --check
```

## Validation not performed and why

- Benchmarks are not run because M020 is an I/O correctness milestone.
- Downstream pairtools commands are not tested in M020 because M080 owns the shell pipeline bridge and Rust downstream commands remain non-goals.

## Cargo required

Yes. M020 changes `tests/compat_oracle.rs`, so Cargo validation must run through `scripts/cargo_guard.sh`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. M020 does not add or commit external fixture data.

## Next recommended milestone

M030: core pair formation for ordinary paired reads.
