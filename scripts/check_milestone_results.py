#!/usr/bin/env python3
"""Validate machine-readable milestone result ledgers."""

from __future__ import annotations

import argparse
import re

from governance_common import REPO_ROOT, load_json, milestone_path, print_list


RESULTS_DIR = REPO_ROOT / "milestone_results"
REQUIRED_FIELDS = ["milestone", "commit", "commands_run", "passed", "artifacts", "blockers"]
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


def validate_result(path, require_commit: bool) -> list[str]:
    errors: list[str] = []
    try:
        data = load_json(path)
    except Exception as exc:  # noqa: BLE001
        return [f"{path}: invalid JSON: {exc}"]

    for field in REQUIRED_FIELDS:
        if field not in data:
            errors.append(f"{path}: missing field {field}")

    milestone = data.get("milestone")
    if not isinstance(milestone, str) or not re.fullmatch(r"M\d{3}", milestone):
        errors.append(f"{path}: milestone must look like M000")
    elif path.name != "TEMPLATE.json":
        try:
            milestone_path(milestone)
        except SystemExit as exc:
            errors.append(f"{path}: milestone {milestone} is not registered: {exc}")
        expected_name = f"{milestone}.json"
        if path.name != expected_name:
            errors.append(f"{path}: filename should be {expected_name}")

    commit = data.get("commit")
    if not isinstance(commit, str):
        errors.append(f"{path}: commit must be a string")
    elif commit != "UNKNOWN" and not COMMIT_RE.fullmatch(commit):
        errors.append(f"{path}: commit must be UNKNOWN or a 40-character lowercase SHA")
    elif require_commit and commit == "UNKNOWN":
        errors.append(f"{path}: commit must be filled before completion")

    commands = data.get("commands_run")
    if not isinstance(commands, list):
        errors.append(f"{path}: commands_run must be a list")
    else:
        for index, row in enumerate(commands):
            if not isinstance(row, dict):
                errors.append(f"{path}: commands_run[{index}] must be an object")
                continue
            if not isinstance(row.get("command"), str) or not row.get("command"):
                errors.append(f"{path}: commands_run[{index}].command is required")
            if row.get("status") not in {"pass", "fail", "not-run", "UNKNOWN"}:
                errors.append(f"{path}: commands_run[{index}].status is invalid")

    if not isinstance(data.get("passed"), bool):
        errors.append(f"{path}: passed must be boolean")
    if not isinstance(data.get("artifacts"), list):
        errors.append(f"{path}: artifacts must be a list")
    blockers = data.get("blockers")
    if not isinstance(blockers, list):
        errors.append(f"{path}: blockers must be a list")
    elif data.get("passed") is True and blockers:
        errors.append(f"{path}: passed results must not contain blockers")

    return errors


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", default="", help="Also require a result file for this milestone.")
    parser.add_argument("--require", action="store_true", help="Fail if --milestone has no result file.")
    parser.add_argument("--require-commit", action="store_true", help="Require non-UNKNOWN commits in result files.")
    args = parser.parse_args()

    if not RESULTS_DIR.exists():
        raise SystemExit("milestone_results/ is missing")

    files = sorted(path for path in RESULTS_DIR.glob("*.json"))
    if not files:
        raise SystemExit("no milestone result JSON files found")

    errors: list[str] = []
    if args.milestone:
        expected = RESULTS_DIR / f"{args.milestone}.json"
        if args.require and not expected.exists():
            errors.append(f"{expected}: required result ledger is missing")

    for path in files:
        errors.extend(validate_result(path, args.require_commit and path.name != "TEMPLATE.json"))

    if errors:
        print_list("milestone result ledger errors", errors)
        raise SystemExit(1)

    print(f"validated {len(files)} milestone result file(s)")


if __name__ == "__main__":
    main()
