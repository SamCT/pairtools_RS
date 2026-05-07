use std::collections::HashMap;
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

fn assert_pairtools_success(args: &[&str]) {
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

fn normalize_flip_output(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("#samheader: @PG\tID:pairtools_flip"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn normalize_markasdup_output(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("#samheader: @PG\tID:pairtools_markasdup"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn normalize_merge_output(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("#samheader: @PG\tID:pairtools_merge"))
        .map(|line| {
            if line.starts_with("#samheader: @PG\tID:bwa-") {
                line.split("\tCL:")
                    .next()
                    .unwrap_or(line)
                    .to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn normalize_split_output(text: &str) -> String {
    text.lines()
        .filter(|line| {
            !line.starts_with("#samheader: @PG\tID:pairtools_split")
                && !line.starts_with("@PG\tID:pairtools_split")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn body_read_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .filter_map(|line| line.split('\t').next().map(str::to_string))
        .collect()
}

fn pair_types_by_read_id(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            (fields[0].to_string(), fields[7].to_string())
        })
        .collect()
}

fn stats_map(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (key, value) = line.split_once('\t')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn normalize_stats_report(text: &str) -> String {
    let normalized_lines = text
        .lines()
        .map(|line| {
            if line.starts_with("summary/complexity_naive\t") {
                "summary/complexity_naive\t<complexity>".to_string()
            } else if line.trim_start().starts_with("complexity_naive: ") {
                format!(
                    "{}complexity_naive: <complexity>",
                    &line[..line.len() - line.trim_start().len()]
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>();

    let mut pair_type_lines: Vec<String> = normalized_lines
        .iter()
        .filter(|line| line.starts_with("pair_types/"))
        .cloned()
        .collect();
    pair_type_lines.sort();

    let mut inserted_pair_types = false;
    let mut out = Vec::new();
    for line in normalized_lines {
        if line.starts_with("pair_types/") {
            if !inserted_pair_types {
                out.extend(pair_type_lines.clone());
                inserted_pair_types = true;
            }
        } else {
            out.push(line);
        }
    }
    out.join("\n") + "\n"
}

fn assert_complexity_close(pairs_rs: &str, pairtools: &str) {
    let pairs_rs = stats_map(pairs_rs);
    let pairtools = stats_map(pairtools);
    let Some(left) = pairs_rs.get("summary/complexity_naive") else {
        return;
    };
    let Some(right) = pairtools.get("summary/complexity_naive") else {
        return;
    };
    let left: f64 = left.parse().unwrap();
    let right: f64 = right.parse().unwrap();
    assert!(
        (left - right).abs() < 1e-10,
        "complexity_naive differed too much: {left} vs {right}"
    );
}

fn assert_stats_report_matches(pairs_rs: &str, pairtools: &str) {
    assert_eq!(
        normalize_stats_report(pairs_rs),
        normalize_stats_report(pairtools)
    );
    assert_complexity_close(pairs_rs, pairtools);
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

fn bgzip_file(input: &Path, output: &Path) {
    let input_s = input.to_string_lossy();
    let bgzip = Command::new("pixi")
        .args(["run", "bgzip", "-c", input_s.as_ref()])
        .output()
        .unwrap();
    assert!(
        bgzip.status.success(),
        "bgzip -c failed:\n{}",
        String::from_utf8_lossy(&bgzip.stderr)
    );
    fs::write(output, bgzip.stdout).unwrap();
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
fn select_expression_engine_matches_pairtools_1_1_3_oracle() {
    for (condition, input) in [
        (
            "chrom1 == \"chr1\" and pos2 >= 20",
            "tests/data/mock.4stats.pairs",
        ),
        (
            "(pair_type == \"UU\" and pos1 <= 1) or (chrom1 == \"!\" and pair_type != \"WW\")",
            "tests/data/mock.4stats.pairs",
        ),
        (
            "not (pair_type == \"UU\") or pos1 > 100",
            "tests/data/mock.pairsam",
        ),
    ] {
        let args = ["select", condition, input];
        assert_eq!(
            normalize_select_output(&run_pairs_rs(&args)),
            normalize_select_output(&run_pairtools(&args)),
            "{condition} on {input}"
        );
    }
}

#[test]
fn select_output_rest_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4stats.pairs";
    let condition = "pos1 > 1 or pair_type == \"NU\"";
    let tmp = TempDir::new().unwrap();
    let pairs_rs_selected = tmp.path().join("pairs-rs.selected.pairs");
    let pairs_rs_rest = tmp.path().join("pairs-rs.rest.pairs");
    let pairtools_selected = tmp.path().join("pairtools.selected.pairs");
    let pairtools_rest = tmp.path().join("pairtools.rest.pairs");
    let pairs_rs_selected_s = pairs_rs_selected.to_string_lossy();
    let pairs_rs_rest_s = pairs_rs_rest.to_string_lossy();
    let pairtools_selected_s = pairtools_selected.to_string_lossy();
    let pairtools_rest_s = pairtools_rest.to_string_lossy();

    run_pairs_rs_to_path(&[
        "select",
        condition,
        "-o",
        pairs_rs_selected_s.as_ref(),
        "--output-rest",
        pairs_rs_rest_s.as_ref(),
        input,
    ]);
    assert_pairtools_success(&[
        "select",
        condition,
        "-o",
        pairtools_selected_s.as_ref(),
        "--output-rest",
        pairtools_rest_s.as_ref(),
        input,
    ]);

    assert_eq!(
        normalize_select_output(&fs::read_to_string(&pairs_rs_selected).unwrap()),
        normalize_select_output(&fs::read_to_string(&pairtools_selected).unwrap())
    );
    assert_eq!(
        normalize_select_output(&fs::read_to_string(&pairs_rs_rest).unwrap()),
        normalize_select_output(&fs::read_to_string(&pairtools_rest).unwrap())
    );
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
        &[
            "select",
            "readID.startswith(\"read\")",
            "tests/data/mock.4stats.pairs",
        ],
        "not implemented: pairtools select condition",
    );
    assert_pairs_rs_failure(
        &[
            "select",
            "(pair_type == \"UU\")",
            "--chrom-subset",
            "chr1",
            "tests/data/mock.4stats.pairs",
        ],
        "not implemented: pairtools select --chrom-subset",
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
fn flip_matches_pairtools_on_pairs_fixture() {
    let input = "tests/data/mock.4flip.pairs";
    let chroms = "tests/data/mock.chrom.sizes";
    let args = ["flip", "-c", chroms, input];
    assert_eq!(
        normalize_flip_output(&run_pairs_rs(&args)),
        normalize_flip_output(&run_pairtools(&args))
    );
}

#[test]
fn flip_supports_stdin_output_and_gz_output() {
    let input = "tests/data/mock.4flip.pairs";
    let chroms = "tests/data/mock.chrom.sizes";
    let expected = run_pairs_rs(&["flip", "-c", chroms, input]);
    let input_bytes = fs::read(input).unwrap();
    let from_stdin = run_pairs_rs_with_stdin(&["flip", "-c", chroms], &input_bytes);
    assert_eq!(
        normalize_flip_output(&from_stdin),
        normalize_flip_output(&expected)
    );

    let tmp = TempDir::new().unwrap();
    let plain = tmp.path().join("flipped.pairs");
    let gz = tmp.path().join("flipped.pairs.gz");
    let plain_s = plain.to_string_lossy();
    let gz_s = gz.to_string_lossy();

    run_pairs_rs_to_path(&["flip", "-c", chroms, "-o", plain_s.as_ref(), input]);
    assert_eq!(
        normalize_flip_output(&fs::read_to_string(&plain).unwrap()),
        normalize_flip_output(&expected)
    );

    run_pairs_rs_to_path(&["flip", "-c", chroms, "-o", gz_s.as_ref(), input]);
    assert_bgzip_compatible(&gz);
    assert_eq!(
        normalize_flip_output(&String::from_utf8(read_gzip_with_gzip(&gz)).unwrap()),
        normalize_flip_output(&expected)
    );
}

#[test]
fn flip_rejects_unsupported_features_loudly() {
    let input = "tests/data/mock.4flip.pairs";
    let chroms = "tests/data/mock.chrom.sizes";
    assert_pairs_rs_failure(
        &["flip", "-c", chroms, "--nproc-in", "2", input],
        "not implemented: pairtools flip --nproc-in",
    );
    assert_pairs_rs_failure(
        &["flip", "-c", chroms, "--nproc-out", "2", input],
        "not implemented: pairtools flip --nproc-out",
    );
    assert_pairs_rs_failure(
        &["flip", "-c", chroms, "--cmd-in", "cat", input],
        "not implemented: pairtools flip --cmd-in",
    );
    assert_pairs_rs_failure(
        &["flip", "-c", chroms, "--cmd-out", "cat", input],
        "not implemented: pairtools flip --cmd-out",
    );
}

#[test]
fn merge_simple_sorted_pairs_matches_pairtools_1_1_3_oracle() {
    let input = "tests/oracle/sort_simple.expected.pairs";
    let args = ["merge", input, input];
    assert_eq!(run_pairs_rs(&args), run_pairtools(&args));
}

#[test]
fn merge_sorted_pairsam_matches_pairtools_1_1_3_oracle() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("input.pairsam");
    fs::write(
        &input,
        "\
## pairs format v1.0.0
#sorted: chr1-chr2-pos1-pos2
#chromosomes: chr1 chr2
#chromsize: chr1 100
#chromsize: chr2 100
#samheader: @SQ\tSN:chr1\tLN:100
#samheader: @SQ\tSN:chr2\tLN:100
#samheader: @PG\tID:bwa\tPN:bwa\tVN:0.7.17\tCL:bwa mem ref r1 r2
#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type sam1 sam2
r1\tchr1\t1\tchr1\t2\t+\t-\tUU\t.\t.
r2\tchr1\t3\tchr2\t5\t+\t+\tUU\t.\t.
",
    )
    .unwrap();
    let input_s = input.to_string_lossy();
    let args = ["merge", input_s.as_ref(), input_s.as_ref()];
    assert_eq!(
        normalize_merge_output(&run_pairs_rs(&args)),
        normalize_merge_output(&run_pairtools(&args))
    );
}

#[test]
fn merge_writes_output_and_gz_output() {
    let input = "tests/oracle/sort_simple.expected.pairs";
    let expected = run_pairs_rs(&["merge", input, input]);
    let tmp = TempDir::new().unwrap();
    let plain = tmp.path().join("merged.pairs");
    let gz = tmp.path().join("merged.pairs.gz");
    let plain_s = plain.to_string_lossy();
    let gz_s = gz.to_string_lossy();

    run_pairs_rs_to_path(&["merge", "-o", plain_s.as_ref(), input, input]);
    assert_eq!(fs::read_to_string(&plain).unwrap(), expected);

    run_pairs_rs_to_path(&["merge", "-o", gz_s.as_ref(), input, input]);
    assert_bgzip_compatible(&gz);
    assert_eq!(read_gzip_with_gzip(&gz), expected.into_bytes());
}

#[test]
fn merge_rejects_unsupported_features_loudly() {
    assert_pairs_rs_failure(
        &["merge", "--nproc", "2", "tests/oracle/sort_simple.expected.pairs"],
        "not implemented: pairtools merge --nproc",
    );
    assert_pairs_rs_failure(
        &[
            "merge",
            "--tmpdir",
            "tmp",
            "tests/oracle/sort_simple.expected.pairs",
        ],
        "not implemented: pairtools merge --tmpdir",
    );
    assert_pairs_rs_failure(
        &[
            "merge",
            "--concatenate",
            "tests/oracle/sort_simple.expected.pairs",
        ],
        "not implemented: pairtools merge --concatenate",
    );
}

#[test]
fn markasdup_pairsam_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.pairsam";
    let args = ["markasdup", input];
    assert_eq!(
        normalize_markasdup_output(&run_pairs_rs(&args)),
        normalize_markasdup_output(&run_pairtools(&args))
    );
}

#[test]
fn markasdup_pairs_without_sam_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4flip.pairs";
    let args = ["markasdup", input];
    assert_eq!(
        normalize_markasdup_output(&run_pairs_rs(&args)),
        normalize_markasdup_output(&run_pairtools(&args))
    );
}

#[test]
fn markasdup_sets_pairsam_sam_duplicate_flags_and_yt_tags() {
    let output = run_pairs_rs(&["markasdup", "tests/data/mock.pairsam"]);
    let first_body = output
        .lines()
        .find(|line| !line.starts_with('#') && !line.is_empty())
        .unwrap();
    let fields: Vec<&str> = first_body.split('\t').collect();
    assert_eq!(fields[7], "DD");
    for sam_col in [8, 9] {
        let sam_fields: Vec<&str> = fields[sam_col].split('\x19').collect();
        let flag: u16 = sam_fields[1].parse().unwrap();
        assert!(flag & 0x400 != 0, "{sam_col} missing duplicate flag");
        assert!(
            sam_fields.contains(&"Yt:Z:DD"),
            "{sam_col} missing Yt:Z:DD"
        );
    }
}

#[test]
fn markasdup_supports_stdin_output_and_gz_output() {
    let input = "tests/data/mock.pairsam";
    let expected = run_pairs_rs(&["markasdup", input]);
    let input_bytes = fs::read(input).unwrap();
    let from_stdin = run_pairs_rs_with_stdin(&["markasdup"], &input_bytes);
    assert_eq!(
        normalize_markasdup_output(&from_stdin),
        normalize_markasdup_output(&expected)
    );

    let tmp = TempDir::new().unwrap();
    let plain = tmp.path().join("marked.pairsam");
    let gz = tmp.path().join("marked.pairsam.gz");
    let plain_s = plain.to_string_lossy();
    let gz_s = gz.to_string_lossy();

    run_pairs_rs_to_path(&["markasdup", "-o", plain_s.as_ref(), input]);
    assert_eq!(
        normalize_markasdup_output(&fs::read_to_string(&plain).unwrap()),
        normalize_markasdup_output(&expected)
    );

    run_pairs_rs_to_path(&["markasdup", "-o", gz_s.as_ref(), input]);
    assert_bgzip_compatible(&gz);
    assert_eq!(
        normalize_markasdup_output(&String::from_utf8(read_gzip_with_gzip(&gz)).unwrap()),
        normalize_markasdup_output(&expected)
    );
}

#[test]
fn markasdup_rejects_unsupported_features_loudly() {
    let input = "tests/data/mock.pairsam";
    assert_pairs_rs_failure(
        &["markasdup", "--nproc-in", "2", input],
        "not implemented: pairtools markasdup --nproc-in",
    );
    assert_pairs_rs_failure(
        &["markasdup", "--nproc-out", "2", input],
        "not implemented: pairtools markasdup --nproc-out",
    );
    assert_pairs_rs_failure(
        &["markasdup", "--cmd-in", "cat", input],
        "not implemented: pairtools markasdup --cmd-in",
    );
    assert_pairs_rs_failure(
        &["markasdup", "--cmd-out", "cat", input],
        "not implemented: pairtools markasdup --cmd-out",
    );
}

#[test]
fn split_pairsam_matches_pairtools_1_1_3_oracle() {
    let tmp = TempDir::new().unwrap();
    let pairs_rs_pairs = tmp.path().join("pairs-rs.pairs");
    let pairs_rs_sam = tmp.path().join("pairs-rs.sam");
    let pairtools_pairs = tmp.path().join("pairtools.pairs");
    let pairtools_sam = tmp.path().join("pairtools.sam");
    let pairs_rs_pairs_s = pairs_rs_pairs.to_string_lossy();
    let pairs_rs_sam_s = pairs_rs_sam.to_string_lossy();
    let pairtools_pairs_s = pairtools_pairs.to_string_lossy();
    let pairtools_sam_s = pairtools_sam.to_string_lossy();
    let input = "tests/data/mock.pairsam";

    run_pairs_rs_to_path(&[
        "split",
        "--output-pairs",
        pairs_rs_pairs_s.as_ref(),
        "--output-sam",
        pairs_rs_sam_s.as_ref(),
        input,
    ]);
    run_pairtools(&[
        "split",
        "--output-pairs",
        pairtools_pairs_s.as_ref(),
        "--output-sam",
        pairtools_sam_s.as_ref(),
        input,
    ]);

    assert_eq!(
        normalize_split_output(&fs::read_to_string(&pairs_rs_pairs).unwrap()),
        normalize_split_output(&fs::read_to_string(&pairtools_pairs).unwrap())
    );
    assert_eq!(
        normalize_split_output(&fs::read_to_string(&pairs_rs_sam).unwrap()),
        normalize_split_output(&fs::read_to_string(&pairtools_sam).unwrap())
    );
}

#[test]
fn split_supports_stdout_and_gz_pairs_output() {
    let input = "tests/data/mock.pairsam";
    let tmp = TempDir::new().unwrap();
    let sam = tmp.path().join("out.sam");
    let pairs_gz = tmp.path().join("out.pairs.gz");
    let sam_s = sam.to_string_lossy();
    let pairs_gz_s = pairs_gz.to_string_lossy();

    let stdout_pairs = run_pairs_rs(&[
        "split",
        "--output-pairs",
        "-",
        "--output-sam",
        sam_s.as_ref(),
        input,
    ]);
    assert!(stdout_pairs.contains("#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type\n"));
    assert!(fs::read_to_string(&sam).unwrap().contains("@SQ\tSN:chr1\tLN:100\n"));

    run_pairs_rs_to_path(&[
        "split",
        "--output-pairs",
        pairs_gz_s.as_ref(),
        "--output-sam",
        sam_s.as_ref(),
        input,
    ]);
    assert_bgzip_compatible(&pairs_gz);
    let decompressed = String::from_utf8(read_gzip_with_gzip(&pairs_gz)).unwrap();
    assert_eq!(
        normalize_split_output(&decompressed),
        normalize_split_output(&stdout_pairs)
    );
}

#[test]
fn split_rejects_unsupported_features_loudly() {
    let input = "tests/data/mock.pairsam";
    assert_pairs_rs_failure(
        &["split", "--nproc-in", "2", "--output-pairs", "out.pairs", input],
        "not implemented: pairtools split --nproc-in",
    );
    assert_pairs_rs_failure(
        &["split", "--cmd-out", "cat", "--output-pairs", "out.pairs", input],
        "not implemented: pairtools split --cmd-out",
    );
    assert_pairs_rs_failure(
        &["split", "--output-sam", "out.bam", input],
        "not implemented: pairtools split --output-sam .bam",
    );
}

#[test]
fn dedup_routes_marks_and_writes_simple_stats() {
    let input = "tests/fixtures/dedup_core/input.pairsam";
    let tmp = TempDir::new().unwrap();
    let nodups = tmp.path().join("nodups.pairsam");
    let dups = tmp.path().join("dups.pairsam.gz");
    let unmapped = tmp.path().join("unmapped.pairsam.gz");
    let stats = tmp.path().join("dedup.stats.txt");
    let nodups_s = nodups.to_string_lossy();
    let dups_s = dups.to_string_lossy();
    let unmapped_s = unmapped.to_string_lossy();
    let stats_s = stats.to_string_lossy();

    run_pairs_rs_to_path(&[
        "dedup",
        "--mark-dups",
        "--output-stats",
        stats_s.as_ref(),
        "--output-dups",
        dups_s.as_ref(),
        "--output-unmapped",
        unmapped_s.as_ref(),
        "-o",
        nodups_s.as_ref(),
        input,
    ]);

    assert_bgzip_compatible(&dups);
    assert_bgzip_compatible(&unmapped);
    assert_eq!(
        body_read_ids(&fs::read_to_string(&nodups).unwrap()),
        ["r_parent", "r_far", "r_unique"]
    );
    let dups_text = String::from_utf8(read_gzip_with_gzip(&dups)).unwrap();
    assert_eq!(body_read_ids(&dups_text), ["r_dup1", "r_dup2"]);
    assert_eq!(
        pair_types_by_read_id(&dups_text),
        [
            ("r_dup1".to_string(), "DD".to_string()),
            ("r_dup2".to_string(), "DD".to_string())
        ]
    );
    assert_eq!(
        body_read_ids(&String::from_utf8(read_gzip_with_gzip(&unmapped)).unwrap()),
        ["r_unmapped"]
    );
    assert_eq!(
        fs::read_to_string(&stats).unwrap(),
        "total\t6\ntotal_mapped\t5\ntotal_unmapped\t1\ntotal_dups\t2\ntotal_nodups\t3\nfraction_dups\t0.4\n"
    );
}

#[test]
fn dedup_routing_matches_pairtools_read_id_classes() {
    let input = "tests/fixtures/dedup_core/input.pairsam";
    let tmp = TempDir::new().unwrap();
    let rs_nodups = tmp.path().join("rs.nodups.pairsam");
    let rs_dups = tmp.path().join("rs.dups.pairsam");
    let rs_unmapped = tmp.path().join("rs.unmapped.pairsam");
    let pt_nodups = tmp.path().join("pt.nodups.pairsam");
    let pt_dups = tmp.path().join("pt.dups.pairsam");
    let pt_unmapped = tmp.path().join("pt.unmapped.pairsam");

    let rs_nodups_s = rs_nodups.to_string_lossy();
    let rs_dups_s = rs_dups.to_string_lossy();
    let rs_unmapped_s = rs_unmapped.to_string_lossy();
    run_pairs_rs_to_path(&[
        "dedup",
        "--mark-dups",
        "--output-dups",
        rs_dups_s.as_ref(),
        "--output-unmapped",
        rs_unmapped_s.as_ref(),
        "-o",
        rs_nodups_s.as_ref(),
        input,
    ]);

    let pt_nodups_s = pt_nodups.to_string_lossy();
    let pt_dups_s = pt_dups.to_string_lossy();
    let pt_unmapped_s = pt_unmapped.to_string_lossy();
    run_pairtools(&[
        "dedup",
        "--mark-dups",
        "--output-dups",
        pt_dups_s.as_ref(),
        "--output-unmapped",
        pt_unmapped_s.as_ref(),
        "-o",
        pt_nodups_s.as_ref(),
        input,
    ]);

    assert_eq!(
        body_read_ids(&fs::read_to_string(&rs_nodups).unwrap()),
        body_read_ids(&fs::read_to_string(&pt_nodups).unwrap())
    );
    assert_eq!(
        body_read_ids(&fs::read_to_string(&rs_dups).unwrap()),
        body_read_ids(&fs::read_to_string(&pt_dups).unwrap())
    );
    assert_eq!(
        body_read_ids(&fs::read_to_string(&rs_unmapped).unwrap()),
        body_read_ids(&fs::read_to_string(&pt_unmapped).unwrap())
    );
}

#[test]
fn dedup_marks_pairsam_sam_duplicate_flags_where_present() {
    let sep = '\x19';
    let input = format!(
        "\
## pairs format v1.0.0
#sorted: chr1-chr2-pos1-pos2
#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type sam1 sam2
parent\tchr1\t100\tchr1\t200\t+\t-\tUU\tparent{sep}65{sep}chr1{sep}100{sep}60{sep}10M{sep}*{sep}0{sep}0{sep}AAAAAAAAAA{sep}IIIIIIIIII{sep}Yt:Z:UU\tparent{sep}129{sep}chr1{sep}200{sep}60{sep}10M{sep}*{sep}0{sep}0{sep}TTTTTTTTTT{sep}IIIIIIIIII{sep}Yt:Z:UU
dup\tchr1\t101\tchr1\t202\t+\t-\tUU\tdup{sep}65{sep}chr1{sep}101{sep}60{sep}10M{sep}*{sep}0{sep}0{sep}AAAAAAAAAA{sep}IIIIIIIIII{sep}Yt:Z:UU\tdup{sep}129{sep}chr1{sep}202{sep}60{sep}10M{sep}*{sep}0{sep}0{sep}TTTTTTTTTT{sep}IIIIIIIIII{sep}Yt:Z:UU
"
    );
    let tmp = TempDir::new().unwrap();
    let input_path = tmp.path().join("input.pairsam");
    let nodups = tmp.path().join("nodups.pairsam");
    let dups = tmp.path().join("dups.pairsam");
    fs::write(&input_path, input).unwrap();

    let input_s = input_path.to_string_lossy();
    let nodups_s = nodups.to_string_lossy();
    let dups_s = dups.to_string_lossy();
    run_pairs_rs_to_path(&[
        "dedup",
        "--mark-dups",
        "--output-dups",
        dups_s.as_ref(),
        "-o",
        nodups_s.as_ref(),
        input_s.as_ref(),
    ]);
    let dups_text = fs::read_to_string(&dups).unwrap();
    let row = dups_text.lines().find(|line| !line.starts_with('#')).unwrap();
    let fields: Vec<&str> = row.split('\t').collect();
    assert_eq!(fields[7], "DD");
    assert!(fields[8].contains(&format!("{sep}1089{sep}")));
    assert!(fields[9].contains(&format!("{sep}1153{sep}")));
    assert!(fields[8].contains("Yt:Z:DD"));
    assert!(fields[9].contains("Yt:Z:DD"));
}

#[test]
fn dedup_rejects_unsupported_features_loudly() {
    let input = "tests/fixtures/dedup_core/input.pairsam";
    assert_pairs_rs_failure(
        &["dedup", "--backend", "scipy", input],
        "not implemented: pairtools dedup --backend",
    );
    assert_pairs_rs_failure(
        &["dedup", "--nproc-in", "2", input],
        "not implemented: pairtools dedup --nproc-in",
    );
    assert_pairs_rs_failure(
        &["dedup", "--send-header-to", "elsewhere", input],
        "not implemented: pairtools dedup --send-header-to elsewhere",
    );
}

#[test]
fn stats_full_tsv_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4stats.pairs";
    let args = ["stats", input];
    assert_stats_report_matches(&run_pairs_rs(&args), &run_pairtools(&args));
}

#[test]
fn stats_no_chromsizes_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4stats.pairs";
    let args = ["stats", "--no-chromsizes", input];
    assert_stats_report_matches(&run_pairs_rs(&args), &run_pairtools(&args));
}

#[test]
fn stats_n_dist_bins_decade_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4stats.pairs";
    let args = ["stats", "--n-dist-bins-decade", "1", input];
    assert_stats_report_matches(&run_pairs_rs(&args), &run_pairtools(&args));
}

#[test]
fn stats_yaml_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4stats.pairs";
    let args = ["stats", "--yaml", input];
    assert_eq!(
        normalize_stats_report(&run_pairs_rs(&args)),
        normalize_stats_report(&run_pairtools(&args))
    );
}

#[test]
fn stats_merge_matches_pairtools_1_1_3_oracle() {
    let input = "tests/data/mock.4stats.pairs";
    let tmp = TempDir::new().unwrap();
    let rs_one = tmp.path().join("rs.one.stats");
    let rs_two = tmp.path().join("rs.two.stats");
    let pt_one = tmp.path().join("pt.one.stats");
    let pt_two = tmp.path().join("pt.two.stats");
    let rs_one_s = rs_one.to_string_lossy();
    let rs_two_s = rs_two.to_string_lossy();
    let pt_one_s = pt_one.to_string_lossy();
    let pt_two_s = pt_two.to_string_lossy();

    run_pairs_rs_to_path(&["stats", "-o", rs_one_s.as_ref(), input]);
    run_pairs_rs_to_path(&["stats", "-o", rs_two_s.as_ref(), input]);
    run_pairtools(&["stats", "-o", pt_one_s.as_ref(), input]);
    run_pairtools(&["stats", "-o", pt_two_s.as_ref(), input]);

    let pairs_rs = run_pairs_rs(&[
        "stats",
        "--merge",
        rs_one_s.as_ref(),
        rs_two_s.as_ref(),
    ]);
    let pairtools = run_pairtools(&[
        "stats",
        "--merge",
        pt_one_s.as_ref(),
        pt_two_s.as_ref(),
    ]);
    assert_stats_report_matches(&pairs_rs, &pairtools);
}

#[test]
fn stats_writes_output_and_threaded_gz_output() {
    let input = "tests/data/mock.4stats.pairs";
    let expected = run_pairs_rs(&["stats", input]);
    let tmp = TempDir::new().unwrap();
    let plain = tmp.path().join("stats.txt");
    let gz = tmp.path().join("stats.txt.gz");
    let plain_s = plain.to_string_lossy();
    let gz_s = gz.to_string_lossy();

    run_pairs_rs_to_path(&["stats", "-o", plain_s.as_ref(), input]);
    assert_eq!(fs::read_to_string(&plain).unwrap(), expected);

    run_pairs_rs_to_path(&["stats", "--nproc-out", "2", "-o", gz_s.as_ref(), input]);
    assert_bgzip_compatible(&gz);
    assert_eq!(read_gzip_with_gzip(&gz), expected.into_bytes());
}

#[test]
fn stats_threaded_bgzf_input_matches_uncompressed() {
    let input = "tests/data/mock.4stats.pairs";
    let tmp = TempDir::new().unwrap();
    let gz_input = tmp.path().join("mock.4stats.pairs.gz");
    bgzip_file(Path::new(input), &gz_input);
    assert_bgzip_compatible(&gz_input);
    let gz_input_s = gz_input.to_string_lossy();

    let expected = run_pairs_rs(&["stats", input]);
    let observed = run_pairs_rs(&["stats", "--nproc-in", "2", gz_input_s.as_ref()]);
    assert_eq!(observed, expected);
}

#[test]
fn stats_rejects_unsupported_features_loudly() {
    let input = "tests/data/mock.4stats.pairs";
    assert_pairs_rs_failure(
        &["stats", "--merge", "--yaml", input],
        "not implemented: pairtools stats --merge --yaml",
    );
    assert_pairs_rs_failure(
        &["stats", "--yaml", "--no-yaml", input],
        "pairtools stats cannot use --yaml and --no-yaml together",
    );
    assert_pairs_rs_failure(
        &["stats", "--filter", "unique:(pair_type==\"UU\")", input],
        "not implemented: pairtools stats --filter",
    );
    assert_pairs_rs_failure(
        &["stats", "--cmd-in", "gzip -dc", input],
        "not implemented: pairtools stats --cmd-in",
    );
    assert_pairs_rs_failure(
        &["stats", "--cmd-out", "gzip", input],
        "not implemented: pairtools stats --cmd-out",
    );
    assert_pairs_rs_failure(
        &["stats", "--bytile-dups", input],
        "not implemented: pairtools stats --bytile-dups",
    );
    assert_pairs_rs_failure(
        &["stats", "--output-bytile-stats", "bytile.txt", input],
        "not implemented: pairtools stats --output-bytile-stats",
    );
    assert_pairs_rs_failure(
        &["stats", "--nproc-in", "0", input],
        "pairtools stats --nproc-in must be greater than zero",
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

    let merge_help = run_pairs_rs(&["merge", "--help"]);
    for option in [
        "--output",
        "--max-nmerge",
        "--tmpdir",
        "--memory",
        "--compress-program",
        "--nproc",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
        "--keep-first-header",
        "--no-keep-first-header",
        "--concatenate",
        "--no-concatenate",
    ] {
        assert!(merge_help.contains(option), "merge help missing {option}");
    }

    let dedup_help = run_pairs_rs(&["dedup", "--help"]);
    for option in [
        "--output",
        "--output-stats",
        "--output-dups",
        "--output-unmapped",
        "--mark-dups",
        "--no-mark-dups",
        "--max-mismatch",
        "--method",
        "--sep",
        "--send-header-to",
        "--c1",
        "--c2",
        "--p1",
        "--p2",
        "--unmapped-chrom",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
    ] {
        assert!(dedup_help.contains(option), "dedup help missing {option}");
    }

    let stats_help = run_pairs_rs(&["stats", "--help"]);
    for option in [
        "--output",
        "--merge",
        "--n-dist-bins-decade",
        "--with-chromsizes",
        "--no-chromsizes",
        "--yaml",
        "--no-yaml",
        "--bytile-dups",
        "--no-bytile-dups",
        "--output-bytile-stats",
        "--filter",
        "--engine",
        "--chrom-subset",
        "--startup-code",
        "--type-cast",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
    ] {
        assert!(stats_help.contains(option), "stats help missing {option}");
    }

    let split_help = run_pairs_rs(&["split", "--help"]);
    for option in [
        "--output-pairs",
        "--output-sam",
        "--nproc-in",
        "--nproc-out",
        "--cmd-in",
        "--cmd-out",
    ] {
        assert!(split_help.contains(option), "split help missing {option}");
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
        ("restrict", vec!["--frags", "frags.bed", "input.pairs"]),
        ("filterbycov", vec!["--max-cov", "3", "input.pairs"]),
        ("phase", vec!["--phase-suffixes", "PAT,MAT", "input.pairs"]),
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
