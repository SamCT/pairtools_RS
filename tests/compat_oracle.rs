use std::process::Command;
use std::sync::Mutex;
use tempfile::TempDir;

static ORACLE_LOCK: Mutex<()> = Mutex::new(());

fn run_pairs_rs(args: &[&str]) -> String {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let output = Command::new(bin).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "pairs-rs failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run_pairtools(args: &[&str]) -> String {
    let _guard = ORACLE_LOCK.lock().unwrap();
    let output = Command::new("pixi")
        .args(["run", "pairtools"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "pairtools oracle failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn assert_parse_fixture(name: &str, extra_args: &[&str]) {
    let chroms = format!("tests/fixtures/parse_milestone1/{name}/chrom.sizes");
    let input = format!("tests/fixtures/parse_milestone1/{name}/input.sam");

    let mut args = vec!["parse", "-c", chroms.as_str(), "--drop-sam"];
    args.extend_from_slice(extra_args);
    args.push(input.as_str());

    assert_eq!(run_pairs_rs(&args), run_pairtools(&args), "{name}");
}

fn assert_parse_fixture_pairsam(name: &str, extra_args: &[&str]) {
    let chroms = format!("tests/fixtures/parse_milestone1/{name}/chrom.sizes");
    let input = format!("tests/fixtures/parse_milestone1/{name}/input.sam");

    let mut args = vec!["parse", "-c", chroms.as_str()];
    args.extend_from_slice(extra_args);
    args.push(input.as_str());

    assert_eq!(run_pairs_rs(&args), run_pairtools(&args), "{name}");
}

#[test]
fn parse_milestone1_matches_pairtools_1_1_3_oracle() {
    let fixtures: &[(&str, &[&str])] = &[
        ("simple_uu", &[]),
        ("simple_uu", &["--assembly", "test_assembly"]),
        ("unmapped_mate", &[]),
        ("low_mapq_mate", &[]),
        ("reverse_5prime", &["--report-alignment-end", "5"]),
        ("reverse_3prime", &["--report-alignment-end", "3"]),
        ("soft_clipped", &[]),
        ("hard_clipped", &[]),
        ("indel_ref_span", &[]),
        ("interchrom_flip", &[]),
        ("same_chrom_position_flip", &[]),
        ("secondary_present", &[]),
        ("supplementary_present", &[]),
    ];

    for (name, extra_args) in fixtures {
        assert_parse_fixture(name, extra_args);
    }
}

#[test]
fn parse_pairsam_matches_pairtools_1_1_3_oracle() {
    let fixtures: &[(&str, &[&str])] = &[
        ("simple_uu", &[]),
        ("unmapped_mate", &[]),
        ("low_mapq_mate", &[]),
        ("secondary_present", &[]),
        ("supplementary_present", &[]),
        ("soft_clipped", &[]),
        ("hard_clipped", &[]),
        ("indel_ref_span", &[]),
    ];

    for (name, extra_args) in fixtures {
        assert_parse_fixture_pairsam(name, extra_args);
    }
}

#[test]
fn parse_add_columns_matches_pairtools_1_1_3_oracle() {
    let fixtures = [
        "simple_uu",
        "unmapped_mate",
        "low_mapq_mate",
        "soft_clipped",
        "hard_clipped",
        "indel_ref_span",
    ];
    for name in fixtures {
        assert_parse_fixture(name, &["--add-columns", "mapq,pos5,pos3,cigar,read_len"]);
    }
    assert_parse_fixture_pairsam(
        "simple_uu",
        &["--add-columns", "mapq,pos5,pos3,cigar,read_len"],
    );
}

#[test]
fn parse_max_inter_align_gap_matches_pairtools_1_1_3_oracle() {
    let chroms = "tests/fixtures/parse_milestone2/bwa_mem2_leading_gap/chrom.sizes";
    let input = "tests/fixtures/parse_milestone2/bwa_mem2_leading_gap/input.sam";
    for gap in ["30", "100"] {
        let args = [
            "parse",
            "-c",
            chroms,
            "--drop-sam",
            "--max-inter-align-gap",
            gap,
            input,
        ];
        assert_eq!(run_pairs_rs(&args), run_pairtools(&args), "gap {gap}");
    }
}

#[test]
fn parse_output_stats_matches_pairtools_1_1_3_oracle() {
    for name in ["simple_uu", "same_chrom_position_flip"] {
        let chroms = format!("tests/fixtures/parse_milestone1/{name}/chrom.sizes");
        let input = format!("tests/fixtures/parse_milestone1/{name}/input.sam");
        let tmp = TempDir::new().unwrap();
        let pairs_rs_stats = tmp.path().join("pairs-rs.stats");
        let pairtools_stats = tmp.path().join("pairtools.stats");
        let pairs_rs_stats_s = pairs_rs_stats.to_string_lossy();
        let pairtools_stats_s = pairtools_stats.to_string_lossy();

        let pairs_rs_args = [
            "parse",
            "-c",
            chroms.as_str(),
            "--output-stats",
            pairs_rs_stats_s.as_ref(),
            input.as_str(),
        ];
        let pairtools_args = [
            "parse",
            "-c",
            chroms.as_str(),
            "--output-stats",
            pairtools_stats_s.as_ref(),
            input.as_str(),
        ];

        assert_eq!(run_pairs_rs(&pairs_rs_args), run_pairtools(&pairtools_args));
        assert_eq!(
            std::fs::read_to_string(&pairs_rs_stats).unwrap(),
            std::fs::read_to_string(&pairtools_stats).unwrap(),
            "{name}"
        );
    }
}

#[test]
fn sort_simple_matches_pairtools_1_1_3_oracle() {
    let args = ["sort", "tests/fixtures/sort_simple/input.pairs"];
    assert_eq!(run_pairs_rs(&args), run_pairtools(&args));
}

fn assert_pairs_rs_failure(args: &[&str], expected_stderr: &str) {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let output = Command::new(bin).args(args).output().unwrap();
    assert!(!output.status.success(), "command unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_stderr),
        "stderr did not contain {expected_stderr:?}:\n{stderr}"
    );
}

#[test]
fn parse_rejects_unsupported_pairtools_options_loudly() {
    assert_pairs_rs_failure(
        &[
            "parse",
            "-c",
            "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes",
            "--drop-sam",
            "--walks-policy",
            "mask",
            "tests/fixtures/parse_milestone1/simple_uu/input.sam",
        ],
        "not implemented",
    );
    assert_pairs_rs_failure(
        &[
            "parse",
            "-c",
            "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes",
            "--drop-sam",
            "--drop-readid",
            "tests/fixtures/parse_milestone1/simple_uu/input.sam",
        ],
        "not implemented: pairtools parse --drop-readid",
    );
    assert_pairs_rs_failure(
        &[
            "parse",
            "-c",
            "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes",
            "--drop-sam",
            "--add-columns",
            "matched_bp",
            "tests/fixtures/parse_milestone1/simple_uu/input.sam",
        ],
        "not implemented: pairtools parse --add-columns matched_bp",
    );
}

#[test]
fn sort_rejects_accepted_but_unimplemented_pairtools_options_loudly() {
    assert_pairs_rs_failure(
        &[
            "sort",
            "--c1",
            "chrom1",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "not implemented: pairtools sort --c1",
    );
    assert_pairs_rs_failure(
        &[
            "sort",
            "--nproc",
            "2",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "not implemented: pairtools sort --nproc",
    );
    assert_pairs_rs_failure(
        &[
            "sort",
            "--cmd-in",
            "cat",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "not implemented: pairtools sort --cmd-in",
    );
}

#[test]
fn missing_pairtools_commands_exist_but_fail_loudly() {
    assert_pairs_rs_failure(
        &[
            "sample",
            "--seed",
            "1",
            "0.5",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "not implemented: pairtools sample compatibility",
    );
    assert_pairs_rs_failure(
        &[
            "header",
            "generate",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "not implemented: pairtools header compatibility",
    );
    assert_pairs_rs_failure(
        &["scaling", "tests/fixtures/sort_simple/input.pairs"],
        "not implemented: pairtools scaling compatibility",
    );
}
