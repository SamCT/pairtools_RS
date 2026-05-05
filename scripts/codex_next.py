#!/usr/bin/env python3
"""Conservative active-milestone runner for Codex work."""

from __future__ import annotations

import subprocess
import sys

from governance_common import REPO_ROOT, active_milestone, load_milestone, print_list, read_test_results


def run(cmd: list[str]) -> int:
    print(f"$ {' '.join(cmd)}", flush=True)
    return subprocess.run(cmd, cwd=REPO_ROOT, text=True).returncode


def recorded_passes(milestone_id: str) -> set[str]:
    return {
        row.get("command", "")
        for row in read_test_results(milestone_id)
        if row.get("status") == "pass"
    }


def print_summary(milestone: dict) -> None:
    print(f"active milestone: {milestone['id']}")
    print(f"name: {milestone['name']}")
    print(f"goal: {milestone['goal']}")
    print_list("explicit non-goals", milestone.get("explicit_non_goals", []))
    print_list("required tests", milestone.get("required_tests", []))
    print_list("required benchmarks", milestone.get("required_benchmarks", []))


def main() -> int:
    milestone_id = active_milestone()
    milestone = load_milestone(milestone_id)

    pre_rc = run(["python3", "scripts/milestone_gate.py", "pre", "--milestone", milestone_id])
    if pre_rc != 0:
        return pre_rc

    print_summary(milestone)

    required = milestone.get("required_tests", [])
    passes = recorded_passes(milestone_id)
    missing = [command for command in required if command not in passes]
    if missing:
        print()
        print("required tests are not all recorded as pass:")
        for command in missing:
            print(f"  - {command}")
        print()
        print("Run each required command, record it with scripts/record_test_result.py, then rerun make codex-next.")

    post_rc = run(["python3", "scripts/milestone_gate.py", "post", "--milestone", milestone_id])
    report_rc = run(["python3", "scripts/codex_report.py", "--milestone", milestone_id])

    if post_rc != 0:
        return post_rc
    if report_rc != 0:
        return report_rc
    if missing:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
