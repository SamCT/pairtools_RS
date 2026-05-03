# Repository Guidance

## Pairtools-rs milestone workflow

This repository is a full pairtools-compatible Rust rewrite.

Rules:
- Do not implement broad feature surface in one task.
- Work one milestone at a time.
- Do not implement downstream milestones unless explicitly requested.
- Pairtools may be used only as an oracle in tests and shell validation scripts, never as a runtime dependency inside pairs-rs.
- Every accepted option must either match pairtools semantics or fail loudly with not implemented.
- No no-op compatibility flags are allowed.
- Use rust-htslib/HTSlib for SAM/BAM/CRAM parsing and BGZF output.
- Preserve exact or normalized oracle parity before claiming performance.
- Performance claims require parity to pass first.

Cargo / WSL:
- Always use local checkout path for codex work: /mnt/d/pairtools_RS (Windows: D:\pairtools_RS).
  export CARGO_TARGET_DIR="$HOME/pairtools_RS_target_codex"
- Before running Cargo, inspect existing cargo/rustc processes.
- Use pixi
- Do not launch duplicate cargo builds/tests.
- If Cargo reports an artifact-directory lock, inspect processes before retrying.

Cargo validation policy:
- Documentation-only changes do not require cargo check.
- Cargo.toml, pixi.toml, build.rs, src/, tests/, benches/, or examples/ changes require cargo check.
- Do not delete .pixi or CARGO_TARGET_DIR unless the user explicitly authorizes it.
- Do not recreate generated environments as a routine fix.
- If cargo check times out at the wrapper level but the Cargo process is still running, continue monitoring the same process and capture logs from tee.
- Do not rerun cargo check solely because the wrapper timed out.
- Use a long timeout for the first cold native build.
- Treat native dependency compilation as expected, not as a hang, unless CPU usage is near zero for several minutes and no new output appears.

Every Codex task must end by updating:
- docs/PAIRTOOLS_COMPATIBILITY.md
- docs/STATUS.md

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

Long-term goal: full pairtools-compatible Rust implementation.
Immediate goal: one bounded, oracle-tested milestone at a time.
