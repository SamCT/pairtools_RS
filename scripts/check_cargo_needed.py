#!/usr/bin/env python3
"""Decide whether Cargo validation is required for the current diff."""

from __future__ import annotations

import argparse

from governance_common import changed_files, first_matching_pattern, load_milestone


DOC_SUFFIXES = (".md", ".rst", ".txt")


def files_from_args(path: str | None) -> list[str]:
    if not path:
        return changed_files()
    with open(path, encoding="utf-8") as handle:
        return sorted(line.strip() for line in handle if line.strip())


def is_doc_only_file(path: str) -> bool:
    return path.endswith(DOC_SUFFIXES) or path.startswith("docs/")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", required=True)
    parser.add_argument("--from-file")
    args = parser.parse_args()

    milestone = load_milestone(args.milestone)
    files = files_from_args(args.from_file)
    patterns = milestone.get("cargo_required_if_paths_changed", [])
    reasons = []
    for path in files:
        pattern = first_matching_pattern(path, patterns)
        if not pattern:
            continue
        if is_doc_only_file(path):
            continue
        reasons.append(f"{path} matched {pattern}")

    required = bool(reasons)
    print(f"cargo_required={'true' if required else 'false'}")
    if reasons:
        print("reason:")
        for reason in reasons:
            print(f"  - {reason}")
        print("recommended_command=scripts/cargo_guard.sh check")
    else:
        print("reason: changed files are documentation/governance only or outside Cargo-trigger paths")


if __name__ == "__main__":
    main()
