# Repository Guidance

This repository is a full pairtools-compatible Rust rewrite. The long-term goal is complete pairtools-compatible behavior in Rust. The immediate rule is one bounded, oracle-tested milestone at a time.

## Enforced Milestone Workflow

For ordinary implementation work, `milestones/ACTIVE_MILESTONE` is authoritative. Governance/bootstrap tasks may use M000 with `--allow-nonactive` only when the user explicitly identifies the task as governance work and the edits stay inside M000 allowed paths.

At task start:

1. Identify the milestone ID from `milestones/ACTIVE_MILESTONE` or explicitly justify changing it.
2. Run:

   ```bash
   python3 scripts/milestone_gate.py pre --milestone <ID>
   ```

3. If preflight fails, do not code.

During the task:

1. Stay within the milestone `allowed_paths`.
2. Do not implement milestone non-goals.
3. Do not run Cargo directly.
4. Use `scripts/cargo_guard.sh` if Cargo validation is required.
5. Record each validation with `scripts/record_test_result.py`.
6. For guided active-milestone execution, use `make codex-next`. This runner lists required tests and fails until they are recorded; it must not be treated as proof that tests ran.

Before the final response:

1. Run the required tests from the active milestone JSON.
2. Run:

   ```bash
   python3 scripts/milestone_gate.py post --milestone <ID>
   python3 scripts/codex_report.py --milestone <ID>
   ```

3. Copy the report fields into the final answer. Do not fabricate tests, benchmarks, or compatibility claims.

## Cargo Policy

- Documentation-only and governance-only changes do not require Cargo validation.
- Rust source, `Cargo.toml`, `Cargo.lock`, `pixi.toml`, `pixi.lock`, `build.rs`, tests, benches, or examples changes require `scripts/cargo_guard.sh check`.
- Never launch duplicate Cargo jobs.
- Never delete `.pixi` without explicit user authorization.
- Never delete `CARGO_TARGET_DIR` without explicit user authorization.
- Always use the local checkout path for Codex work: `/mnt/d/pairtools_RS` (Windows: `D:\pairtools_RS`).
- The default target directory for guarded Cargo commands is:

  ```bash
  export CARGO_TARGET_DIR="$HOME/pairtools_RS_target_codex"
  ```

- If Cargo reports an artifact-directory lock, inspect active processes before retrying.

## Compatibility Policy

- Every accepted option must implement pairtools-compatible semantics or fail loudly with `not implemented`.
- Compatibility flags with ignored behavior are forbidden.
- Placeholder success paths are forbidden.
- Pairtools may be used only as an oracle in tests, scripts, and benchmarks.
- Rust runtime must not call pairtools.
- Rust runtime must not shell out to samtools, bgzip, or gzip unless a future milestone explicitly allows it.
- Use `rust-htslib`/HTSlib for SAM/BAM/CRAM input and BGZF output.
- Preserve exact or normalized oracle parity before claiming performance.
- Performance claims require parity to pass first.

## Milestone Policy

- `milestones/ACTIVE_MILESTONE` is authoritative.
- Future work must fit the active milestone or explicitly change `ACTIVE_MILESTONE` in the same commit with justification.
- Downstream commands are non-goals unless the active milestone explicitly lists them.
- Do not implement broad feature surface in one task.
- Keep `milestones/README.md` synchronized with milestone JSON files.
- Record completed milestone evidence in `milestone_results/<MILESTONE>.json` when the active milestone requires a result ledger.
- Every Codex task must end by updating or explicitly confirming no content change is needed in:
  - `docs/PAIRTOOLS_COMPATIBILITY.md`
  - `docs/STATUS.md`

## Required Final Report Fields

Every Codex task must report:

- branch
- commit SHA
- files changed
- implemented behavior
- intentionally unsupported behavior
- tests run
- tests not run and why
- benchmark results, if applicable
- next recommended milestone
