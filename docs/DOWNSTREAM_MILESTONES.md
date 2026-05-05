# Downstream Command Milestones

M100 is the planning boundary for replacing pairtools downstream shell steps with Rust commands one milestone at a time. It does not implement command behavior.

## Sequence

1. M110 `select` core (scoped implementation present):
   - Implement `pairs-rs select '(pair_type == "UU")'`.
   - Preserve headers and stream selected records.
   - Support `-o/--output`.
   - Fail loudly for unsupported predicates and unsupported select options.
2. M120 `merge` core (scoped implementation present):
   - Merge already sorted pairs/pairsam inputs.
   - Preserve compatible headers.
   - Use deterministic stable merge behavior.
3. M150 `dedup` core (scoped implementation present):
   - Route nodups, duplicates, and unmapped pairs from sorted pairs/pairsam input.
   - Support the real pipeline output streams and simple stats.
   - Mark duplicate pair records with `DD`.
4. M130 `stats` core (scoped implementation present; M131/M132 expanded report and I/O coverage):
   - Implement stable counts needed by the hybrid pipeline.
   - Compare to pairtools oracle on small deterministic fixtures.
5. M140 `split` core (next missing downstream command):
   - Split pairsam into pairs output and SAM stream.
   - Preserve SAM columns and validate BAM handoff in shell tests.

## Production Validation Milestones

- M141 `split` production validation:
  - Validate the exact production-shaped split command and BAM handoff on small pipeline-style pairsam.
  - Do not broaden split behavior beyond the M140 scoped implementation without a follow-up milestone.
- M160 all-Rust Hi-C pipeline:
  - Wire implemented Rust parse, sort, merge, dedup, select, split, and stats commands into an all-Rust shell pipeline.
  - Do not claim production parity until M161 passes.
- M161 real-data oracle validation:
  - Run the all-Rust pipeline on external real-data fixtures and compare against the pairtools oracle outputs.
  - This milestone gates optimization claims.
- M300 full-pipeline benchmark:
  - Benchmark only after M161 real-data validation passes.
  - Report exact commands, artifacts, wall time, CPU, memory, disk, output size, and throughput.

## Post-Validation Parity Candidates

- M170 flip core
- M171 markasdup core
- M180 select expression engine
- M190 advanced merge
- M191 dedup parity expansion
- M192 stats filters/bytile/chrom subsets
- M193 sort custom columns/memory semantics
- M200 filterbycov core
- M210 restrict core
- M220 sample core
- M230 header subcommands
- M240 parse2 core
- M250 phase core
- M260 scaling core

## Fixtures

Small downstream fixtures live in `tests/data/` today and should be copied or narrowed into milestone-specific fixtures as each command is implemented. Pairtools oracle outputs must remain small, deterministic, and committed only when they are needed by a guarded test.

## Oracle Policy

Pairtools may be used only in tests, shell validation, and oracle generation. Rust runtime code must not call pairtools, samtools, bgzip, or gzip.

## Non-Goals

Do not implement multiple downstream commands in one task. Do not claim all-Rust pipeline parity until merge, dedup, select, split, and stats each have their own oracle-tested milestones.
