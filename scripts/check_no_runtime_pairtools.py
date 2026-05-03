#!/usr/bin/env python3
"""Reject Rust runtime shellouts to pairtools or other external genomics tools."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

from governance_common import REPO_ROOT, load_milestone, print_list


RUNTIME_PATTERNS = [
    re.compile(r'Command::new\s*\(\s*"(?P<cmd>pairtools|samtools|bgzip|gzip)"\s*\)'),
    re.compile(r"Command::new\s*\(\s*'(?P<cmd>pairtools|samtools|bgzip|gzip)'\s*\)"),
    re.compile(r"process::Command.*(?P<cmd>pairtools|samtools|bgzip|gzip)"),
    re.compile(r"std::process::Command.*(?P<cmd>pairtools|samtools|bgzip|gzip)"),
]


def strip_line_comment(line: str) -> str:
    return line.split("//", 1)[0]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", default="")
    args = parser.parse_args()

    allowed = set()
    if args.milestone:
        milestone = load_milestone(args.milestone)
        allowed.update(milestone.get("allowed_commands", []))

    offenders: list[str] = []
    src_root = REPO_ROOT / "src"
    for path in sorted(src_root.rglob("*.rs")):
        rel = path.relative_to(REPO_ROOT).as_posix()
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            code = strip_line_comment(line)
            for pattern in RUNTIME_PATTERNS:
                match = pattern.search(code)
                if match and match.group("cmd") not in allowed:
                    offenders.append(f"{rel}:{lineno}: {line.strip()}")

    if offenders:
        print_list("forbidden Rust runtime external tool calls", offenders)
        raise SystemExit(1)
    print("runtime external-tool check passed")


if __name__ == "__main__":
    main()
