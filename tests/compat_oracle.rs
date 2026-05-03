use std::fs;
use std::process::Command;
fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap()
}
fn run(args: &[&str]) -> String {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let o = Command::new(bin).args(args).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap()
}
#[test]
fn parse_simple_matches_oracle() {
    assert_eq!(
        run(&[
            "parse",
            "-c",
            "tests/fixtures/parse_simple/chrom.sizes",
            "--drop-sam",
            "tests/fixtures/parse_simple/input.sam"
        ]),
        read("tests/oracle/parse_simple.expected.pairs")
    );
}
#[test]
fn sort_simple_matches_oracle() {
    assert_eq!(
        run(&["sort", "tests/fixtures/sort_simple/input.pairs"]),
        read("tests/oracle/sort_simple.expected.pairs")
    );
}
#[test]
fn parse_rejects_without_drop_sam() {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let o = Command::new(bin)
        .args([
            "parse",
            "-c",
            "tests/fixtures/parse_simple/chrom.sizes",
            "tests/fixtures/parse_simple/input.sam",
        ])
        .output()
        .unwrap();
    assert!(!o.status.success());
}
