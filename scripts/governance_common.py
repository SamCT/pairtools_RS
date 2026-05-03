#!/usr/bin/env python3
"""Shared standard-library helpers for repository governance scripts."""

from __future__ import annotations

import datetime as dt
import fnmatch
import json
import re
import subprocess
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
MILESTONES_DIR = REPO_ROOT / "milestones"
STATE_DIR = REPO_ROOT / "target" / "codex_task_state"
PROCESS_PATTERN = re.compile(r"(cargo|rustc|cc|c\+\+|clang|ld|pairs-rs)")
ACTIVE_PROCESS_TOKEN = re.compile(
    r"(^|[/\s-])(cargo|rustc|rustdoc|cc|c\+\+|g\+\+|gcc|clang|ld|lld|mold|pairs-rs)(\s|$)"
)


def run(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=check,
    )


def git(args: list[str], check: bool = True) -> str:
    return run(["git", *args], check=check).stdout.strip()


def milestone_files() -> list[Path]:
    return sorted(MILESTONES_DIR.glob("M*.json"))


def milestone_path(milestone_id: str) -> Path:
    matches = sorted(MILESTONES_DIR.glob(f"{milestone_id}-*.json"))
    if not matches:
        raise SystemExit(f"milestone JSON not found for {milestone_id}")
    if len(matches) > 1:
        raise SystemExit(f"multiple milestone JSON files found for {milestone_id}: {matches}")
    return matches[0]


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_milestone(milestone_id: str) -> dict:
    data = load_json(milestone_path(milestone_id))
    if data.get("id") != milestone_id:
        raise SystemExit(f"{milestone_path(milestone_id)} id mismatch")
    return data


def active_milestone() -> str:
    path = MILESTONES_DIR / "ACTIVE_MILESTONE"
    if not path.exists():
        raise SystemExit("milestones/ACTIVE_MILESTONE is missing")
    return path.read_text(encoding="utf-8").strip()


def current_branch() -> str:
    return git(["rev-parse", "--abbrev-ref", "HEAD"])


def current_commit() -> str:
    return git(["rev-parse", "HEAD"])


def git_status_short() -> str:
    return run(["git", "status", "--short", "--branch"]).stdout.rstrip()


def _status_paths() -> set[str]:
    output = run(["git", "status", "--porcelain"]).stdout
    paths: set[str] = set()
    def add_path(raw_path: str) -> None:
        if raw_path.endswith("/"):
            directory = REPO_ROOT / raw_path
            if directory.is_dir():
                for child in directory.rglob("*"):
                    if child.is_file():
                        paths.add(child.relative_to(REPO_ROOT).as_posix())
                return
        paths.add(raw_path)

    for line in output.splitlines():
        if not line:
            continue
        path = line[3:]
        if " -> " in path:
            old_path, new_path = path.split(" -> ", 1)
            add_path(old_path)
            add_path(new_path)
        else:
            add_path(path)
    return paths


def changed_files() -> list[str]:
    paths: set[str] = set()
    for args in (["diff", "--name-only"], ["diff", "--cached", "--name-only"]):
        output = run(["git", *args]).stdout
        paths.update(line for line in output.splitlines() if line)
    paths.update(_status_paths())
    return sorted(paths)


def path_matches(path: str, pattern: str) -> bool:
    path = path.replace("\\", "/")
    pattern = pattern.replace("\\", "/")
    if pattern.endswith("/**"):
        prefix = pattern[:-3]
        return path == prefix.rstrip("/") or path.startswith(prefix)
    if pattern.endswith("/"):
        return path.startswith(pattern)
    if any(ch in pattern for ch in "*?["):
        return fnmatch.fnmatch(path, pattern)
    return path == pattern


def first_matching_pattern(path: str, patterns: Iterable[str]) -> str | None:
    for pattern in patterns:
        if path_matches(path, pattern):
            return pattern
    return None


def ps_ef() -> str:
    return subprocess.run(
        ["ps", "-ef"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    ).stdout


def matching_processes() -> list[str]:
    return [line for line in ps_ef().splitlines() if PROCESS_PATTERN.search(line)]


def active_build_processes() -> list[str]:
    ignore = (
        "systemd-journald",
        "scripts/milestone_gate.py",
        "scripts/check_",
        "scripts/codex_report.py",
        "scripts/cargo_guard.sh",
        "grep -E",
    )
    active = []
    for line in matching_processes():
        if any(token in line for token in ignore):
            continue
        if ACTIVE_PROCESS_TOKEN.search(line):
            active.append(line)
    return active


def state_path(milestone_id: str, phase: str) -> Path:
    return STATE_DIR / f"{milestone_id}.{phase}.json"


def tests_path(milestone_id: str) -> Path:
    return STATE_DIR / f"{milestone_id}.tests.jsonl"


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_test_results(milestone_id: str) -> list[dict]:
    path = tests_path(milestone_id)
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def print_list(label: str, values: Iterable[str]) -> None:
    values = list(values)
    print(f"{label}:")
    if values:
        for value in values:
            print(f"  - {value}")
    else:
        print("  - none")
