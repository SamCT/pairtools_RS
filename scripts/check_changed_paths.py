#!/usr/bin/env python3
"""Check changed files against milestone path boundaries."""

from __future__ import annotations

import argparse

from governance_common import changed_files, first_matching_pattern, load_milestone


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", required=True)
    args = parser.parse_args()

    milestone = load_milestone(args.milestone)
    allowed = milestone.get("allowed_paths", [])
    forbidden = milestone.get("forbidden_paths", [])
    failed = False
    files = changed_files()

    for path in files:
        forbidden_match = first_matching_pattern(path, forbidden)
        allowed_match = first_matching_pattern(path, allowed)
        if forbidden_match:
            print(f"FORBIDDEN {path} matched {forbidden_match}")
            failed = True
        elif allowed_match:
            print(f"ALLOWED {path} matched {allowed_match}")
        else:
            print(f"FORBIDDEN {path} matched no allowed path")
            failed = True

    if failed:
        raise SystemExit(1)
    print(f"changed path check passed for {len(files)} file(s)")


if __name__ == "__main__":
    main()
