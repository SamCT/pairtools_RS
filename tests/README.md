# Test Fixtures and Oracle Outputs

This directory is reserved for small deterministic fixtures and oracle outputs used by milestone-gated tests.

## Layout

- `fixtures/`: small SAM fixtures and matching chrom sizes.
- `oracle/`: pairtools oracle outputs.
- `golden/`: expected `pairs-rs` outputs after oracle parity is established.
- `scripts/`: tiny fixture regeneration or comparison helpers.

## Naming Convention

- `<milestone>_<case>.sam`
- `<milestone>_<case>.chrom.sizes`
- `<milestone>_<case>.pairtools.out`
- `<milestone>_<case>.pairs_rs.out`

Use lowercase case names with underscores. Keep outputs deterministic and small enough for review.

## Regenerating Oracles

Regenerate oracle outputs through explicit milestone commands, for example:

```bash
pixi run pairtools parse --help
```

Pairtools is allowed only as a test oracle or shell validation tool. It must not become a Rust runtime dependency.
