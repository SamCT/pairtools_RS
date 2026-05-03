#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: scripts/cargo_guard.sh <check|test|build|clippy|fmt-check>" >&2
}

if [[ $# -ne 1 ]]; then
  usage
  exit 2
fi

subcommand="$1"
case "$subcommand" in
  check|test|build|clippy|fmt-check) ;;
  *)
    usage
    exit 2
    ;;
esac

process_pattern='cargo|rustc|cc|c\+\+|clang|ld|pairs-rs'
processes="$(ps -ef | grep -E "$process_pattern" | grep -v grep || true)"
active=""
if [[ -n "$processes" ]]; then
  active="$(printf "%s\n" "$processes" \
    | grep -E '(^|[/[:space:]-])(cargo|rustc|rustdoc|cc|c\+\+|g\+\+|gcc|clang|ld|lld|mold|pairs-rs)([[:space:]]|$)' \
    | grep -Ev 'systemd-journald|scripts/cargo_guard.sh|scripts/milestone_gate.py|scripts/check_|scripts/codex_report.py' \
    || true)"
fi

if [[ -n "$active" ]]; then
  echo "$processes" >&2
  echo "another Cargo/Rust/native build process is active; not starting cargo $subcommand" >&2
  echo "inspect processes before retrying:" >&2
  echo "  ps -ef | grep -E 'cargo|rustc|cc|c\\+\\+|clang|ld|pairs-rs' | grep -v grep || true" >&2
  exit 1
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/pairtools_RS_target_codex}"
mkdir -p target/codex_logs
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
log_path="target/codex_logs/cargo_${subcommand}_${timestamp}.log"

case "$subcommand" in
  fmt-check)
    run_cmd=(pixi run cargo fmt -- --check)
    ;;
  clippy)
    run_cmd=(pixi run cargo clippy --all-targets -- -D warnings)
    ;;
  *)
    run_cmd=(pixi run cargo "$subcommand")
    ;;
esac

echo "running: ${run_cmd[*]}"
echo "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
echo "log: $log_path"

set +e
"${run_cmd[@]}" 2>&1 | tee "$log_path"
status=${PIPESTATUS[0]}
set -e

if [[ "$status" -ne 0 ]]; then
  echo "cargo $subcommand failed with status $status" >&2
  echo "no automatic retry was attempted." >&2
  echo "if this looks like a lock or stalled build, inspect processes before retrying:" >&2
  echo "  ps -ef | grep -E 'cargo|rustc|cc|c\\+\\+|clang|ld|pairs-rs' | grep -v grep || true" >&2
fi

exit "$status"
