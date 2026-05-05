#!/usr/bin/env python3
"""Preflight, postflight, and status gate for milestone-scoped Codex work."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from governance_common import (
    REPO_ROOT,
    STATE_DIR,
    active_build_processes,
    active_milestone,
    changed_files,
    current_branch,
    current_commit,
    git_status_short,
    load_json,
    load_milestone,
    milestone_files,
    print_list,
    read_test_results,
    state_path,
    utc_now,
    write_json,
)


CHECKS = [
    ("changed paths", ["python3", "scripts/check_changed_paths.py"]),
    ("runtime pairtools", ["python3", "scripts/check_no_runtime_pairtools.py"]),
    ("compat flag guard", ["python3", "scripts/check_no_noop_flags.py"]),
    ("legacy parser drift", ["python3", "scripts/check_parse_lite_drift.py"]),
    ("docs sync", ["python3", "scripts/check_docs_sync.py"]),
    ("milestone result ledgers", ["python3", "scripts/check_milestone_results.py"]),
    ("cargo needed", ["python3", "scripts/check_cargo_needed.py"]),
]


def ensure_repo_root() -> None:
    if Path.cwd().resolve() != REPO_ROOT.resolve():
        raise SystemExit(f"must run from repository root: {REPO_ROOT}")
    if not (REPO_ROOT / ".git").exists():
        raise SystemExit("repository root does not contain .git")


def run_schema_check(allow_multiple_active: bool = False) -> None:
    cmd = ["python3", "scripts/check_milestone_schema.py"]
    if allow_multiple_active:
        cmd.append("--allow-multiple-active")
    result = subprocess.run(cmd, cwd=REPO_ROOT, text=True)
    if result.returncode != 0:
        raise SystemExit(result.returncode)


def ensure_active_requested(milestone_id: str, allow_nonactive: bool) -> None:
    active = active_milestone()
    if active != milestone_id and not allow_nonactive:
        raise SystemExit(
            f"requested milestone {milestone_id} but milestones/ACTIVE_MILESTONE is {active}; "
            "pass --allow-nonactive only for read-only inspection"
        )


def print_milestone_summary(milestone: dict) -> None:
    print(f"milestone ID: {milestone['id']}")
    print(f"milestone name: {milestone['name']}")
    print(f"goal: {milestone['goal']}")
    print_list("explicit non-goals", milestone.get("explicit_non_goals", []))
    print_list("allowed paths", milestone.get("allowed_paths", []))
    print_list("forbidden paths", milestone.get("forbidden_paths", []))
    print_list("required tests", milestone.get("required_tests", []))
    print_list("required docs", milestone.get("docs_required", []))
    print_list("cargo-trigger paths", milestone.get("cargo_required_if_paths_changed", []))


def print_process_table_or_none() -> list[str]:
    active = active_build_processes()
    print("active cargo/native process inspection:")
    if active:
        for line in active:
            print(f"  {line}")
    else:
        print("  none")
    return active


def preflight(milestone_id: str, allow_nonactive: bool) -> None:
    ensure_repo_root()
    run_schema_check()
    ensure_active_requested(milestone_id, allow_nonactive)
    milestone = load_milestone(milestone_id)
    print_milestone_summary(milestone)
    active = print_process_table_or_none()
    if active:
        raise SystemExit("active Cargo/Rust/native build process found; do not code or launch Cargo")

    state = {
        "milestone": milestone_id,
        "branch": current_branch(),
        "commit_sha": current_commit(),
        "timestamp": utc_now(),
        "git_status_short": git_status_short(),
        "active_processes": active,
        "allowed_paths": milestone.get("allowed_paths", []),
        "forbidden_paths": milestone.get("forbidden_paths", []),
        "docs_required": milestone.get("docs_required", []),
        "cargo_required_if_paths_changed": milestone.get("cargo_required_if_paths_changed", []),
    }
    write_json(state_path(milestone_id, "pre"), state)
    print(f"wrote {state_path(milestone_id, 'pre').relative_to(REPO_ROOT)}")


def docs_exist(milestone: dict) -> None:
    missing = [path for path in milestone.get("docs_required", []) if not (REPO_ROOT / path).exists()]
    if missing:
        print_list("missing docs_required files", missing)
        raise SystemExit(1)


def docs_changed_when_required(milestone: dict, files: list[str]) -> None:
    docs = set(milestone.get("docs_required", []))
    if not docs:
        return
    changed_docs = docs.intersection(files)
    script_or_source_changed = any(
        path.startswith(("src/", "scripts/"))
        or path in {"Cargo.toml", "Cargo.lock", "pixi.toml", "pixi.lock", "build.rs"}
        for path in files
    )
    if script_or_source_changed and not changed_docs:
        print_list("changed source/script files", files)
        raise SystemExit("docs_required files must change when source or scripts change")


def run_post_check(label: str, base_cmd: list[str], milestone_id: str) -> dict:
    cmd = [*base_cmd, "--milestone", milestone_id]
    result = subprocess.run(cmd, cwd=REPO_ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    ok = result.returncode == 0
    return {"name": label, "command": " ".join(cmd), "status": "pass" if ok else "fail"}


def expected_vs_observed(milestone: dict, milestone_id: str) -> dict:
    expected = milestone.get("required_tests", [])
    observed = read_test_results(milestone_id)
    observed_text = [
        (row.get("name", ""), row.get("status", ""), row.get("command", ""))
        for row in observed
    ]
    print_list("tests expected", expected)
    if observed_text:
        print("tests observed:")
        for name, status, command in observed_text:
            print(f"  - {status}: {name} :: {command}")
    else:
        print("tests observed:")
        print("  - none recorded")
    return {"expected": expected, "observed": observed}


def postflight(milestone_id: str, allow_nonactive: bool) -> None:
    ensure_repo_root()
    run_schema_check()
    ensure_active_requested(milestone_id, allow_nonactive)
    milestone = load_milestone(milestone_id)
    pre_path = state_path(milestone_id, "pre")
    if not pre_path.exists():
        raise SystemExit(f"preflight state missing: {pre_path.relative_to(REPO_ROOT)}")

    files = changed_files()
    print_list("changed files", files)
    docs_exist(milestone)
    docs_changed_when_required(milestone, files)

    checks = []
    failed = False
    for label, cmd in CHECKS:
        check = run_post_check(label, cmd, milestone_id)
        checks.append(check)
        failed = failed or check["status"] != "pass"

    tests = expected_vs_observed(milestone, milestone_id)
    state = {
        "milestone": milestone_id,
        "branch": current_branch(),
        "starting_commit_sha": load_json(pre_path).get("commit_sha"),
        "current_commit_sha": current_commit(),
        "timestamp": utc_now(),
        "changed_files": files,
        "checks": checks,
        "tests": tests,
        "git_status_short": git_status_short(),
    }
    write_json(state_path(milestone_id, "post"), state)
    print(f"wrote {state_path(milestone_id, 'post').relative_to(REPO_ROOT)}")
    if failed:
        raise SystemExit(1)


def status(milestone_id: str | None, allow_nonactive: bool) -> None:
    ensure_repo_root()
    run_schema_check(allow_multiple_active=True)
    active = active_milestone()
    if milestone_id:
        ensure_active_requested(milestone_id, allow_nonactive)
    print(f"active milestone: {active}")
    for path in milestone_files():
        data = load_json(path)
        marker = "*" if data["id"] == active else " "
        print(f"{marker} {data['id']} {data['status']}: {data['name']}")
    target = milestone_id or active
    for phase in ("pre", "post"):
        path = state_path(target, phase)
        print(f"{phase} state: {path.relative_to(REPO_ROOT)}")
        if path.exists():
            data = load_json(path)
            print(f"  branch: {data.get('branch')}")
            print(f"  commit: {data.get('commit_sha') or data.get('current_commit_sha')}")
            print(f"  timestamp: {data.get('timestamp')}")
        else:
            print("  missing")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["pre", "post", "status"])
    parser.add_argument("--milestone", default="")
    parser.add_argument("--allow-nonactive", action="store_true")
    args = parser.parse_args()

    if args.command in {"pre", "post"} and not args.milestone:
        parser.error("--milestone is required for pre and post")

    if args.command == "pre":
        preflight(args.milestone, args.allow_nonactive)
    elif args.command == "post":
        postflight(args.milestone, args.allow_nonactive)
    else:
        status(args.milestone or None, args.allow_nonactive)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as exc:
        print(exc.stdout or "", end="")
        print(exc.stderr or "", end="", file=sys.stderr)
        raise SystemExit(exc.returncode)
