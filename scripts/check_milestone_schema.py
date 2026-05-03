#!/usr/bin/env python3
"""Validate milestone JSON files without external dependencies."""

from __future__ import annotations

import argparse
import re
import sys

from governance_common import MILESTONES_DIR, load_json, milestone_files


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-multiple-active", action="store_true")
    args = parser.parse_args()

    schema_path = MILESTONES_DIR / "schema.json"
    if not schema_path.exists():
        raise SystemExit("milestones/schema.json is missing")
    schema = load_json(schema_path)
    required = schema.get("required_fields", [])
    string_fields = set(schema.get("string_fields", []))
    list_fields = set(schema.get("list_fields", []))
    status_values = set(schema.get("status_values", []))
    errors: list[str] = []
    seen: dict[str, str] = {}
    active: list[str] = []

    files = milestone_files()
    if not files:
        errors.append("no milestones/M*.json files found")

    for path in files:
        try:
            data = load_json(path)
        except Exception as exc:  # noqa: BLE001
            errors.append(f"{path}: invalid JSON: {exc}")
            continue

        missing = [field for field in required if field not in data]
        if missing:
            errors.append(f"{path}: missing required fields: {', '.join(missing)}")

        for field in string_fields:
            if field in data and not isinstance(data[field], str):
                errors.append(f"{path}: field {field} must be a string")
        for field in list_fields:
            if field in data and not isinstance(data[field], list):
                errors.append(f"{path}: field {field} must be a list")

        milestone_id = data.get("id")
        if not isinstance(milestone_id, str) or not re.fullmatch(r"M\d{3}", milestone_id):
            errors.append(f"{path}: id must look like M000")
            continue
        if milestone_id in seen:
            errors.append(f"duplicate milestone id {milestone_id}: {seen[milestone_id]} and {path}")
        seen[milestone_id] = str(path)
        if not path.name.startswith(f"{milestone_id}-"):
            errors.append(f"{path}: filename must start with {milestone_id}-")
        if data.get("status") not in status_values:
            errors.append(f"{path}: status must be one of {sorted(status_values)}")
        if data.get("status") == "active":
            active.append(milestone_id)

    if "M000" not in seen:
        errors.append("M000 milestone is missing")
    if not args.allow_multiple_active and len(active) != 1:
        errors.append(f"expected exactly one active milestone, found {active}")

    if errors:
        for message in errors:
            print(f"ERROR: {message}", file=sys.stderr)
        raise SystemExit(1)

    print(f"validated {len(files)} milestone files")
    print(f"active milestones: {', '.join(active) if active else 'none'}")


if __name__ == "__main__":
    main()
