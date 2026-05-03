#!/usr/bin/env python3
"""Record a milestone validation result as JSONL."""

from __future__ import annotations

import argparse
import json

from governance_common import tests_path, utc_now


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--milestone", required=True)
    parser.add_argument("--name", required=True)
    parser.add_argument("--status", required=True, choices=["pass", "fail", "not-run"])
    parser.add_argument("--command", default="")
    parser.add_argument("--reason", default="")
    parser.add_argument("--log-path", default="")
    args = parser.parse_args()

    row = {
        "timestamp": utc_now(),
        "milestone": args.milestone,
        "name": args.name,
        "status": args.status,
        "command": args.command,
        "reason": args.reason
    }
    if args.log_path:
        row["log_path"] = args.log_path

    path = tests_path(args.milestone)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(row, sort_keys=True) + "\n")
    print(f"recorded {args.status}: {args.name}")


if __name__ == "__main__":
    main()
