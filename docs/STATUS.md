# pairs-rs Status

Last reconciled: 2026-05-04

## Active milestone

M000: governance automation and anti-drift infrastructure.

## Current branch

`master`

## Current commit

`uncommitted` during M000 validation. The final task response must report the committed SHA after the single governance commit is amended or created.

## Purpose of M000

M000 turns repository prose guidance into executable checks. It does not add parse, sort, downstream command, compression, or performance behavior.

## Implemented behavior

- A machine-readable milestone registry under `milestones/` defines active and planned milestone scope.
- `milestones/ACTIVE_MILESTONE` names the current milestone authority.
- `scripts/milestone_gate.py` provides preflight, postflight, and status checks.
- `scripts/cargo_guard.sh` is the required Cargo wrapper when Rust, Cargo, Pixi, tests, benches, or examples change.
- Governance check scripts enforce milestone schema, changed-path boundaries, docs synchronization, Cargo-needed detection, runtime external-tool policy, compatibility-placeholder policy, and legacy parser framing policy.
- `scripts/record_test_result.py` records validation evidence under `target/codex_task_state/`.
- `scripts/codex_report.py` prints required final report fields without fabricating results.
- GitHub Actions, PR/issue templates, Make targets, and fixture planning docs support the same workflow.

## Intentionally unsupported behavior

- M000 does not change Rust parse or sort implementation behavior.
- M000 does not implement `merge`, `dedup`, `select`, `split`, `stats`, `parse2`, `header`, `restrict`, `phase`, `sample`, `scaling`, `filterbycov`, or other downstream commands.
- M000 does not run benchmarks and does not add performance claims.
- Cargo validation is not required for M000 unless Rust source, Cargo metadata, Pixi metadata, tests, benches, or examples change.

## What is still not automated

- The gates use heuristic text scans; they are guardrails, not a substitute for review.
- The report helper prints placeholders for behavior and benchmark fields; a human or Codex must still fill them with actual evidence.
- CI can prove governance checks run, but it cannot prove pairtools parity unless a future milestone adds oracle fixtures and commands.
- The active milestone must still be advanced intentionally by a future task.

## Validation performed

M000 validation commands:

```bash
git status --short --branch
python3 -m py_compile scripts/milestone_gate.py scripts/codex_report.py scripts/check_no_runtime_pairtools.py scripts/check_no_noop_flags.py scripts/check_docs_sync.py scripts/check_parse_lite_drift.py scripts/check_changed_paths.py scripts/check_milestone_schema.py scripts/check_cargo_needed.py scripts/record_test_result.py scripts/list_milestones.py
bash -n scripts/cargo_guard.sh
python3 scripts/check_milestone_schema.py
python3 scripts/milestone_gate.py pre --milestone M000
python3 scripts/check_no_runtime_pairtools.py --milestone M000
python3 scripts/check_no_noop_flags.py --milestone M000
python3 scripts/check_parse_lite_drift.py --milestone M000
python3 scripts/check_cargo_needed.py --milestone M000
python3 scripts/milestone_gate.py post --milestone M000
python3 scripts/codex_report.py --milestone M000
git diff --check
```

Each command must be recorded with `scripts/record_test_result.py`.

## Validation not performed and why

- `cargo check` is not expected for M000 because this milestone changes governance, docs, scripts, CI, templates, milestone metadata, and README-style test planning docs only.
- Parse/sort oracle parity is not rerun in M000 because no Rust parse/sort behavior is changed.
- Benchmarks are not run because performance is a hard non-goal for M000.

## Current Rust implementation baseline

The current binary remains a partial pairtools-compatible `parse`/`sort` implementation from prior milestones. M000 does not re-verify that implementation. Compatibility claims remain limited to previously recorded oracle tests and must be re-established by future milestone-gated validation before new performance or parity claims are made.

## Legacy parse-lite names

`scripts/parse_lite_benchmark.sh`, `scripts/run_parse_lite_pipeline.sh`, and `scripts/TESTING_PARSE_LITE.md` are legacy artifacts. They are not the milestone authority and must not be used to infer current binary scope.

## Next recommended milestone

M010: CLI inventory only, focused on pairtools command and option inventory plus loud failures for unsupported options.
