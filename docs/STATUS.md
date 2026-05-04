# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M010: CLI inventory.

M000 governance automation is complete. M010 continues the milestone workflow by testing the current command and option inventory without adding command behavior.

## Current branch

`master`

## Current commit

`uncommitted` during M010 validation. The final task response must report the committed SHA.

## Implemented behavior

- `milestones/ACTIVE_MILESTONE` now selects M010.
- M000 is marked complete and M010 is marked active in the milestone registry.
- M010 adds integration-test coverage that the top-level CLI help lists the current pairtools command surface.
- M010 adds integration-test coverage that `parse --help` and `sort --help` expose the currently inventoried options.
- M010 adds integration-test coverage that unsupported top-level options fail loudly.
- M010 extends unsupported-command coverage so `parse2`, `dedup`, `flip`, `merge`, `split`, `select`, `stats`, `restrict`, `filterbycov`, `phase`, `markasdup`, `sample`, `header`, and `scaling` all fail loudly with `not implemented`.

## Intentionally unsupported behavior

- M010 does not implement new parse behavior.
- M010 does not implement new sort behavior.
- M010 does not implement downstream command behavior.
- M010 does not change `src/parse.rs` or `src/sort.rs`.
- M010 does not claim new oracle parity beyond CLI inventory and loud-failure behavior.
- M010 does not run benchmarks or make performance claims.

## Validation performed

M010 validation commands:

```bash
git status --short --branch
python3 scripts/milestone_gate.py pre --milestone M010
scripts/cargo_guard.sh check
scripts/cargo_guard.sh test
python3 scripts/milestone_gate.py post --milestone M010
python3 scripts/codex_report.py --milestone M010
git diff --check
```

Pairtools oracle inventory commands for M010:

```bash
pixi run pairtools --help
pixi run pairtools parse --help
```

Each validation command must be recorded with `scripts/record_test_result.py`.

## Validation not performed and why

- Benchmarks are not run because M010 is an inventory and loud-failure milestone, not a performance milestone.
- Full parse/sort oracle parity expansion is not part of M010.

## Current Rust implementation baseline

The binary remains a partial pairtools-compatible `parse`/`sort` implementation. M010 only hardens the CLI inventory baseline and unsupported-command failures. Compatibility claims for parse/sort behavior remain bounded by prior oracle tests and future milestone-gated validation.

## Legacy parse-lite names

`scripts/parse_lite_benchmark.sh`, `scripts/run_parse_lite_pipeline.sh`, and `scripts/TESTING_PARSE_LITE.md` are legacy artifacts. They are not the milestone authority and must not be used to infer current binary scope.

## Next recommended milestone

M020: parse input/output plumbing only.
