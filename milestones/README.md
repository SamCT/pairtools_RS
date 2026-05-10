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

Registry documentation must stay synchronized with milestone JSON files. Any commit that adds, removes, renames, changes milestone status, or deliberately defers an active validation milestone must update this README in the same task.

Current registry:

- `M000-governance.json`: governance automation only.
- `M005-codex-autonomous-runner.json`: active-milestone runner command planning.
- `M006-milestone-result-ledger.json`: machine-readable result ledger planning.
- `M007-milestone-registry-sync.json`: registry sync and active milestone transition.
- `M010-cli-inventory.json`: CLI inventory only.
- `M020-parse-io.json`: parse input/output plumbing.
- `M030-parse-core-pairs.json`: ordinary paired-read parse formation.
- `M040-parse-pairsam-and-extra-columns.json`: pairsam and supported extra columns.
- `M050-parse-walks-and-chimeric-limits.json`: bounded walk/chimeric behavior.
- `M055-parse-walk-resolution.json`: pairtools parse walk-resolution policy parity.
- `M056-parse-all-walks.json`: `--walks-policy all` parse parity for committed fixtures.
- `M060-sort-core.json`: sort key/header/determinism.
- `M070-sort-compression-and-tempfiles.json`: sort compression, tempdir, and nproc behavior.
- `M080-hybrid-pipeline.json`: shell pipeline bridge.
- `M090-benchmarking.json`: benchmark harnesses after parity.
- `M100-downstream-command-planning.json`: downstream planning only.
- `M110-select-core.json`: scoped `select` implementation.
- `M120-merge-core.json`: scoped sorted-input `merge` implementation.
- `M130-stats-core.json`: scoped `stats` implementation.
- `M131-stats-report-parity.json`: pairtools-style stats report parity expansion.
- `M132-stats-io-and-merge.json`: stats merge, YAML, and BGZF I/O.
- `M140-split-core.json`: scoped `split` implementation, currently planned after registry sync.
- `M141-split-production-validation.json`: production-shaped split validation.
- `M150-dedup-core.json`: scoped sorted-input `dedup` implementation.
- `M151-dedup-production-validation.json`: production-shaped dedup validation.
- `M160-all-rust-hic-pipeline.json`: all-Rust pipeline shell orchestration.
- `M161-real-data-oracle-validation.json`: external real-data oracle validation; current active milestone using explicit external PT01 baseline metadata.
- `M162-threading-validation.json`: cross-tool threading option and determinism validation.
- `M170-flip-core.json`: scoped `flip` implementation, complete for committed oracle fixtures.
- `M171-markasdup-core.json`: scoped `markasdup` implementation, complete for committed oracle fixtures.
- `M180-select-expression-engine.json`: expanded safe select expression engine, complete for committed oracle fixtures.
- `M190-advanced-merge.json`: advanced merge options and temp planning.
- `M191-dedup-parity-expansion.json`: dedup parent IDs, extra-column matching, and richer stats.
- `M192-stats-filters-bytile-chrom-subsets.json`: stats filters, by-tile, type casts, and chrom subsets.
- `M193-sort-custom-columns-memory-semantics.json`: sort custom columns and memory semantics.
- `M194-cross-command-threaded-io.json`: threaded BGZF I/O expansion across eligible commands.
- `M200-filterbycov-core.json`: scoped `filterbycov` implementation.
- `M210-restrict-core.json`: scoped restriction-fragment annotation.
- `M220-sample-core.json`: deterministic `sample` implementation.
- `M230-header-subcommands.json`: scoped `header` subcommands.
- `M240-parse2-core.json`: scoped `parse2` implementation.
- `M250-phase-core.json`: scoped `phase` implementation.
- `M260-scaling-core.json`: scoped `scaling` implementation.
- `M300-full-pipeline-benchmark.json`: full-pipeline benchmark after real-data validation.
