#!/usr/bin/env python3
"""Prevent parse-lite framing from becoming current project direction."""

from __future__ import annotations

import argparse
import re

from governance_common import REPO_ROOT, print_list


PATTERN = re.compile(r"parse-lite|parse_lite|lite parser|reduced feature set", re.IGNORECASE)
LEGACY_FILES = {
    "scripts/TESTING_PARSE_LITE.md",
    "scripts/parse_lite_benchmark.sh",
    "scripts/run_parse_lite_pipeline.sh",
    "scripts/check_parse_lite_drift.py",
}
DOC_ALLOW = {"docs/STATUS.md", "docs/PAIRTOOLS_COMPATIBILITY.md"}


def allowed(rel: str, line: str) -> bool:
    lower = line.lower()
    if "check_parse_lite_drift.py" in line:
        return True
    if rel in LEGACY_FILES:
        return True
    if rel in DOC_ALLOW:
        return "legacy" in lower or "not the milestone authority" in lower
    if rel == "README.md":
        return "legacy" in lower or "not current" in lower or "not the current direction" in lower
    if rel == "AGENTS.md":
        return "legacy" in lower or "anti-drift" in lower or "not current" in lower
    if rel.startswith("milestones/"):
        return "historical" in lower or "anti-goal" in lower or "legacy" in lower
    return False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", default="")
    parser.parse_args()

    offenders = []
    for root in ["AGENTS.md", "README.md", "docs", "milestones", "scripts"]:
        path = REPO_ROOT / root
        paths = [path] if path.is_file() else sorted(path.rglob("*"))
        for child in paths:
            if not child.is_file():
                continue
            rel = child.relative_to(REPO_ROOT).as_posix()
            if child.suffix not in {".md", ".txt", ".json", ".py", ".sh"} and child.name != "ACTIVE_MILESTONE":
                continue
            for lineno, line in enumerate(child.read_text(encoding="utf-8", errors="ignore").splitlines(), start=1):
                if PATTERN.search(line) and not allowed(rel, line):
                    offenders.append(f"{rel}:{lineno}: {line.strip()}")

    if offenders:
        print_list("parse-lite drift offenders", offenders)
        raise SystemExit(1)
    print("parse-lite drift check passed")


if __name__ == "__main__":
    main()
