#!/usr/bin/env bash
set -euo pipefail

rustc --version
cargo --version
samtools --version | head -n 1
pairtools --version
bwa 2>&1 | head -n 1

cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release

./target/release/pairs-rs --help
python scripts/compare_pairs.py --help
