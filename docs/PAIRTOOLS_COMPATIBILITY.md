# PAIRTOOLS_COMPATIBILITY

Command inventory derived from repository CLI definitions (`pairtools/cli/*.py`).

## Commands

| Command | Status |
|---|---|
| parse | partial implementation |
| sort | partial implementation |
| parse2, dedup, flip, merge, split, select, stats, restrict, filterbycov, phase, markasdup, sample, header, scaling | explicitly `not implemented` |

## Parse options

| Option | Pairtools | Rust status |
|---|---|---|
| `-c/--chroms-path` | yes | implemented |
| `-o/--output` | yes | implemented |
| `--drop-sam` | yes | required for now; otherwise explicit error |
| `--min-mapq` | yes | implemented |
| `--walks-policy` (`mask,5any,5unique,3any,3unique,all`) | yes | accepted (selection logic in progress) |
| `--report-alignment-end` (`5,3`) | yes | implemented |
| `--max-inter-align-gap` | yes | not implemented |
| `--output-stats` | yes | implemented (basic total counter) |
| other parse options from pairtools (`--assembly`, `--drop-readid`, etc.) | yes | not implemented |

## Sort options

| Option | Pairtools | Rust status |
|---|---|---|
| `-o/--output` | yes | implemented |
| `--c1 --c2 --p1 --p2 --pt --extra-col` | yes | accepted, core keying uses default columns |
| `--nproc --tmpdir --memory` | yes | accepted; external merge implemented with chunk spills |
| `--compress-program --cmd-in --cmd-out` | yes | accepted but not wired |

## Notes
- SAM/BAM/CRAM input remains through rust-htslib/HTSlib.
- Non-implemented commands intentionally fail fast with `not implemented`.
