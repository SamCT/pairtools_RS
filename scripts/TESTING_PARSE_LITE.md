# Testing `pairs-rs` parse-lite with your own FASTQs

## Prerequisites
- `bwa-mem2` installed and index already built.
- `pairtools` installed.
- `samtools` installed (needed only when parser input is BAM).
- Rust toolchain (`cargo`) installed.

## Quick smoke test (small R1/R2)
```bash
cargo check
./scripts/run_parse_lite_pipeline.sh \
  /path/to/bwa/index/prefix \
  /path/to/sample_R1.fastq.gz \
  /path/to/sample_R2.fastq.gz \
  /path/to/chrom.sizes \
  sample_test \
  4
```

Outputs:
- `sample_test.pairs`
- `sample_test.sorted.pairs`
- `sample_test.dedup.pairs`

## Compare against traditional pairtools parse
Use the same alignments and compare line counts + timing:
```bash
# Rust parse-lite
bwa-mem2 mem -t 4 -5SP /path/to/index R1.fq.gz R2.fq.gz \
  | cargo run --release -- --no-header --walks-policy 5unique --drop-readid --nproc 4 > rust.pairs

# Traditional pairtools parse
bwa-mem2 mem -t 4 -5SP /path/to/index R1.fq.gz R2.fq.gz \
  | pairtools parse --no-sam-headers --walks-policy 5unique --drop-readid > py.pairs

wc -l rust.pairs py.pairs
```

## Optional benchmark helper
If you already have an input SAM/BAM, run:
```bash
./scripts/parse_lite_benchmark.sh /path/to/input.sam bench_out 4
# or
./scripts/parse_lite_benchmark.sh /path/to/input.bam bench_out 4
```


## Important parity note
This parse-lite binary currently accepts `--walks-policy`, but only `5unique` is implemented.
For strict comparison against Python `pairtools parse`, run both with `--walks-policy 5unique` and `--drop-readid`.
