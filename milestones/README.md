# Milestone Registry

This directory is the machine-readable authority for Codex tasks in this repository.

`ACTIVE_MILESTONE` selects the default milestone for new work. A task may change it only when that same commit justifies the milestone transition in docs and passes the gate.

Each task must run:

```bash
python3 scripts/milestone_gate.py pre --milestone <ID>
```

before making changes. Before completion, run the milestone's required tests, record the test results, then run:

```bash
python3 scripts/milestone_gate.py post --milestone <ID>
python3 scripts/codex_report.py --milestone <ID>
```

The JSON files define goals, non-goals, allowed paths, forbidden paths, allowed commands, forbidden commands, oracle and candidate commands, required validation, required docs, and status fields. Prose documentation may explain the milestone workflow, but the JSON registry plus `scripts/milestone_gate.py` enforce it.

Current registry:

- `M000-governance.json`: governance automation only.
- `M010-cli-inventory.json`: CLI inventory only.
- `M020-parse-io.json`: parse input/output plumbing.
- `M030-parse-core-pairs.json`: ordinary paired-read parse formation.
- `M040-parse-pairsam-and-extra-columns.json`: pairsam and supported extra columns.
- `M050-parse-walks-and-chimeric-limits.json`: bounded walk/chimeric behavior.
- `M060-sort-core.json`: sort key/header/determinism.
- `M070-sort-compression-and-tempfiles.json`: sort compression, tempdir, and nproc behavior.
- `M080-hybrid-pipeline.json`: shell pipeline bridge.
- `M090-benchmarking.json`: benchmark harnesses after parity.
- `M100-downstream-command-planning.json`: downstream planning only.
