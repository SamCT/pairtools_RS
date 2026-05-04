# Downstream Command Milestones

M100 is the planning boundary for replacing pairtools downstream shell steps with Rust commands one milestone at a time. It does not implement command behavior.

## Sequence

1. M110 `select` core:
   - Implement `pairs-rs select '(pair_type == "UU")'`.
   - Preserve headers and stream selected records.
   - Support `-o/--output`.
   - Fail loudly for unsupported predicates and unsupported select options.
2. M120 `merge` core:
   - Merge already sorted pairs/pairsam inputs.
   - Preserve compatible headers.
   - Use deterministic stable merge behavior.
3. M130 `stats` core:
   - Implement stable counts needed by the hybrid pipeline.
   - Compare to pairtools oracle on small deterministic fixtures.
4. M140 `split` core:
   - Split pairsam into pairs output and SAM stream.
   - Preserve SAM columns and validate BAM handoff in shell tests.
5. M150 `dedup` planning and core:
   - Dedup requires a separate design pass because correctness depends on duplicate keys, parent IDs, duplicate marking, unmapped/dups output streams, and stats parity.

## Fixtures

Small downstream fixtures live in `tests/data/` today and should be copied or narrowed into milestone-specific fixtures as each command is implemented. Pairtools oracle outputs must remain small, deterministic, and committed only when they are needed by a guarded test.

## Oracle Policy

Pairtools may be used only in tests, shell validation, and oracle generation. Rust runtime code must not call pairtools, samtools, bgzip, or gzip.

## Non-Goals

Do not implement multiple downstream commands in one task. Do not claim all-Rust pipeline parity until merge, dedup, select, split, and stats each have their own oracle-tested milestones.
