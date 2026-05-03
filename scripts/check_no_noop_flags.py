#!/usr/bin/env python3
"""Guard against accepted-but-unimplemented compatibility behavior."""

from __future__ import annotations

import argparse
import re

from governance_common import REPO_ROOT, changed_files, print_list


PHRASES = [
    "no-op",
    "noop",
    "ignored for compatibility",
    "accepted but ignored",
    "silently ignored",
    "placeholder implementation",
    "fake support",
    "TODO compatibility",
    "stubbed as success",
]

POLICY_ALLOW = [
    "forbidden",
    "must not",
    "not allowed",
    "no no-op",
    "no accepted",
    "not accepted",
    "fail loudly",
    "may not be accepted",
    "reject",
    "guard against",
    "anti-drift",
]

SCAN_ROOTS = ["src", "tests", "docs", "scripts", "AGENTS.md", "README.md"]
SKIP_FILES = {"scripts/check_no_noop_flags.py"}
RUST_HEURISTICS = [
    re.compile(r"#\s*\[\s*allow\s*\(\s*unused"),
    re.compile(r"\blet\s+_[A-Za-z0-9_]*\s*="),
    re.compile(r"=>\s*\{\s*Ok\s*\(\s*\(\s*\)\s*\)\s*\}"),
]


def iter_files() -> list[str]:
    files: list[str] = []
    for root in SCAN_ROOTS:
        path = REPO_ROOT / root
        if path.is_file():
            files.append(root)
        elif path.is_dir():
            for child in sorted(path.rglob("*")):
                if child.is_file() and child.suffix in {".rs", ".py", ".sh", ".md", ".txt", ".toml", ".yml", ".yaml"}:
                    files.append(child.relative_to(REPO_ROOT).as_posix())
    return sorted(set(files))


def phrase_allowed(rel: str, line: str) -> bool:
    lower = line.lower()
    if "check_no_noop_flags.py" in line:
        return True
    if rel in SKIP_FILES:
        return True
    if rel.endswith((".md", ".txt")) or rel == "AGENTS.md" or rel == "README.md":
        return any(token in lower for token in POLICY_ALLOW)
    if rel.startswith("scripts/") and any(token in lower for token in POLICY_ALLOW):
        return True
    return False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", default="")
    parser.parse_args()

    changed_rust = {path for path in changed_files() if path.endswith(".rs")}
    offenders: list[str] = []
    warnings: list[str] = []

    for rel in iter_files():
        if rel in SKIP_FILES:
            continue
        text = (REPO_ROOT / rel).read_text(encoding="utf-8", errors="ignore")
        for lineno, line in enumerate(text.splitlines(), start=1):
            lower = line.lower()
            if any(phrase.lower() in lower for phrase in PHRASES) and not phrase_allowed(rel, line):
                offenders.append(f"{rel}:{lineno}: {line.strip()}")
            if rel.endswith(".rs"):
                for heuristic in RUST_HEURISTICS:
                    if heuristic.search(line):
                        entry = f"{rel}:{lineno}: {line.strip()}"
                        if rel in changed_rust:
                            offenders.append(entry)
                        else:
                            warnings.append(entry)

    if warnings:
        print_list("pre-existing Rust compatibility heuristics to inspect", warnings)
    if offenders:
        print_list("compatibility placeholder offenders", offenders)
        raise SystemExit(1)
    print("compatibility placeholder check passed")


if __name__ == "__main__":
    main()
