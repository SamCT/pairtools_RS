use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const CHROMS: &str = "tests/fixtures/walks/walks.chrom.sizes";
const POLICIES: &[&str] = &["mask", "5any", "5unique", "3any", "3unique", "all"];
const CASES: &[(&str, u64)] = &[
    ("simple_non_walk", 30),
    ("r1_chimera_rescue_cis_convergent_near", 30),
    ("r2_chimera_rescue_cis_convergent_near", 30),
    ("chimera_no_rescue_trans", 30),
    ("chimera_no_rescue_wrong_orientation", 30),
    ("chimera_no_rescue_too_far", 30),
    ("chimera_inner_multi_rescue", 30),
    ("chimera_inner_null_rescue", 30),
    ("both_sides_chimeric_2x2", 30),
    ("multi_alignment_with_long_gap", 30),
    ("multi_alignment_with_long_gap", 300),
    ("no_unique_available_for_5unique", 30),
    ("no_unique_available_for_3unique", 30),
    ("all_policy_simple_three_alignments", 30),
];

fn oracle_stem(case_name: &str, policy: &str, gap: u64) -> String {
    if case_name == "multi_alignment_with_long_gap" && gap != 30 {
        format!("tests/oracle/walks/{case_name}.gap{gap}.{policy}")
    } else {
        format!("tests/oracle/walks/{case_name}.{policy}")
    }
}

fn normalize_pairsam(text: &str) -> String {
    text.lines()
        .filter(|line| {
            !line.starts_with("#command:")
                && !line.starts_with("#metadata:")
                && !line.starts_with("#date:")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn pair_type_counts(pairsam: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in pairsam.lines().filter(|line| !line.starts_with('#')) {
        let Some(pair_type) = line.split('\t').nth(7) else {
            panic!("pairsam row has fewer than 8 columns: {line}");
        };
        *counts.entry(pair_type.to_string()).or_insert(0) += 1;
    }
    counts
}

fn run_pairs_rs_parse(case_name: &str, policy: &str, gap: u64, stats_path: &Path) -> String {
    let bin = env!("CARGO_BIN_EXE_pairs-rs");
    let input = format!("tests/fixtures/walks/{case_name}.sam");
    let output = Command::new(bin)
        .args([
            "parse",
            "--chroms-path",
            CHROMS,
            "--assembly",
            "walks_test",
            "--min-mapq",
            "10",
            "--walks-policy",
            policy,
            "--max-inter-align-gap",
            &gap.to_string(),
            "--max-molecule-size",
            "750",
            "--report-alignment-end",
            "5",
            "--add-columns",
            "mapq,pos5,pos3,cigar,read_len",
            "--output-stats",
            stats_path.to_string_lossy().as_ref(),
            &input,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "pairs-rs failed for {case_name} {policy} gap {gap}:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn parse_walk_policies_match_pairtools_oracles() {
    for (case_name, gap) in CASES {
        for policy in POLICIES {
            let stem = oracle_stem(case_name, policy, *gap);
            let oracle_pairsam_path = format!("{stem}.pairsam");
            let oracle_stats_path = format!("{stem}.stats.txt");
            assert!(
                Path::new(&oracle_pairsam_path).exists(),
                "missing oracle pairsam; regenerate with bash tests/scripts/generate_walk_oracles.sh: {oracle_pairsam_path}"
            );
            assert!(
                Path::new(&oracle_stats_path).exists(),
                "missing oracle stats; regenerate with bash tests/scripts/generate_walk_oracles.sh: {oracle_stats_path}"
            );

            let tmp = TempDir::new().unwrap();
            let stats_path = tmp.path().join("candidate.stats.txt");
            let candidate = run_pairs_rs_parse(case_name, policy, *gap, &stats_path);
            let oracle = fs::read_to_string(&oracle_pairsam_path).unwrap();

            assert_eq!(
                normalize_pairsam(&candidate),
                normalize_pairsam(&oracle),
                "{case_name} {policy} gap {gap} pairsam mismatch"
            );
            assert_eq!(
                pair_type_counts(&candidate),
                pair_type_counts(&oracle),
                "{case_name} {policy} gap {gap} pair_type counts mismatch"
            );
            assert_eq!(
                fs::read_to_string(&stats_path).unwrap(),
                fs::read_to_string(&oracle_stats_path).unwrap(),
                "{case_name} {policy} gap {gap} stats mismatch"
            );
        }
    }
}
