# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M060: sort core.

M020, M030, M040, and M050 are now marked complete. M060 is active as the next milestone.

## Current branch

`master`

## Current commit

`uncommitted` during parse milestone closure. The final task response must report the committed SHA.

## Implemented behavior

Completed parse milestones now covered by the existing guarded oracle suite:

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
  - Unsupported walk policies fail loudly.

## Intentionally unsupported behavior

- Full pairtools `parse2` behavior is not implemented.
- Full complex-walk parity is not claimed beyond the scoped M050 fixtures.
- Non-adjacent repeated read names remain unsupported and fail loudly.
- Rust downstream commands remain unimplemented.
- No sort behavior is changed by this parse milestone closure.
- No benchmarks or performance claims are added.

## Validation performed

Validation commands for this parse milestone closure:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M050 --allow-nonactive
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
python3 scripts/check_no_runtime_pairtools.py --milestone M050
python3 scripts/check_no_noop_flags.py --milestone M050
python3 scripts/check_parse_lite_drift.py --milestone M050
python3 scripts/check_cargo_needed.py --milestone M050
python3 scripts/milestone_gate.py post --milestone M050 --allow-nonactive
python3 scripts/codex_report.py --milestone M050
git diff --check
```

## Validation not performed and why

- Benchmarks were not run because parse milestones M030-M050 are correctness milestones, not performance milestones.
- New parse behavior was not added in this closure; the work reconciles existing oracle coverage and milestone state.

## Cargo required

Yes for validation of the parse milestone closure, because the milestone required tests are `scripts/cargo_guard.sh check` and `scripts/cargo_guard.sh test`.

## External real-data oracle status

External real-data oracle discovery for M080 remains documented in `docs/REAL_DATA_ORACLE_TESTING.md`. No external fixture data is committed.

## Next recommended milestone

M060: sort core.
