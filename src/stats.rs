use crate::cli::StatsArgs;
use rust_htslib::htslib;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::{c_int, c_void};
use std::path::{Path, PathBuf};

const UNMAPPED_CHROM: &str = "!";
const STRANDS: [&str; 4] = ["+-", "-+", "--", "++"];
const CONVERGENCE_STRANDS: [&str; 4] = ["++", "--", "-+", "+-"];
const CONVERGENCE_THRESHOLD: f64 = 0.05;

pub fn cmd_stats(args: StatsArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_stats_options(&args)?;
    let options = StatsOptions::from_args(&args)?;
    let mut stats = if args.merge {
        merge_stats_inputs(&args.inputs, &options)?
    } else {
        let mut stats = PairsStats::new(options.n_dist_bins_decade);
        for input in stats_inputs(&args.inputs) {
            read_pairs_input(input.as_deref(), &mut stats, &options)?;
        }
        stats
    };
    stats.calculate_summaries();

    let mut out = open_output(args.output.as_deref(), options.nproc_out)?;
    if options.yaml {
        write_yaml(&mut out, &stats, options.include_chromsizes)?;
    } else {
        write_tsv(&mut out, &stats, options.include_chromsizes)?;
    }
    out.flush()?;
    Ok(())
}

fn reject_unsupported_stats_options(
    args: &StatsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.with_chromsizes && args.no_chromsizes {
        return Err("pairtools stats cannot use --with-chromsizes and --no-chromsizes together".into());
    }
    if args.yaml && args.no_yaml {
        return Err("pairtools stats cannot use --yaml and --no-yaml together".into());
    }
    if args.merge && args.yaml {
        return Err("not implemented: pairtools stats --merge --yaml".into());
    }
    if args.bytile_dups {
        return Err("not implemented: pairtools stats --bytile-dups".into());
    }
    if args.output_bytile_stats.is_some() {
        return Err("not implemented: pairtools stats --output-bytile-stats".into());
    }
    if !args.filter.is_empty() {
        return Err("not implemented: pairtools stats --filter".into());
    }
    if args.engine.is_some() {
        return Err("not implemented: pairtools stats --engine".into());
    }
    if args.chrom_subset.is_some() {
        return Err("not implemented: pairtools stats --chrom-subset".into());
    }
    if args.startup_code.is_some() {
        return Err("not implemented: pairtools stats --startup-code".into());
    }
    if !args.type_cast.is_empty() {
        return Err("not implemented: pairtools stats --type-cast".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools stats --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools stats --cmd-out".into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct StatsOptions {
    n_dist_bins_decade: usize,
    include_chromsizes: bool,
    yaml: bool,
    nproc_in: usize,
    nproc_out: usize,
}

impl StatsOptions {
    fn from_args(args: &StatsArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let n_dist_bins_decade = args.n_dist_bins_decade.unwrap_or(8);
        if n_dist_bins_decade == 0 {
            return Err("pairtools stats --n-dist-bins-decade must be greater than zero".into());
        }
        let nproc_in = args.nproc_in.unwrap_or(3);
        let nproc_out = args.nproc_out.unwrap_or(8);
        if nproc_in == 0 {
            return Err("pairtools stats --nproc-in must be greater than zero".into());
        }
        if nproc_out == 0 {
            return Err("pairtools stats --nproc-out must be greater than zero".into());
        }
        Ok(Self {
            n_dist_bins_decade,
            include_chromsizes: !args.no_chromsizes,
            yaml: args.yaml && !args.no_yaml,
            nproc_in,
            nproc_out,
        })
    }
}

fn stats_inputs(inputs: &[PathBuf]) -> Vec<Option<&Path>> {
    if inputs.is_empty() {
        vec![None]
    } else {
        inputs.iter().map(|path| Some(path.as_path())).collect()
    }
}

#[derive(Clone)]
struct PairsStats {
    dist_bins: Vec<u64>,
    total: u64,
    total_unmapped: u64,
    total_single_sided_mapped: u64,
    total_mapped: u64,
    total_dups: u64,
    total_nodups: u64,
    cis: u64,
    trans: u64,
    cis_1kb: u64,
    cis_2kb: u64,
    cis_4kb: u64,
    cis_10kb: u64,
    cis_20kb: u64,
    cis_40kb: u64,
    frac_cis: f64,
    frac_cis_1kb: f64,
    frac_cis_2kb: f64,
    frac_cis_4kb: f64,
    frac_cis_10kb: f64,
    frac_cis_20kb: f64,
    frac_cis_40kb: f64,
    frac_dups: f64,
    complexity_naive: f64,
    convergence: ConvergenceStats,
    pair_type_counts: OrderedCounts,
    chrom_freq_counts: OrderedCounts,
    dist_freq: HashMap<&'static str, Vec<u64>>,
    chromsizes: OrderedCounts,
}

impl PairsStats {
    fn new(n_dist_bins_decade: usize) -> Self {
        let dist_bins = make_dist_bins(n_dist_bins_decade);
        let mut dist_freq = HashMap::new();
        for strand in STRANDS {
            dist_freq.insert(strand, vec![0; dist_bins.len()]);
        }
        Self {
            dist_bins,
            total: 0,
            total_unmapped: 0,
            total_single_sided_mapped: 0,
            total_mapped: 0,
            total_dups: 0,
            total_nodups: 0,
            cis: 0,
            trans: 0,
            cis_1kb: 0,
            cis_2kb: 0,
            cis_4kb: 0,
            cis_10kb: 0,
            cis_20kb: 0,
            cis_40kb: 0,
            frac_cis: 0.0,
            frac_cis_1kb: 0.0,
            frac_cis_2kb: 0.0,
            frac_cis_4kb: 0.0,
            frac_cis_10kb: 0.0,
            frac_cis_20kb: 0.0,
            frac_cis_40kb: 0.0,
            frac_dups: 0.0,
            complexity_naive: 0.0,
            convergence: ConvergenceStats::default(),
            pair_type_counts: OrderedCounts::default(),
            chrom_freq_counts: OrderedCounts::default(),
            dist_freq,
            chromsizes: OrderedCounts::default(),
        }
    }

    fn add_assign(&mut self, other: &PairsStats) -> Result<(), Box<dyn std::error::Error>> {
        if self.dist_bins != other.dist_bins {
            return Err("cannot merge stats with different distance bins".into());
        }
        self.total += other.total;
        self.total_unmapped += other.total_unmapped;
        self.total_single_sided_mapped += other.total_single_sided_mapped;
        self.total_mapped += other.total_mapped;
        self.total_dups += other.total_dups;
        self.total_nodups += other.total_nodups;
        self.cis += other.cis;
        self.trans += other.trans;
        self.cis_1kb += other.cis_1kb;
        self.cis_2kb += other.cis_2kb;
        self.cis_4kb += other.cis_4kb;
        self.cis_10kb += other.cis_10kb;
        self.cis_20kb += other.cis_20kb;
        self.cis_40kb += other.cis_40kb;
        self.pair_type_counts.add_all(&other.pair_type_counts);
        self.chrom_freq_counts.add_all(&other.chrom_freq_counts);
        for strand in STRANDS {
            let target = self.dist_freq.get_mut(strand).expect("all strands initialized");
            let source = other.dist_freq.get(strand).expect("all strands initialized");
            for (t, s) in target.iter_mut().zip(source) {
                *t += *s;
            }
        }
        if self.chromsizes.counts.is_empty() {
            self.chromsizes = other.chromsizes.clone();
        } else if !other.chromsizes.counts.is_empty() && self.chromsizes.counts != other.chromsizes.counts {
            return Err("can't merge stats with different chromsizes".into());
        }
        Ok(())
    }

    fn calculate_summaries(&mut self) {
        self.frac_cis = ratio(self.cis, self.total_nodups);
        self.frac_cis_1kb = ratio(self.cis_1kb, self.total_nodups);
        self.frac_cis_2kb = ratio(self.cis_2kb, self.total_nodups);
        self.frac_cis_4kb = ratio(self.cis_4kb, self.total_nodups);
        self.frac_cis_10kb = ratio(self.cis_10kb, self.total_nodups);
        self.frac_cis_20kb = ratio(self.cis_20kb, self.total_nodups);
        self.frac_cis_40kb = ratio(self.cis_40kb, self.total_nodups);
        self.frac_dups = ratio(self.total_dups, self.total_mapped);
        self.complexity_naive = estimate_library_complexity(self.total_mapped, self.total_dups);
        self.convergence = calculate_convergence(self);
    }
}

#[derive(Clone, Default)]
struct OrderedCounts {
    order: Vec<String>,
    counts: HashMap<String, u64>,
}

impl OrderedCounts {
    fn add(&mut self, key: String, value: u64) {
        if !self.counts.contains_key(&key) {
            self.order.push(key.clone());
        }
        *self.counts.entry(key).or_insert(0) += value;
    }

    fn set(&mut self, key: String, value: u64) {
        if !self.counts.contains_key(&key) {
            self.order.push(key.clone());
        }
        self.counts.insert(key, value);
    }

    fn add_all(&mut self, other: &OrderedCounts) {
        for (key, value) in other.iter() {
            self.add(key.to_string(), value);
        }
    }

    fn iter(&self) -> impl Iterator<Item = (&str, u64)> {
        self.order
            .iter()
            .map(|key| (key.as_str(), self.counts.get(key).copied().unwrap_or(0)))
    }
}

#[derive(Clone, Default)]
struct ConvergenceStats {
    convergence_dist: String,
    strands_w_max_convergence_dist: String,
    below_by_strand: HashMap<&'static str, u64>,
    below_all: u64,
    above_all: u64,
}

#[derive(Clone)]
struct Columns {
    chrom1: usize,
    chrom2: usize,
    pos1: usize,
    pos2: usize,
    strand1: usize,
    strand2: usize,
    pair_type: usize,
}

fn read_pairs_input(
    path: Option<&Path>,
    stats: &mut PairsStats,
    options: &StatsOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = open_input(path, options.nproc_in)?;
    let (headers, first_body_line) = read_header(reader.as_mut())?;
    let columns = Columns::from_headers(&headers)?;
    add_chromsizes_from_headers(&headers, stats);

    if let Some(line) = first_body_line {
        process_record(&line, &columns, stats)?;
    }

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = trim_line_end(&line);
        if trimmed.is_empty() {
            continue;
        }
        process_record(trimmed, &columns, stats)?;
    }
    Ok(())
}

impl Columns {
    fn from_headers(headers: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let columns_line = headers
            .iter()
            .find(|line| line.starts_with("#columns:"))
            .ok_or("Input .pairs/.pairsam header is missing #columns")?;
        let names: Vec<&str> = columns_line
            .split_once(':')
            .map(|(_, rest)| rest)
            .unwrap_or("")
            .split_whitespace()
            .collect();
        let index = |name: &str| -> Result<usize, Box<dyn std::error::Error>> {
            names
                .iter()
                .position(|column| *column == name)
                .ok_or_else(|| format!("Input .pairs/.pairsam header is missing {name} column").into())
        };
        Ok(Self {
            chrom1: index("chrom1")?,
            chrom2: index("chrom2")?,
            pos1: index("pos1")?,
            pos2: index("pos2")?,
            strand1: index("strand1")?,
            strand2: index("strand2")?,
            pair_type: index("pair_type")?,
        })
    }
}

fn add_chromsizes_from_headers(headers: &[String], stats: &mut PairsStats) {
    for header in headers {
        let Some(rest) = header.strip_prefix("#chromsize:") else {
            continue;
        };
        let mut fields = rest.split_whitespace();
        let Some(chrom) = fields.next() else {
            continue;
        };
        let Some(size) = fields.next() else {
            continue;
        };
        let Ok(size) = size.parse::<u64>() else {
            continue;
        };
        if !stats.chromsizes.counts.contains_key(chrom) {
            stats.chromsizes.add(chrom.to_string(), size);
        }
    }
}

fn process_record(
    line: &str,
    columns: &Columns,
    stats: &mut PairsStats,
) -> Result<(), Box<dyn std::error::Error>> {
    let fields: Vec<&str> = line.split('\t').collect();
    let chrom1 = text_field(&fields, columns.chrom1);
    let chrom2 = text_field(&fields, columns.chrom2);
    let pos1 = int_field(&fields, columns.pos1)?;
    let pos2 = int_field(&fields, columns.pos2)?;
    let strand1 = text_field(&fields, columns.strand1);
    let strand2 = text_field(&fields, columns.strand2);
    let pair_type = text_field(&fields, columns.pair_type);

    stats.total += 1;
    stats.pair_type_counts.add(pair_type.to_string(), 1);

    let side1_unmapped = chrom1 == UNMAPPED_CHROM;
    let side2_unmapped = chrom2 == UNMAPPED_CHROM;
    if side1_unmapped && side2_unmapped {
        stats.total_unmapped += 1;
        return Ok(());
    }
    if side1_unmapped || side2_unmapped {
        stats.total_single_sided_mapped += 1;
        return Ok(());
    }

    stats.total_mapped += 1;
    if pair_type == "DD" {
        stats.total_dups += 1;
        return Ok(());
    }

    stats.total_nodups += 1;
    stats
        .chrom_freq_counts
        .add(format!("{chrom1}/{chrom2}"), 1);
    if chrom1 == chrom2 {
        stats.cis += 1;
        let dist = pos1.abs_diff(pos2);
        let strand_key = match (strand1, strand2) {
            ("+", "-") => "+-",
            ("-", "+") => "-+",
            ("-", "-") => "--",
            ("+", "+") => "++",
            _ => "++",
        };
        let bin_idx = dist_bin_idx(&stats.dist_bins, dist);
        if let Some(freqs) = stats.dist_freq.get_mut(strand_key) {
            freqs[bin_idx] += 1;
        }
        if dist >= 1_000 {
            stats.cis_1kb += 1;
        }
        if dist >= 2_000 {
            stats.cis_2kb += 1;
        }
        if dist >= 4_000 {
            stats.cis_4kb += 1;
        }
        if dist >= 10_000 {
            stats.cis_10kb += 1;
        }
        if dist >= 20_000 {
            stats.cis_20kb += 1;
        }
        if dist >= 40_000 {
            stats.cis_40kb += 1;
        }
    } else {
        stats.trans += 1;
    }
    Ok(())
}

fn merge_stats_inputs(
    inputs: &[PathBuf],
    options: &StatsOptions,
) -> Result<PairsStats, Box<dyn std::error::Error>> {
    if inputs.is_empty() {
        return Err("pairs-rs stats --merge requires at least one stats file".into());
    }
    let mut merged = PairsStats::new(options.n_dist_bins_decade);
    let mut saw_any = false;
    for input in inputs {
        let parsed = read_stats_file(input.as_path(), options)?;
        if saw_any {
            merged.add_assign(&parsed)?;
        } else {
            merged = parsed;
            saw_any = true;
        }
    }
    Ok(merged)
}

fn read_stats_file(
    path: &Path,
    options: &StatsOptions,
) -> Result<PairsStats, Box<dyn std::error::Error>> {
    let mut stats = PairsStats::new(options.n_dist_bins_decade);
    let mut reader = open_input(Some(path), options.nproc_in)?;
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        let trimmed = trim_line_end(&line);
        if !trimmed.is_empty() {
            let Some((key, value)) = trimmed.split_once('\t') else {
                return Err(format!("invalid stats line: {trimmed}").into());
            };
            apply_stats_key(&mut stats, key, value)?;
        }
        line.clear();
    }
    Ok(stats)
}

