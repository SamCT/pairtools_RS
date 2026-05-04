use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
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

fn run_pairs_rs_with_stdin(args: &[&str], stdin: &[u8]) -> String {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "pairs-rs failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run_pairtools(args: &[&str]) -> String {
    let oracle_lock_guard = ORACLE_LOCK.lock().unwrap();
    let output = Command::new("pixi")
        .args(["run", "pairtools"])
        .args(args)
        .output()
        .unwrap();
    drop(oracle_lock_guard);
    assert!(
        output.status.success(),
        "pairtools oracle failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn assert_pairs_rs_success(args: &[&str]) {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let output = Command::new(bin).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "pairs-rs failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_pairs_rs_to_path(args: &[&str]) {
    assert_pairs_rs_success(args);
}

fn write_bam_from_sam(sam_path: &str, bam_path: &Path) {
    use rust_htslib::bam::{Format, Header, Read, Reader, Writer};

    let mut reader = Reader::from_path(sam_path).unwrap();
    let header = Header::from_template(reader.header());
    let mut writer = Writer::from_path(bam_path, &header, Format::Bam).unwrap();
    for record in reader.records() {
        writer.write(&record.unwrap()).unwrap();
    }
}

fn parsed_pairsam_fixture(name: &str, extra_args: &[&str]) -> String {
    let chroms = format!("tests/fixtures/parse_milestone1/{name}/chrom.sizes");
    let input = format!("tests/fixtures/parse_milestone1/{name}/input.sam");

    let mut args = vec!["parse", "-c", chroms.as_str()];
    args.extend_from_slice(extra_args);
    args.push(input.as_str());

    run_pairs_rs(&args)
}

fn parse_generated_pairsam_with_extra_columns() -> String {
    let mut header = Vec::new();
    let mut body = Vec::new();
    for name in [
        "unmapped_mate",
        "hard_clipped",
        "simple_uu",
        "interchrom_flip",
        "same_chrom_position_flip",
    ] {
        let parsed = parsed_pairsam_fixture(
            name,
            &[
                "--assembly",
                "test_assembly",
                "--add-columns",
                "mapq,pos5,pos3,cigar,read_len",
            ],
        );
        if header.is_empty() {
            header.extend(
                parsed
                    .lines()
                    .filter(|line| line.starts_with('#'))
                    .map(str::to_string),
            );
        }
        body.extend(
            parsed
                .lines()
                .filter(|line| !line.starts_with('#'))
                .map(str::to_string),
        );
    }
    body.reverse();

    let mut pairsam = String::new();
    for line in header {
        pairsam.push_str(&line);
        pairsam.push('\n');
    }
    for line in body {
        pairsam.push_str(&line);
        pairsam.push('\n');
    }
    pairsam
}

fn stable_equal_key_pairsam(rows: usize) -> String {
    let mut pairsam = String::from(
        "## pairs format v1.0.0\n#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type sam1 sam2 mapq1 mapq2 pos51 pos52 pos31 pos32 cigar1 cigar2 read_len1 read_len2\n",
    );
    for idx in 0..rows {
        pairsam.push_str(&format!(
            "r{idx:05}\tchr1\t10\tchr1\t10\t+\t-\tUU\tread{idx}\x19Yt:Z:UU\tread{idx}\x19Yt:Z:UU\t60\t60\t10\t10\t14\t14\t5M\t5M\t5\t5\n"
        ));
    }
    pairsam
}

fn read_gzip_with_gzip(path: &Path) -> Vec<u8> {
    let path_s = path.to_string_lossy();
    let output = Command::new("pixi")
        .args(["run", "gzip", "-dc", path_s.as_ref()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "gzip -dc failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn normalize_select_output(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("#samheader: @PG\tID:pairtools_select"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn assert_bgzip_compatible(path: &Path) {
    let path_s = path.to_string_lossy();
    let output = Command::new("pixi")
        .args(["run", "bgzip", "-t", path_s.as_ref()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "bgzip -t failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
fn parse_io_reads_stdin_sam_and_bam_path_like_sam_path() {
    let chroms = "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes";
    let sam = "tests/fixtures/parse_milestone1/simple_uu/input.sam";
    let sam_args = ["parse", "-c", chroms, "--drop-sam", sam];
    let expected = run_pairs_rs(&sam_args);

    let stdin_args = ["parse", "-c", chroms, "--drop-sam"];
    let sam_bytes = fs::read(sam).unwrap();
    assert_eq!(run_pairs_rs_with_stdin(&stdin_args, &sam_bytes), expected);

    let tmp = TempDir::new().unwrap();
    let bam = tmp.path().join("input.bam");
    write_bam_from_sam(sam, &bam);
    let bam_s = bam.to_string_lossy();
    let bam_args = ["parse", "-c", chroms, "--drop-sam", bam_s.as_ref()];
    assert_eq!(run_pairs_rs(&bam_args), expected);
}

#[test]
fn parse_io_writes_pairs_and_stats_to_explicit_files() {
    let chroms = "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes";
    let input = "tests/fixtures/parse_milestone1/simple_uu/input.sam";
    let tmp = TempDir::new().unwrap();
    let pairs_out = tmp.path().join("out.pairs");
    let stats_out = tmp.path().join("out.stats.txt");
    let pairs_out_s = pairs_out.to_string_lossy();
    let stats_out_s = stats_out.to_string_lossy();
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let output = Command::new(bin)
        .args([
            "parse",
            "-c",
            chroms,
            "--drop-sam",
            "-o",
            pairs_out_s.as_ref(),
            "--output-stats",
            stats_out_s.as_ref(),
            input,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "pairs-rs failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "stdout should be empty when -o is used");

    let pairs = fs::read_to_string(&pairs_out).unwrap();
    let stats = fs::read_to_string(&stats_out).unwrap();
    assert!(pairs.contains("#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type"));
    assert!(pairs.lines().any(|line| !line.starts_with('#')));
    assert!(stats.lines().any(|line| line.starts_with("total\t")));
}

#[test]
fn parse_io_rejects_compressed_output_boundaries_loudly() {
    let chroms = "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes";
    let input = "tests/fixtures/parse_milestone1/simple_uu/input.sam";
    let tmp = TempDir::new().unwrap();
    let pairs_gz = tmp.path().join("out.pairs.gz");
    let stats_gz = tmp.path().join("out.stats.txt.gz");
    let pairs_gz_s = pairs_gz.to_string_lossy();
    let stats_gz_s = stats_gz.to_string_lossy();

    assert_pairs_rs_failure(
        &[
            "parse",
            "-c",
            chroms,
            "--drop-sam",
            "-o",
            pairs_gz_s.as_ref(),
            input,
        ],
        "not implemented: compressed parse output",
    );
    assert_pairs_rs_failure(
        &[
            "parse",
            "-c",
            chroms,
            "--drop-sam",
            "--output-stats",
            stats_gz_s.as_ref(),
            input,
        ],
        "not implemented: compressed parse stats output",
    );
}

#[test]
fn sort_simple_matches_pairtools_1_1_3_oracle() {
    let args = ["sort", "tests/fixtures/sort_simple/input.pairs"];
    assert_eq!(run_pairs_rs(&args), run_pairtools(&args));
}

#[test]
fn select_pair_type_uu_matches_pairtools_1_1_3_oracle() {
    for input in ["tests/data/mock.4stats.pairs", "tests/data/mock.pairsam"] {
        let args = ["select", "(pair_type == \"UU\")", input];
        assert_eq!(
            normalize_select_output(&run_pairs_rs(&args)),
            normalize_select_output(&run_pairtools(&args)),
            "{input}"
        );
    }
}

#[test]
fn select_writes_output_and_gz_output() {
    let input = "tests/data/mock.4stats.pairs";
    let expected = run_pairs_rs(&["select", "(pair_type == \"UU\")", input]);
    let tmp = TempDir::new().unwrap();
    let plain = tmp.path().join("selected.pairs");
    let gz = tmp.path().join("selected.pairs.gz");
    let plain_s = plain.to_string_lossy();
    let gz_s = gz.to_string_lossy();

    run_pairs_rs_to_path(&[
        "select",
        "(pair_type == \"UU\")",
        "-o",
        plain_s.as_ref(),
        input,
    ]);
    assert_eq!(
        normalize_select_output(&fs::read_to_string(&plain).unwrap()),
        normalize_select_output(&expected)
    );

    run_pairs_rs_to_path(&[
        "select",
        "(pair_type == \"UU\")",
        "-o",
        gz_s.as_ref(),
        input,
    ]);
    assert_bgzip_compatible(&gz);
    assert_eq!(
        normalize_select_output(&String::from_utf8(read_gzip_with_gzip(&gz)).unwrap()),
        normalize_select_output(&expected)
    );
}

#[test]
fn select_rejects_unsupported_features_loudly() {
    assert_pairs_rs_failure(
        &["select", "chrom1 == \"chr1\"", "tests/data/mock.4stats.pairs"],
        "not implemented: pairtools select condition",
    );
    assert_pairs_rs_failure(
        &[
            "select",
            "(pair_type == \"UU\")",
            "--output-rest",
            "rest.pairs",
            "tests/data/mock.4stats.pairs",
        ],
        "not implemented: pairtools select --output-rest",
    );
    assert_pairs_rs_failure(
        &[
            "select",
            "(pair_type == \"UU\")",
            "--nproc-in",
            "2",
            "tests/data/mock.4stats.pairs",
        ],
        "not implemented: pairtools select --nproc-in",
    );
}

#[test]
fn sort_parse_generated_pairsam_matches_pairtools_1_1_3_oracle() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("parse-generated.pairsam");
    let tmpdir = tmp.path().join("sort-tmp");
    fs::create_dir(&tmpdir).unwrap();
    fs::write(&input, parse_generated_pairsam_with_extra_columns()).unwrap();

    let input_s = input.to_string_lossy();
    let tmpdir_s = tmpdir.to_string_lossy();
    let args = [
        "sort",
        "--nproc",
        "8",
        "--tmpdir",
        tmpdir_s.as_ref(),
        input_s.as_ref(),
    ];
    assert_eq!(run_pairs_rs(&args), run_pairtools(&args));
}

#[test]
fn sort_nproc_1_and_8_are_identical_and_stable_across_spills() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("stable-equal-key.pairsam");
    let tmpdir1 = tmp.path().join("sort-tmp-1");
    let tmpdir8 = tmp.path().join("sort-tmp-8");
    fs::create_dir(&tmpdir1).unwrap();
    fs::create_dir(&tmpdir8).unwrap();
    fs::write(&input, stable_equal_key_pairsam(20_050)).unwrap();

    let input_s = input.to_string_lossy();
    let tmpdir1_s = tmpdir1.to_string_lossy();
    let tmpdir8_s = tmpdir8.to_string_lossy();
    let out1 = run_pairs_rs(&[
        "sort",
        "--nproc",
        "1",
        "--tmpdir",
        tmpdir1_s.as_ref(),
        input_s.as_ref(),
    ]);
    let out8 = run_pairs_rs(&[
        "sort",
        "--nproc",
        "8",
        "--tmpdir",
        tmpdir8_s.as_ref(),
        input_s.as_ref(),
    ]);

    assert_eq!(out1, out8);
    for (idx, line) in out8
        .lines()
        .filter(|line| !line.starts_with('#'))
        .enumerate()
    {
        assert!(
            line.starts_with(&format!("r{idx:05}\t")),
            "equal-key rows were not emitted stably at body row {idx}: {line}"
        );
    }
}

#[test]
fn sort_writes_gz_output() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.pairsam");
    let output = tmp.path().join("sorted.pairsam.gz");
    fs::write(&input, parse_generated_pairsam_with_extra_columns()).unwrap();

    let input_s = input.to_string_lossy();
    let output_s = output.to_string_lossy();
    let expected = run_pairs_rs(&["sort", "--nproc", "4", input_s.as_ref()]);
    run_pairs_rs_to_path(&[
        "sort",
        "--nproc",
        "4",
        "-o",
        output_s.as_ref(),
        input_s.as_ref(),
    ]);
    assert_bgzip_compatible(&output);
    assert_eq!(read_gzip_with_gzip(&output), expected.into_bytes());
}

#[test]
fn sort_gz_nproc_1_and_8_decompress_identically() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("stable-equal-key.pairsam");
    let out1 = tmp.path().join("sorted.nproc1.pairsam.gz");
    let out8 = tmp.path().join("sorted.nproc8.pairsam.gz");
    fs::write(&input, stable_equal_key_pairsam(20_050)).unwrap();

    let input_s = input.to_string_lossy();
    let out1_s = out1.to_string_lossy();
    let out8_s = out8.to_string_lossy();
    run_pairs_rs_to_path(&[
        "sort",
        "--nproc",
        "1",
        "-o",
        out1_s.as_ref(),
        input_s.as_ref(),
    ]);
    run_pairs_rs_to_path(&[
        "sort",
        "--nproc",
        "8",
        "-o",
        out8_s.as_ref(),
        input_s.as_ref(),
    ]);

    assert_bgzip_compatible(&out1);
    assert_bgzip_compatible(&out8);
    assert_eq!(read_gzip_with_gzip(&out1), read_gzip_with_gzip(&out8));
}

#[test]
fn sort_uses_tmpdir_for_spilled_chunks() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.pairsam");
    let missing_tmpdir = tmp.path().join("missing-sort-tmp");
    fs::write(&input, parse_generated_pairsam_with_extra_columns()).unwrap();

    let input_s = input.to_string_lossy();
    let missing_tmpdir_s = missing_tmpdir.to_string_lossy();
    assert_pairs_rs_failure(
        &[
            "sort",
            "--tmpdir",
            missing_tmpdir_s.as_ref(),
            input_s.as_ref(),
        ],
        "No such file",
    );
}

#[test]
fn sort_updates_existing_samheader_pg_chain() {
    let input = "## pairs format v1.0.0\n#samheader: @HD\tVN:1.6\tSO:unsorted\n#samheader: @SQ\tSN:chr1\tLN:100\n#samheader: @PG\tID:bwa\tPN:bwa\tVN:0.7.17\n#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type sam1 sam2\nr1\tchr1\t1\tchr1\t2\t+\t-\tUU\t.\t.\n";
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("with-pg.pairsam");
    fs::write(&path, input).unwrap();

    let path_s = path.to_string_lossy();
    let sorted = run_pairs_rs(&["sort", path_s.as_ref()]);
    assert!(sorted.contains("#sorted: chr1-chr2-pos1-pos2\n"));
    assert!(sorted.contains("#samheader: @PG\tID:pairtools_sort\tPN:pairtools_sort\tCL:"));
    assert!(sorted.contains("\tPP:bwa\tVN:1.1.3\n"));
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
fn cli_inventory_lists_current_pairtools_command_surface() {
    let help = run_pairs_rs(&["--help"]);
    for command in [
        "parse",
        "sort",
        "parse2",
        "dedup",
        "flip",
        "merge",
        "split",
        "select",
        "stats",
        "restrict",
        "filterbycov",
        "phase",
        "markasdup",
        "sample",
        "header",
        "scaling",
    ] {
        assert!(help.contains(command), "top-level help missing {command}");
    }

    let parse_help = run_pairs_rs(&["parse", "--help"]);
    for option in [
        "--chroms-path",
        "--output",
        "--assembly",
        "--min-mapq",
        "--max-molecule-size",
        "--drop-readid",
        "--drop-seq",
        "--drop-sam",
        "--add-pair-index",
        "--add-columns",
        "--output-parsed-alignments",
        "--output-stats",
        "--report-alignment-end",
        "--max-inter-align-gap",
        "--walks-policy",
        "--readid-transform",
        "--flip",
        "--no-flip",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
    ] {
        assert!(parse_help.contains(option), "parse help missing {option}");
    }

    let sort_help = run_pairs_rs(&["sort", "--help"]);
    for option in [
        "--output",
        "--tmpdir",
        "--memory",
        "--c1",
        "--c2",
        "--p1",
        "--p2",
        "--pt",
        "--extra-col",
        "--nproc",
        "--compress-program",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
    ] {
        assert!(sort_help.contains(option), "sort help missing {option}");
    }

    let select_help = run_pairs_rs(&["select", "--help"]);
    for option in [
        "--output",
        "--output-rest",
        "--chrom-subset",
        "--startup-code",
        "--type-cast",
        "--remove-columns",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
    ] {
        assert!(select_help.contains(option), "select help missing {option}");
    }
}

#[test]
fn unsupported_top_level_options_fail_loudly() {
    assert_pairs_rs_failure(
        &["--post-mortem", "stats"],
        "not implemented: top-level --post-mortem",
    );
    assert_pairs_rs_failure(
        &["--output-profile", "profile.txt", "stats"],
        "not implemented: top-level --output-profile",
    );
    assert_pairs_rs_failure(&["--verbose", "stats"], "not implemented: top-level --verbose");
    assert_pairs_rs_failure(&["--debug", "stats"], "not implemented: top-level --debug");
}

#[test]
fn parse_rejects_unsupported_pairtools_options_loudly() {
    assert_pairs_rs_failure(
        &[
            "parse",
            "-c",
            "tests/fixtures/parse_milestone1/simple_uu/chrom.sizes",
            "--drop-sam",
            "--no-flip",
            "tests/fixtures/parse_milestone1/simple_uu/input.sam",
        ],
        "not implemented: pairtools parse --no-flip",
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
fn parse_rejects_non_adjacent_read_names_loudly() {
    let tmp = TempDir::new().unwrap();
    let chroms = tmp.path().join("chrom.sizes");
    let input = tmp.path().join("non-adjacent.sam");
    fs::write(&chroms, "chr1\t1000\n").unwrap();
    fs::write(
        &input,
        "\
@HD\tVN:1.6\tSO:unsorted
@SQ\tSN:chr1\tLN:1000
r1\t99\tchr1\t10\t60\t10M\t=\t50\t40\tAAAAAAAAAA\tIIIIIIIIII
r2\t99\tchr1\t20\t60\t10M\t=\t60\t40\tCCCCCCCCCC\tIIIIIIIIII
r1\t147\tchr1\t50\t60\t10M\t=\t10\t-40\tTTTTTTTTTT\tIIIIIIIIII
",
    )
    .unwrap();

    let chroms_s = chroms.to_string_lossy();
    let input_s = input.to_string_lossy();
    assert_pairs_rs_failure(
        &["parse", "-c", chroms_s.as_ref(), input_s.as_ref()],
        "not implemented: pairs-rs parse requires query-name grouped input",
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
            "0",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "--nproc must be greater than zero",
    );
    assert_pairs_rs_failure(
        &[
            "sort",
            "--memory",
            "1G",
            "tests/fixtures/sort_simple/input.pairs",
        ],
        "not implemented: pairtools sort --memory",
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
    for (command, args) in [
        ("parse2", vec!["--single-end", "input.sam"]),
        ("dedup", vec!["--output", "out.pairs", "input.pairs"]),
        ("flip", vec!["--chroms-path", "chrom.sizes", "input.pairs"]),
        ("merge", vec!["--nproc", "2", "a.pairs", "b.pairs"]),
        ("split", vec!["--output-pairs", "out.pairs", "input.pairsam"]),
        ("stats", vec!["--with-chromsizes", "chrom.sizes", "input.pairs"]),
        ("restrict", vec!["--frags", "frags.bed", "input.pairs"]),
        ("filterbycov", vec!["--max-cov", "3", "input.pairs"]),
        ("phase", vec!["--phase-suffixes", "PAT,MAT", "input.pairs"]),
        ("markasdup", vec!["input.pairsam"]),
        ("sample", vec!["--seed", "1", "0.5", "input.pairs"]),
        ("header", vec!["generate", "input.pairs"]),
        ("scaling", vec!["input.pairs"]),
    ] {
        let mut all_args = vec![command];
        all_args.extend(args);
        assert_pairs_rs_failure(
            &all_args,
            &format!("not implemented: pairtools {command} compatibility"),
        );
    }
}
