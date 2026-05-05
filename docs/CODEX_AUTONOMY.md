# Codex Autonomous Milestone Protocol

Codex must not operate as an open-ended assistant. Codex must operate as a milestone executor.

## Startup

1. Read `milestones/ACTIVE_MILESTONE`.
2. Read the matching `milestones/<ID>-*.json`.
3. Read `docs/STATUS.md`.
4. Read `docs/PAIRTOOLS_COMPATIBILITY.md`.

## Scope

- Modify only files listed in `allowed_paths`.
- Never modify files listed in `forbidden_paths`.
- If a required change appears to need a forbidden path, stop and write a blocker report instead of editing it.

## Execution

- Implement the smallest change needed to satisfy the active milestone.
- Prefer tests and validation scripts over source edits when the milestone is validation-only.
- Never claim full pairtools parity unless the milestone explicitly requires and verifies it.
- Never claim optimization unless a benchmark listed in `required_benchmarks` was run.

## Validation

- Run every command in `required_tests`.
- Run the milestone gate before finishing. This repository currently supports:

  ```bash
  python3 scripts/milestone_gate.py pre --milestone <ID>
  python3 scripts/milestone_gate.py post --milestone <ID>
  ```

- If a test fails, inspect, fix within scope, and rerun.
- If blocked by missing external data or unavailable tools, record the exact blocker in `docs/STATUS.md`.

## Completion

- Update `docs/STATUS.md`.
- Update `docs/PAIRTOOLS_COMPATIBILITY.md` only if behavior changed or validation status changed.
- Commit with message: `<MILESTONE_ID>: <short result>`.
- Final report must include commit SHA, changed files, commands run, pass/fail status, and next milestone recommendation.
