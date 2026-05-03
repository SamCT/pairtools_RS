use std::process::Command;
use std::sync::Mutex;

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
            "--output-stats",
            "stats.txt",
            "tests/fixtures/parse_milestone1/simple_uu/input.sam",
        ],
        "not implemented: pairtools parse --output-stats",
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
