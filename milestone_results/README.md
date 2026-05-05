# Milestone Result Ledger

This directory stores machine-readable completion records for milestone tasks.

Each completed milestone should have a small JSON file named:

```text
milestone_results/<MILESTONE>.json
```

The ledger is evidence, not decoration. A milestone must not be claimed as passed unless the result JSON or status docs list the exact commands that were run.

Required fields:

- `milestone`: milestone ID, such as `M140`
- `commit`: commit SHA that contains the completed work
- `commands_run`: exact validation commands and outcomes
- `passed`: `true` only when all required validation passed
- `artifacts`: committed files, logs, or external artifact paths needed to interpret the result
- `blockers`: unresolved blockers, or an empty list

External data paths may be referenced, but large test data must not be committed.