fn apply_stats_key(
    stats: &mut PairsStats,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match key {
        "total" => stats.total = parse_u64(value)?,
        "total_unmapped" => stats.total_unmapped = parse_u64(value)?,
        "total_single_sided_mapped" => stats.total_single_sided_mapped = parse_u64(value)?,
        "total_mapped" => stats.total_mapped = parse_u64(value)?,
        "total_dups" => stats.total_dups = parse_u64(value)?,
        "total_nodups" => stats.total_nodups = parse_u64(value)?,
        "cis" => stats.cis = parse_u64(value)?,
        "trans" => stats.trans = parse_u64(value)?,
        "cis_1kb+" => stats.cis_1kb = parse_u64(value)?,
        "cis_2kb+" => stats.cis_2kb = parse_u64(value)?,
        "cis_4kb+" => stats.cis_4kb = parse_u64(value)?,
        "cis_10kb+" => stats.cis_10kb = parse_u64(value)?,
        "cis_20kb+" => stats.cis_20kb = parse_u64(value)?,
        "cis_40kb+" => stats.cis_40kb = parse_u64(value)?,
        _ if key.starts_with("pair_types/") => {
            stats.pair_type_counts.set(key["pair_types/".len()..].to_string(), parse_u64(value)?);
        }
        _ if key.starts_with("chrom_freq/") => {
            stats.chrom_freq_counts.set(key["chrom_freq/".len()..].to_string(), parse_u64(value)?);
        }
        _ if key.starts_with("chromsizes/") => {
            stats.chromsizes.set(key["chromsizes/".len()..].to_string(), parse_u64(value)?);
        }
        _ if key.starts_with("dist_freq/") => {
            let rest = &key["dist_freq/".len()..];
            let Some((bin_label, strand)) = rest.rsplit_once('/') else {
                return Err(format!("invalid dist_freq key: {key}").into());
            };
            let bin_left = dist_label_left(bin_label)?;
            let Some(idx) = stats.dist_bins.iter().position(|bin| *bin == bin_left) else {
                return Err(format!("stats dist_freq bin {bin_left} is not configured").into());
            };
            let strand = match strand {
                "+-" => "+-",
                "-+" => "-+",
                "--" => "--",
                "++" => "++",
                _ => return Err(format!("invalid dist_freq strand: {strand}").into()),
            };
            if let Some(freqs) = stats.dist_freq.get_mut(strand) {
                freqs[idx] = parse_u64(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_u64(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .parse::<u64>()
        .map_err(|err| format!("failed to parse integer stats value {value:?}: {err}").into())
}

fn dist_label_left(label: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let left = label
        .strip_suffix('+')
        .or_else(|| label.split_once('-').map(|(left, _)| left))
        .unwrap_or(label);
    parse_u64(left)
}

fn make_dist_bins(n_dist_bins_decade: usize) -> Vec<u64> {
    let step = 1.0 / n_dist_bins_decade as f64;
    let mut bins = vec![0_u64];
    let mut value = 0.0;
    while value <= 9.000_001 {
        let rounded = 10_f64.powf(value).round() as u64;
        if bins.last().copied() != Some(rounded) {
            bins.push(rounded);
        }
        value += step;
    }
    bins
}

fn dist_bin_idx(bins: &[u64], dist: u64) -> usize {
    let idx = bins.partition_point(|bin| *bin <= dist);
    idx.saturating_sub(1)
}

fn calculate_convergence(stats: &PairsStats) -> ConvergenceStats {
    let len = stats.dist_bins.len();
    let mut idx_maxs: HashMap<&'static str, usize> = CONVERGENCE_STRANDS
        .into_iter()
        .map(|strand| (strand, 0))
        .collect();
    for idx in 0..len {
        let mut total = 0.0;
        for strand in CONVERGENCE_STRANDS {
            total += stats.dist_freq[strand][idx] as f64;
        }
        let avg = total / CONVERGENCE_STRANDS.len() as f64;
        if avg == 0.0 {
            continue;
        }
        for strand in CONVERGENCE_STRANDS {
            let value = stats.dist_freq[strand][idx] as f64;
            let rel_dev = ((value - avg).abs()) / avg;
            if rel_dev > CONVERGENCE_THRESHOLD {
                idx_maxs.insert(strand, idx);
            }
        }
    }

    let mut convergence_bin_idx = 0_usize;
    let mut convergence_strands = "??";
    let mut convergence_dist = "0".to_string();
    for strand in CONVERGENCE_STRANDS {
        let idx = idx_maxs[strand];
        if idx > convergence_bin_idx {
            convergence_bin_idx = idx;
            convergence_strands = strand;
            convergence_dist = if idx < len {
                stats
                    .dist_bins
                    .get(convergence_bin_idx + 1)
                    .copied()
                    .unwrap_or(u64::MAX)
                    .to_string()
            } else {
                u64::MAX.to_string()
            };
        }
    }

    let mut below_by_strand = HashMap::new();
    let mut below_all = 0;
    let mut above_all = 0;
    for strand in CONVERGENCE_STRANDS {
        let freqs = &stats.dist_freq[strand];
        let below: u64 = freqs[..=convergence_bin_idx.min(freqs.len() - 1)].iter().sum();
        let above: u64 = if convergence_bin_idx + 1 < freqs.len() {
            freqs[convergence_bin_idx + 1..].iter().sum()
        } else {
            0
        };
        below_by_strand.insert(strand, below);
        below_all += below;
        above_all += above;
    }

    ConvergenceStats {
        convergence_dist,
        strands_w_max_convergence_dist: convergence_strands.to_string(),
        below_by_strand,
        below_all,
        above_all,
    }
}

fn write_tsv(
    out: &mut Box<dyn Write>,
    stats: &PairsStats,
    include_chromsizes: bool,
) -> io::Result<()> {
    writeln!(out, "total\t{}", stats.total)?;
    writeln!(out, "total_unmapped\t{}", stats.total_unmapped)?;
    writeln!(
        out,
        "total_single_sided_mapped\t{}",
        stats.total_single_sided_mapped
    )?;
    writeln!(out, "total_mapped\t{}", stats.total_mapped)?;
    writeln!(out, "total_dups\t{}", stats.total_dups)?;
    writeln!(out, "total_nodups\t{}", stats.total_nodups)?;
    writeln!(out, "cis\t{}", stats.cis)?;
    writeln!(out, "trans\t{}", stats.trans)?;
    for (pair_type, count) in stats.pair_type_counts.iter() {
        writeln!(out, "pair_types/{pair_type}\t{count}")?;
    }
    writeln!(out, "cis_1kb+\t{}", stats.cis_1kb)?;
    writeln!(out, "cis_2kb+\t{}", stats.cis_2kb)?;
    writeln!(out, "cis_4kb+\t{}", stats.cis_4kb)?;
    writeln!(out, "cis_10kb+\t{}", stats.cis_10kb)?;
    writeln!(out, "cis_20kb+\t{}", stats.cis_20kb)?;
    writeln!(out, "cis_40kb+\t{}", stats.cis_40kb)?;
    write_summary_tsv(out, stats)?;
    for (chrom_pair, count) in stats.chrom_freq_counts.iter() {
        writeln!(out, "chrom_freq/{chrom_pair}\t{count}")?;
    }
    write_dist_freq_tsv(out, stats)?;
    if include_chromsizes {
        for (chrom, size) in stats.chromsizes.iter() {
            writeln!(out, "chromsizes/{chrom}\t{size}")?;
        }
    }
    Ok(())
}

fn write_summary_tsv(out: &mut Box<dyn Write>, stats: &PairsStats) -> io::Result<()> {
    writeln!(out, "summary/frac_cis\t{}", format_float(stats.frac_cis))?;
    writeln!(out, "summary/frac_cis_1kb+\t{}", format_float(stats.frac_cis_1kb))?;
    writeln!(out, "summary/frac_cis_2kb+\t{}", format_float(stats.frac_cis_2kb))?;
    writeln!(out, "summary/frac_cis_4kb+\t{}", format_float(stats.frac_cis_4kb))?;
    writeln!(out, "summary/frac_cis_10kb+\t{}", format_float(stats.frac_cis_10kb))?;
    writeln!(out, "summary/frac_cis_20kb+\t{}", format_float(stats.frac_cis_20kb))?;
    writeln!(out, "summary/frac_cis_40kb+\t{}", format_float(stats.frac_cis_40kb))?;
    writeln!(out, "summary/frac_dups\t{}", format_float(stats.frac_dups))?;
    writeln!(
        out,
        "summary/complexity_naive\t{}",
        format_float(stats.complexity_naive)
    )?;
    let conv = &stats.convergence;
    writeln!(
        out,
        "summary/dist_freq_convergence/convergence_dist\t{}",
        conv.convergence_dist
    )?;
    writeln!(
        out,
        "summary/dist_freq_convergence/strands_w_max_convergence_dist\t{}",
        conv.strands_w_max_convergence_dist
    )?;
    writeln!(
        out,
        "summary/dist_freq_convergence/convergence_rel_diff_threshold\t{}",
        format_float(CONVERGENCE_THRESHOLD)
    )?;
    for strand in CONVERGENCE_STRANDS {
        writeln!(
            out,
            "summary/dist_freq_convergence/n_cis_pairs_below_convergence_dist/{strand}\t{}",
            conv.below_by_strand.get(strand).copied().unwrap_or(0)
        )?;
    }
    writeln!(
        out,
        "summary/dist_freq_convergence/n_cis_pairs_below_convergence_dist_all_strands\t{}",
        conv.below_all
    )?;
    writeln!(
        out,
        "summary/dist_freq_convergence/n_cis_pairs_above_convergence_dist_all_strands\t{}",
        conv.above_all
    )?;
    write_convergence_fracs_tsv(out, stats, "cis", stats.cis)?;
    write_convergence_fracs_tsv(out, stats, "total_mapped", stats.total_mapped)?;
    write_convergence_fracs_tsv(out, stats, "total_nodups", stats.total_nodups)?;
    Ok(())
}

fn write_convergence_fracs_tsv(
    out: &mut Box<dyn Write>,
    stats: &PairsStats,
    label: &str,
    denom: u64,
) -> io::Result<()> {
    for strand in CONVERGENCE_STRANDS {
        let below = stats.convergence.below_by_strand.get(strand).copied().unwrap_or(0);
        writeln!(
            out,
            "summary/dist_freq_convergence/frac_{label}_in_cis_below_convergence_dist/{strand}\t{}",
            format_float(ratio(below, denom))
        )?;
    }
    writeln!(
        out,
        "summary/dist_freq_convergence/frac_{label}_in_cis_below_convergence_dist_all_strands\t{}",
        format_float(ratio(stats.convergence.below_all, denom))
    )?;
    writeln!(
        out,
        "summary/dist_freq_convergence/frac_{label}_in_cis_above_convergence_dist_all_strands\t{}",
        format_float(ratio(stats.convergence.above_all, denom))
    )?;
    Ok(())
}

fn write_dist_freq_tsv(out: &mut Box<dyn Write>, stats: &PairsStats) -> io::Result<()> {
    for idx in 0..stats.dist_bins.len() {
        for strand in STRANDS {
            let label = dist_label(&stats.dist_bins, idx);
            writeln!(
                out,
                "dist_freq/{label}/{strand}\t{}",
                stats.dist_freq[strand][idx]
            )?;
        }
    }
    Ok(())
}

fn write_yaml(
    out: &mut Box<dyn Write>,
    stats: &PairsStats,
    include_chromsizes: bool,
) -> io::Result<()> {
    writeln!(out, "no_filter:")?;
    write_yaml_u64(out, 1, "total", stats.total)?;
    write_yaml_u64(out, 1, "total_unmapped", stats.total_unmapped)?;
    write_yaml_u64(
        out,
        1,
        "total_single_sided_mapped",
        stats.total_single_sided_mapped,
    )?;
    write_yaml_u64(out, 1, "total_mapped", stats.total_mapped)?;
    write_yaml_u64(out, 1, "total_dups", stats.total_dups)?;
    write_yaml_u64(out, 1, "total_nodups", stats.total_nodups)?;
    write_yaml_u64(out, 1, "cis", stats.cis)?;
    write_yaml_u64(out, 1, "trans", stats.trans)?;
    if !stats.pair_type_counts.order.is_empty() {
        writeln!(out, "  pair_types:")?;
        for (pair_type, count) in stats.pair_type_counts.iter() {
            writeln!(out, "    {pair_type}: {count}")?;
        }
    }
    write_yaml_u64(out, 1, "cis_1kb+", stats.cis_1kb)?;
    write_yaml_u64(out, 1, "cis_2kb+", stats.cis_2kb)?;
    write_yaml_u64(out, 1, "cis_4kb+", stats.cis_4kb)?;
    write_yaml_u64(out, 1, "cis_10kb+", stats.cis_10kb)?;
    write_yaml_u64(out, 1, "cis_20kb+", stats.cis_20kb)?;
    write_yaml_u64(out, 1, "cis_40kb+", stats.cis_40kb)?;
    writeln!(out, "  summary:")?;
    writeln!(out, "    frac_cis: {}", format_float(stats.frac_cis))?;
    writeln!(out, "    frac_cis_1kb+: {}", format_float(stats.frac_cis_1kb))?;
    writeln!(out, "    frac_cis_2kb+: {}", format_float(stats.frac_cis_2kb))?;
    writeln!(out, "    frac_cis_4kb+: {}", format_float(stats.frac_cis_4kb))?;
    writeln!(out, "    frac_cis_10kb+: {}", format_float(stats.frac_cis_10kb))?;
    writeln!(out, "    frac_cis_20kb+: {}", format_float(stats.frac_cis_20kb))?;
    writeln!(out, "    frac_cis_40kb+: {}", format_float(stats.frac_cis_40kb))?;
    writeln!(out, "    frac_dups: {}", format_float(stats.frac_dups))?;
    writeln!(
        out,
        "    complexity_naive: {}",
        format_float(stats.complexity_naive)
    )?;
    write_yaml_convergence(out, stats)?;
    if !stats.chrom_freq_counts.order.is_empty() {
        writeln!(out, "  chrom_freq:")?;
        for (chrom_pair, count) in stats.chrom_freq_counts.iter() {
            writeln!(out, "    {chrom_pair}: {count}")?;
        }
    }
    writeln!(out, "  dist_freq:")?;
    for strand in STRANDS {
        writeln!(out, "    {strand}:")?;
        for (idx, bin) in stats.dist_bins.iter().enumerate() {
            writeln!(out, "      {bin}: {}", stats.dist_freq[strand][idx])?;
        }
    }
    if include_chromsizes && !stats.chromsizes.order.is_empty() {
        writeln!(out, "  chromsizes:")?;
        for (chrom, size) in stats.chromsizes.iter() {
            writeln!(out, "    {chrom}: {size}")?;
        }
    }
    Ok(())
}

fn write_yaml_u64(out: &mut Box<dyn Write>, indent: usize, key: &str, value: u64) -> io::Result<()> {
    if value > 0 {
        writeln!(out, "{}{key}: {value}", "  ".repeat(indent))?;
    }
    Ok(())
}

fn write_yaml_convergence(out: &mut Box<dyn Write>, stats: &PairsStats) -> io::Result<()> {
    let conv = &stats.convergence;
    writeln!(out, "    dist_freq_convergence:")?;
    writeln!(out, "      convergence_dist: {}", conv.convergence_dist)?;
    writeln!(
        out,
        "      strands_w_max_convergence_dist: {}",
        conv.strands_w_max_convergence_dist
    )?;
    writeln!(
        out,
        "      convergence_rel_diff_threshold: {}",
        format_float(CONVERGENCE_THRESHOLD)
    )?;
    writeln!(out, "      n_cis_pairs_below_convergence_dist:")?;
    for strand in CONVERGENCE_STRANDS {
        writeln!(
            out,
            "        {strand}: {}",
            conv.below_by_strand.get(strand).copied().unwrap_or(0)
        )?;
    }
    writeln!(
        out,
        "      n_cis_pairs_below_convergence_dist_all_strands: {}",
        conv.below_all
    )?;
    writeln!(
        out,
        "      n_cis_pairs_above_convergence_dist_all_strands: {}",
        conv.above_all
    )?;
    write_yaml_convergence_frac(out, stats, "cis", stats.cis)?;
    write_yaml_convergence_frac(out, stats, "total_mapped", stats.total_mapped)?;
    write_yaml_convergence_frac(out, stats, "total_nodups", stats.total_nodups)?;
    Ok(())
}

fn write_yaml_convergence_frac(
    out: &mut Box<dyn Write>,
    stats: &PairsStats,
    label: &str,
    denom: u64,
) -> io::Result<()> {
    writeln!(out, "      frac_{label}_in_cis_below_convergence_dist:")?;
    for strand in CONVERGENCE_STRANDS {
        let below = stats.convergence.below_by_strand.get(strand).copied().unwrap_or(0);
        writeln!(out, "        {strand}: {}", format_float(ratio(below, denom)))?;
    }
    writeln!(
        out,
        "      frac_{label}_in_cis_below_convergence_dist_all_strands: {}",
        format_float(ratio(stats.convergence.below_all, denom))
    )?;
    writeln!(
        out,
        "      frac_{label}_in_cis_above_convergence_dist_all_strands: {}",
        format_float(ratio(stats.convergence.above_all, denom))
    )?;
    Ok(())
}

fn text_field<'a>(fields: &[&'a str], idx: usize) -> &'a str {
    fields.get(idx).copied().unwrap_or("")
}

fn int_field(fields: &[&str], idx: usize) -> Result<u64, Box<dyn std::error::Error>> {
    fields
        .get(idx)
        .ok_or_else(|| format!("Input .pairs/.pairsam record is missing column {idx}"))?
        .parse::<u64>()
        .map_err(|err| format!("failed to parse integer column {idx}: {err}").into())
}

fn dist_label(bins: &[u64], idx: usize) -> String {
    if idx + 1 < bins.len() {
        format!("{}-{}", bins[idx], bins[idx + 1])
    } else {
        format!("{}+", bins[idx])
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn format_float(value: f64) -> String {
    if value == 0.0 {
        return "0.0".to_string();
    }
    let mut text = value.to_string();
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn estimate_library_complexity(nseq: u64, ndup: u64) -> f64 {
    if nseq == 0 {
        return 0.0;
    }
    let u = (nseq.saturating_sub(ndup)) as f64 / nseq as f64;
    if u == 0.0 {
        return 0.0;
    }
    let z = -(-1.0 / u).exp() / u;
    let w = lambert_w0_negative(z);
    let seq_to_complexity = w + 1.0 / u;
    nseq as f64 / seq_to_complexity
}

fn lambert_w0_negative(z: f64) -> f64 {
    let mut lo: f64 = -1.0;
    let mut hi: f64 = 0.0;
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        let value = mid * mid.exp();
        if value < z {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

fn open_input(
    path: Option<&Path>,
    nproc: usize,
) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path, nproc)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed stats input .lz4".into())
        }
        Some(path) => Ok(Box::new(BufReader::new(File::open(path)?))),
        None => Ok(Box::new(BufReader::new(io::stdin()))),
    }
}

fn open_output(
    path: Option<&Path>,
    nproc: usize,
) -> Result<Box<dyn Write>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufWriter::new(io::stdout()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufWriter::new(BgzfWriter::create(path, nproc)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed stats output .lz4".into())
        }
        Some(path) => Ok(Box::new(BufWriter::new(File::create(path)?))),
        None => Ok(Box::new(BufWriter::new(io::stdout()))),
    }
}

fn read_header(
    reader: &mut dyn BufRead,
) -> Result<(Vec<String>, Option<String>), Box<dyn std::error::Error>> {
    let mut headers = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok((headers, None));
        }
        let trimmed = trim_line_end(&line).to_string();
        if trimmed.starts_with('#') {
            headers.push(trimmed);
        } else {
            return Ok((headers, Some(trimmed)));
        }
    }
}

