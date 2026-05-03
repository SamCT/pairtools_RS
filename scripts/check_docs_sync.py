#!/usr/bin/env python3
"""Ensure milestone-required docs track source, script, and governance changes."""

from __future__ import annotations

import argparse

from governance_common import REPO_ROOT, changed_files, load_milestone, print_list


IMPLEMENTATION_OR_SCRIPT_PREFIXES = ("src/", "scripts/", "tests/", "benches/", "examples/")
IMPLEMENTATION_OR_CONFIG_FILES = {"Cargo.toml", "Cargo.lock", "pixi.toml", "pixi.lock", "build.rs"}
M000_REQUIRED_CHANGED = {
    "AGENTS.md",
    "docs/STATUS.md",
    "docs/PAIRTOOLS_COMPATIBILITY.md",
    "milestones/README.md",
}
STATUS_REQUIRED_TEXT = [
    "Active milestone",
    "Current branch",
    "Current commit",
    "Implemented behavior",
    "Intentionally unsupported behavior",
    "Validation performed",
    "Validation not performed and why",
    "Next recommended milestone",
]


def source_or_script_changed(files: list[str]) -> bool:
    return any(
        path.startswith(IMPLEMENTATION_OR_SCRIPT_PREFIXES) or path in IMPLEMENTATION_OR_CONFIG_FILES
        for path in files
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", required=True)
    args = parser.parse_args()

    milestone = load_milestone(args.milestone)
    files = changed_files()
    docs_required = set(milestone.get("docs_required", []))
    missing_docs = [path for path in docs_required if not (REPO_ROOT / path).exists()]
    if missing_docs:
        print_list("missing docs_required files", missing_docs)
        raise SystemExit(1)

    missing_changes = []
    if docs_required and source_or_script_changed(files):
        missing_changes.extend(sorted(docs_required - set(files)))
    if args.milestone == "M000":
        missing_changes.extend(sorted(M000_REQUIRED_CHANGED - set(files)))

    if missing_changes:
        print_list("required doc/governance files not changed", sorted(set(missing_changes)))
        raise SystemExit(1)

    status_path = REPO_ROOT / "docs/STATUS.md"
    status_text = status_path.read_text(encoding="utf-8") if status_path.exists() else ""
    missing_status = [token for token in STATUS_REQUIRED_TEXT if token not in status_text]
    if args.milestone not in status_text:
        missing_status.append(args.milestone)
    if missing_status:
        print_list("docs/STATUS.md missing required text", missing_status)
        raise SystemExit(1)

    print("docs sync check passed")


if __name__ == "__main__":
    main()
