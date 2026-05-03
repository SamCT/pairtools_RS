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

fn assert_parse_fixture(name: &str, extra_args: &[&str]) {
    let chroms = format!("tests/fixtures/parse_milestone1/{name}/chrom.sizes");
    let input = format!("tests/fixtures/parse_milestone1/{name}/input.sam");
    let expected = format!("tests/oracle/parse_milestone1/{name}.expected.pairs");
    let mut args = vec!["parse", "-c", chroms.as_str(), "--drop-sam"];
    args.extend_from_slice(extra_args);
    args.push(input.as_str());
    assert_eq!(run(&args), read(&expected));
}

#[test]
fn parse_milestone1_simple_uu_pair() {
    assert_parse_fixture("simple_uu", &[]);
}

#[test]
fn parse_milestone1_unmapped_mate() {
    assert_parse_fixture("unmapped_mate", &[]);
}

#[test]
fn parse_milestone1_low_mapq_mate() {
    assert_parse_fixture("low_mapq_mate", &[]);
}

#[test]
fn parse_milestone1_reverse_strand_5prime_coordinate() {
    assert_parse_fixture("reverse_5prime", &["--report-alignment-end", "5"]);
}

#[test]
fn parse_milestone1_reverse_strand_3prime_coordinate() {
    assert_parse_fixture("reverse_3prime", &["--report-alignment-end", "3"]);
}

#[test]
fn parse_milestone1_soft_clipped_cigar() {
    assert_parse_fixture("soft_clipped", &[]);
}

#[test]
fn parse_milestone1_hard_clipped_cigar() {
    assert_parse_fixture("hard_clipped", &[]);
}

#[test]
fn parse_milestone1_indel_reference_span() {
    assert_parse_fixture("indel_ref_span", &[]);
}

#[test]
fn parse_milestone1_interchromosomal_flip() {
    assert_parse_fixture("interchrom_flip", &[]);
}

#[test]
fn parse_milestone1_same_chromosome_position_flip() {
    assert_parse_fixture("same_chrom_position_flip", &[]);
}

#[test]
fn parse_milestone1_secondary_alignment_present() {
    assert_parse_fixture("secondary_present", &[]);
}

#[test]
fn parse_milestone1_supplementary_alignment_present() {
    assert_parse_fixture("supplementary_present", &[]);
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

#[test]
fn parse_rejects_unimplemented_walk_policy() {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let o = Command::new(bin)
        .args([
            "parse",
            "-c",
            "tests/fixtures/parse_simple/chrom.sizes",
            "--drop-sam",
            "--walks-policy",
            "mask",
            "tests/fixtures/parse_simple/input.sam",
        ])
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("not implemented"));
}
