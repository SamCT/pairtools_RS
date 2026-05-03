# Repository Guidance

This repository is a full pairtools-compatible Rust rewrite.

- No parse-lite behavior is acceptable.
- No no-op flags may be accepted.
- Pairtools may be used only as a test oracle, never at runtime.
- Use rust-htslib/HTSlib for SAM/BAM/CRAM input.
- Every accepted option must either be implemented exactly or fail loudly.
