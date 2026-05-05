#!/usr/bin/env python3
"""Print a milestone final-report template without fabricating results."""

from __future__ import annotations

import argparse

from governance_common import (
    REPO_ROOT,
    changed_files,
    current_branch,
    current_commit,
    first_matching_pattern,
    load_json,
    load_milestone,
    read_test_results,
    state_path,
)


DOC_SUFFIXES = (".md", ".rst", ".txt")


def cargo_required(milestone: dict, files: list[str]) -> tuple[bool, list[str]]:
    reasons = []
    for path in files:
        pattern = first_matching_pattern(path, milestone.get("cargo_required_if_paths_changed", []))
        if pattern and not (path.endswith(DOC_SUFFIXES) or path.startswith("docs/")):
            reasons.append(f"{path} matched {pattern}")
    return bool(reasons), reasons


def first(values: list[str]) -> str:
    return values[0] if values else "not applicable"


def print_test_results(results: list[dict]) -> None:
    if not results:
        print("  UNKNOWN: no test results recorded. Run scripts/record_test_result.py for each validation.")
        return
    for row in results:
        status = row.get("status", "UNKNOWN")
        name = row.get("name", "UNKNOWN")
        command = row.get("command", "")
        reason = row.get("reason", "")
        tail = f" :: {command}" if command else ""
        if reason:
            tail += f" :: {reason}"
        print(f"  - {status}: {name}{tail}")


def print_bullets(values: list[str]) -> None:
    if values:
        for value in values:
            print(f"  - {value}")
    else:
        print("  - none")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", required=True)
    args = parser.parse_args()

    milestone = load_milestone(args.milestone)
    pre_path = state_path(args.milestone, "pre")
    post_path = state_path(args.milestone, "post")
    pre = load_json(pre_path) if pre_path.exists() else {}
    post = load_json(post_path) if post_path.exists() else {}
    files = changed_files()
    tests = read_test_results(args.milestone)
    docs_updated = [path for path in milestone.get("docs_required", []) if path in files]
    cargo_needed, cargo_reasons = cargo_required(milestone, files)
    anti_drift = post.get("checks", [])

    print("branch:")
    print(f"  {current_branch()}")
    print("commit SHA:")
    print(f"  {current_commit()}")
    print("starting commit SHA:")
    print(f"  {pre.get('commit_sha', 'UNKNOWN: run python3 scripts/milestone_gate.py pre --milestone ' + args.milestone)}")
    print("files changed:")
    print_bullets(files)
    print("implemented behavior:")
    print(f"  UNKNOWN: fill in manually for {args.milestone}; report only behavior changed in this task.")
    print("intentionally unsupported behavior:")
    print("  UNKNOWN: fill in manually from milestone non-goals and observed repository limits.")
    print("milestone:")
    print(f"  {milestone['id']}: {milestone['name']}")
    print("explicit non-goals:")
    print_bullets(milestone.get("explicit_non_goals", []))
    print("oracle command:")
    print(f"  {first(milestone.get('oracle_commands', []))}")
    print("candidate command:")
    print(f"  {first(milestone.get('candidate_commands', []))}")
    print("tests run:")
    print_test_results([row for row in tests if row.get("status") in {"pass", "fail"}])
    print("tests not run and why:")
    not_run = [row for row in tests if row.get("status") == "not-run"]
    if not_run:
        print_test_results(not_run)
    else:
        print("  UNKNOWN: record not-run validations explicitly if any were skipped.")
    print("benchmark results:")
    if milestone.get("required_benchmarks"):
        print("  UNKNOWN: required benchmark results must be filled manually from actual runs.")
    else:
        print("  not applicable")
    print("docs updated:")
    print_bullets(docs_updated)
    print("cargo required:")
    print(f"  {'yes' if cargo_needed else 'no'}")
    if cargo_reasons:
        for reason in cargo_reasons:
            print(f"  - {reason}")
    print("cargo run:")
    cargo_rows = [row for row in tests if "cargo" in row.get("name", "").lower()]
    if cargo_rows:
        print_test_results(cargo_rows)
    elif cargo_needed:
        print("  UNKNOWN: cargo is required; run scripts/cargo_guard.sh check and record the result.")
    else:
        print("  not run; not required by changed files")
    print("anti-drift checks:")
    if anti_drift:
        for check in anti_drift:
            print(f"  - {check.get('status', 'UNKNOWN')}: {check.get('name')} :: {check.get('command')}")
    else:
        print("  UNKNOWN: run python3 scripts/milestone_gate.py post --milestone " + args.milestone)
    print("next recommended milestone:")
    candidates = milestone.get("next_milestone_candidates", [])
    print(f"  {first(candidates)}")
    print("result ledger:")
    ledger = REPO_ROOT / "milestone_results" / f"{args.milestone}.json"
    if ledger.exists():
        print(f"  {ledger.relative_to(REPO_ROOT)}")
    else:
        print(f"  missing: {ledger.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