fn has_suffix(path: &Path, suffix: &str) -> bool {
    path.to_string_lossy().ends_with(suffix)
}

fn trim_line_end(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

struct BgzfReader {
    handle: *mut htslib::BGZF,
}

impl BgzfReader {
    fn open(path: &Path, nproc: usize) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "input path contains NUL byte")
        })?;
        let mode = CString::new("r").expect("static BGZF mode has no NUL bytes");
        let handle = unsafe { htslib::bgzf_open(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        enable_bgzf_threads(handle, nproc, "decompression")?;
        Ok(Self { handle })
    }

    fn close(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        let status = unsafe { htslib::bgzf_close(self.handle) };
        self.handle = std::ptr::null_mut();
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to close BGZF stream, HTSlib status {status}"),
            ))
        }
    }
}

impl Read for BgzfReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let read = unsafe {
            htslib::bgzf_read(self.handle, buf.as_mut_ptr() as *mut c_void, buf.len())
        };
        if read < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to read BGZF stream",
            ))
        } else {
            Ok(read as usize)
        }
    }
}

impl Drop for BgzfReader {
    fn drop(&mut self) {
        drop(self.close());
    }
}

struct BgzfWriter {
    handle: *mut htslib::BGZF,
}

impl BgzfWriter {
    fn create(path: &Path, nproc: usize) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "output path contains NUL byte")
        })?;
        let mode = CString::new("w").expect("static BGZF mode has no NUL bytes");
        let handle = unsafe { htslib::bgzf_open(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        enable_bgzf_threads(handle, nproc, "compression")?;
        Ok(Self { handle })
    }

    fn close(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        let status = unsafe { htslib::bgzf_close(self.handle) };
        self.handle = std::ptr::null_mut();
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to close BGZF stream, HTSlib status {status}"),
            ))
        }
    }
}

impl Write for BgzfWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let written =
            unsafe { htslib::bgzf_write(self.handle, buf.as_ptr() as *const c_void, buf.len()) };
        if written < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write BGZF stream",
            ))
        } else {
            Ok(written as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let status = unsafe { htslib::bgzf_flush(self.handle) };
        if status == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to flush BGZF stream, HTSlib status {status}"),
            ))
        }
    }
}

impl Drop for BgzfWriter {
    fn drop(&mut self) {
        drop(self.close());
    }
}

fn enable_bgzf_threads(
    handle: *mut htslib::BGZF,
    nproc: usize,
    label: &str,
) -> io::Result<()> {
    if nproc <= 1 {
        return Ok(());
    }
    let status = unsafe { htslib::bgzf_mt(handle, nproc as c_int, 256) };
    if status == 0 {
        Ok(())
    } else {
        unsafe {
            htslib::bgzf_close(handle);
        }
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("failed to enable BGZF {label} threads, HTSlib status {status}"),
        ))
    }
}
