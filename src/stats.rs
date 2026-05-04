use crate::cli::StatsArgs;
use rust_htslib::htslib;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};

const UNMAPPED_CHROM: &str = "!";

pub fn cmd_stats(args: StatsArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_stats_options(&args)?;
    let mut stats = PairsStats::default();
    let inputs = stats_inputs(&args.inputs);

    for input in inputs {
        read_stats_input(input.as_deref(), &mut stats)?;
    }

    let mut out = open_output(args.output.as_deref())?;
    write_stats(&mut out, &stats, args.with_chromsizes)?;
    out.flush()?;
    Ok(())
}

fn reject_unsupported_stats_options(
    args: &StatsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.merge {
        return Err("not implemented: pairtools stats --merge".into());
    }
    if args.n_dist_bins_decade.is_some() {
        return Err("not implemented: pairtools stats --n-dist-bins-decade".into());
    }
    if args.with_chromsizes && args.no_chromsizes {
        return Err("pairtools stats cannot use --with-chromsizes and --no-chromsizes together".into());
    }
    if args.yaml {
        return Err("not implemented: pairtools stats --yaml".into());
    }
    if args.no_yaml {
        return Err("not implemented: pairtools stats --no-yaml".into());
    }
    if args.bytile_dups {
        return Err("not implemented: pairtools stats --bytile-dups".into());
    }
    if args.no_bytile_dups {
        return Err("not implemented: pairtools stats --no-bytile-dups".into());
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
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools stats --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools stats --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools stats --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools stats --cmd-out".into());
    }
    Ok(())
}

fn stats_inputs(inputs: &[PathBuf]) -> Vec<Option<&Path>> {
    if inputs.is_empty() {
        vec![None]
    } else {
        inputs.iter().map(|path| Some(path.as_path())).collect()
    }
}

#[derive(Default)]
struct PairsStats {
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
    pair_type_counts: OrderedCounts,
    chrom_freq_counts: OrderedCounts,
    chromsizes: OrderedCounts,
}

#[derive(Default)]
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

    fn iter(&self) -> impl Iterator<Item = (&str, u64)> {
        self.order
            .iter()
            .map(|key| (key.as_str(), self.counts.get(key).copied().unwrap_or(0)))
    }
}

#[derive(Clone)]
struct Columns {
    chrom1: usize,
    chrom2: usize,
    pos1: usize,
    pos2: usize,
    pair_type: usize,
}

fn read_stats_input(
    path: Option<&Path>,
    stats: &mut PairsStats,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = open_input(path)?;
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

fn write_stats(
    out: &mut Box<dyn Write>,
    stats: &PairsStats,
    with_chromsizes: bool,
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
    writeln!(out, "summary/frac_cis\t{}", frac(stats.cis, stats.total_nodups))?;
    writeln!(
        out,
        "summary/frac_cis_1kb+\t{}",
        frac(stats.cis_1kb, stats.total_nodups)
    )?;
    writeln!(
        out,
        "summary/frac_cis_2kb+\t{}",
        frac(stats.cis_2kb, stats.total_nodups)
    )?;
    writeln!(
        out,
        "summary/frac_cis_4kb+\t{}",
        frac(stats.cis_4kb, stats.total_nodups)
    )?;
    writeln!(
        out,
        "summary/frac_cis_10kb+\t{}",
        frac(stats.cis_10kb, stats.total_nodups)
    )?;
    writeln!(
        out,
        "summary/frac_cis_20kb+\t{}",
        frac(stats.cis_20kb, stats.total_nodups)
    )?;
    writeln!(
        out,
        "summary/frac_cis_40kb+\t{}",
        frac(stats.cis_40kb, stats.total_nodups)
    )?;
    writeln!(out, "summary/frac_dups\t{}", frac(stats.total_dups, stats.total_mapped))?;
    for (chrom_pair, count) in stats.chrom_freq_counts.iter() {
        writeln!(out, "chrom_freq/{chrom_pair}\t{count}")?;
    }
    if with_chromsizes {
        for (chrom, size) in stats.chromsizes.iter() {
            writeln!(out, "chromsizes/{chrom}\t{size}")?;
        }
    }
    Ok(())
}

fn frac(numerator: u64, denominator: u64) -> String {
    if denominator == 0 || numerator == 0 {
        "0.0".to_string()
    } else {
        (numerator as f64 / denominator as f64).to_string()
    }
}

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed stats input .lz4".into())
        }
        Some(path) => Ok(Box::new(BufReader::new(File::open(path)?))),
        None => Ok(Box::new(BufReader::new(io::stdin()))),
    }
}

fn open_output(path: Option<&Path>) -> Result<Box<dyn Write>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufWriter::new(io::stdout()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufWriter::new(BgzfWriter::create(path)?)))
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
    fn open(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "input path contains NUL byte")
        })?;
        let mode = CString::new("r").expect("static BGZF mode has no NUL bytes");
        let handle = unsafe { htslib::bgzf_open(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
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
    fn create(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "output path contains NUL byte")
        })?;
        let mode = CString::new("w").expect("static BGZF mode has no NUL bytes");
        let handle = unsafe { htslib::bgzf_open(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
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
