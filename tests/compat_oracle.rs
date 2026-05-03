use std::fs;
use std::process::Command;
fn read(path: &str) -> String { fs::read_to_string(path).unwrap() }
fn run(args:&[&str])->String{ let bin=env!("CARGO_BIN_EXE_pairs-rs"); let o=Command::new(bin).args(args).output().unwrap(); assert!(o.status.success()); String::from_utf8(o.stdout).unwrap() }
#[test] fn parse_simple_matches_committed_oracle(){assert_eq!(run(&["parse","-c","tests/fixtures/parse_simple/chrom.sizes","tests/fixtures/parse_simple/input.sam"]),read("tests/oracle/parse_simple.expected.pairs"));}
#[test] fn parse_reverse_3prime_fixture(){assert_eq!(run(&["parse","-c","tests/fixtures/parse_rev3/chrom.sizes","--report-alignment-end","3prime","tests/fixtures/parse_rev3/input.sam"]),read("tests/oracle/parse_rev3.expected.pairs"));}
#[test] fn parse_flip_by_chrom_order_fixture(){assert_eq!(run(&["parse","-c","tests/fixtures/parse_flip/chrom.sizes","tests/fixtures/parse_flip/input.sam"]),read("tests/oracle/parse_flip.expected.pairs"));}
#[test] fn sort_simple_matches_committed_oracle(){assert_eq!(run(&["sort","tests/fixtures/sort_simple/input.pairs"]),read("tests/oracle/sort_simple.expected.pairs"));}
