#!/usr/bin/env python3
"""Conservative active-milestone runner for Codex work."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys

from governance_common import (
    REPO_ROOT,
    active_milestone,
    load_milestone,
    print_list,
    read_test_results,
    tests_path,
    utc_now,
)


def run_cmd(cmd: list[str]) -> int:
    print(f"$ {' '.join(cmd)}", flush=True)
    return subprocess.run(cmd, cwd=REPO_ROOT, text=True).returncode


def run_shell(command: str) -> int:
    print(f"$ {command}", flush=True)
    return subprocess.run(command, cwd=REPO_ROOT, shell=True, executable="/bin/bash", text=True).returncode


def record_result(milestone_id: str, name: str, status: str, command: str, reason: str = "") -> None:
    row = {
        "timestamp": utc_now(),
        "milestone": milestone_id,
        "name": name,
        "status": status,
        "command": command,
        "reason": reason,
    }
    path = tests_path(milestone_id)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(row, sort_keys=True) + "\n")


def recorded_passes(milestone_id: str) -> set[str]:
    return {
        row.get("command", "")
        for row in read_test_results(milestone_id)
        if row.get("status") == "pass"
    }


def print_summary(milestone: dict) -> None:
    print(f"active milestone: {milestone['id']}")
    print(f"name: {milestone['name']}")
    print(f"status: {milestone['status']}")
    print(f"goal: {milestone['goal']}")
    print_list("explicit non-goals", milestone.get("explicit_non_goals", []))
    print_list("allowed paths", milestone.get("allowed_paths", []))
    print_list("forbidden paths", milestone.get("forbidden_paths", []))
    print_list("required tests", milestone.get("required_tests", []))
    print_list("required benchmarks", milestone.get("required_benchmarks", []))


def print_recorded_tests(milestone_id: str) -> None:
    rows = read_test_results(milestone_id)
    print("recorded tests:")
    if not rows:
        print("  - none")
        return
    for row in rows:
        print(f"  - {row.get('status')}: {row.get('command')} :: {row.get('reason', '')}")


def current_invocation_satisfies() -> set[str]:
    commands: set[str] = set()
    if os.environ.get("MAKELEVEL"):
        commands.add("make codex-next")
    return commands


def missing_required(milestone: dict, milestone_id: str) -> list[str]:
    required = milestone.get("required_tests", [])
    passes = recorded_passes(milestone_id) | current_invocation_satisfies()
    return [command for command in required if command not in passes]


def run_required_tests(milestone: dict, milestone_id: str) -> int:
    status = 0
    for command in milestone.get("required_tests", []):
        if command == "make codex-next" or "codex_next.py --run-required-tests" in command:
            print(f"refusing recursive required test: {command}")
            record_result(
                milestone_id,
                command,
                "not-run",
                command,
                "recursive runner invocation must be executed and recorded by the caller",
            )
            status = 2
            continue
        rc = run_shell(command)
        record_result(milestone_id, command, "pass" if rc == 0 else "fail", command)
        if rc != 0:
            status = rc
            break
    return status


def run_default(milestone: dict, milestone_id: str) -> int:
    pre_rc = run_cmd(["python3", "scripts/milestone_gate.py", "pre", "--milestone", milestone_id])
    if pre_rc != 0:
        return pre_rc

    print_summary(milestone)
    missing = missing_required(milestone, milestone_id)
    if missing:
        print()
        print("required tests are not all recorded as pass:")
        for command in missing:
            print(f"  - {command}")
        print()
        print("Run each required command, record it with scripts/record_test_result.py, then rerun make codex-next.")

    post_rc = run_cmd(["python3", "scripts/milestone_gate.py", "post", "--milestone", milestone_id])
    report_rc = run_cmd(["python3", "scripts/codex_report.py", "--milestone", milestone_id])

    if post_rc != 0:
        return post_rc
    if report_rc != 0:
        return report_rc
    if missing:
        return 2
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", default="", help="Override the active milestone for inspection or testing.")
    parser.add_argument("--status", action="store_true", help="Print active milestone summary and recorded validations.")
    parser.add_argument("--run-required-tests", action="store_true", help="Execute non-recursive required tests and record results.")
    parser.add_argument("--chain", action="store_true", help="Print the next milestone candidates after the current milestone.")
    args = parser.parse_args()

    milestone_id = args.milestone or active_milestone()
    milestone = load_milestone(milestone_id)

    if args.status:
        print_summary(milestone)
        print_recorded_tests(milestone_id)
        if args.chain:
            print_list("next milestone candidates", milestone.get("next_milestone_candidates", []))
        return 0

    if args.run_required_tests:
        return run_required_tests(milestone, milestone_id)

    rc = run_default(milestone, milestone_id)
    if args.chain:
        print_list("next milestone candidates", milestone.get("next_milestone_candidates", []))
    return rc


if __name__ == "__main__":
    sys.exit(main())
