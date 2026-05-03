#!/usr/bin/env python3
"""List registered milestones."""

from __future__ import annotations

from governance_common import active_milestone, load_json, milestone_files


def main() -> None:
    active = active_milestone()
    print(f"active milestone: {active}")
    for path in milestone_files():
        data = load_json(path)
        marker = "*" if data["id"] == active else " "
        print(f"{marker} {data['id']} {data['status']}: {data['name']}")


if __name__ == "__main__":
    main()
