use crate::cli::DedupArgs;
use rust_htslib::htslib;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::c_void;
use std::path::Path;

const SAM_SEP: char = '\x19';
const INTER_SAM_SEP: &str = "\x19NEXT_SAM\x19";
const DUP_FLAG: u16 = 0x400;

pub fn cmd_dedup(args: DedupArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_dedup_options(&args)?;
    let method = DedupMethod::parse(&args.method)?;
    if args.max_mismatch < 0 {
        return Err("pairtools dedup --max-mismatch must be non-negative".into());
    }
    let sep = decode_sep(&args.sep)?;
    let send_header_to = HeaderDestination::parse(&args.send_header_to)?;
    let mark_dups = if args.no_mark_dups {
        false
    } else {
        true
    };

    let mut reader = open_input(args.input.as_deref())?;
    let (headers, first_body_line) = read_header(reader.as_mut())?;
    let columns = Columns::from_headers(&headers, &args, sep)?;
    let mut outputs = Outputs::open(&args, &headers, send_header_to)?;
    let mut stats = DedupStats::default();
    let mut parents: Vec<ParentRecord> = Vec::new();

    if let Some(line) = first_body_line {
        process_line(
            line,
            &columns,
            sep,
            &args.unmapped_chrom,
            method,
            args.max_mismatch,
            mark_dups,
            &mut parents,
            &mut outputs,
            &mut stats,
        )?;
    }

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = trim_line_end(&line).to_string();
        if trimmed.is_empty() {
            continue;
        }
        process_line(
            trimmed,
            &columns,
            sep,
            &args.unmapped_chrom,
            method,
            args.max_mismatch,
            mark_dups,
            &mut parents,
            &mut outputs,
            &mut stats,
        )?;
    }
    outputs.flush()?;
    if let Some(path) = args.output_stats {
        write_stats(&path, &stats)?;
    }
    Ok(())
}

fn reject_unsupported_dedup_options(
    args: &DedupArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.output_bytile_stats.is_some() {
        return Err("not implemented: pairtools dedup --output-bytile-stats".into());
    }
    if args.backend.is_some() {
        return Err("not implemented: pairtools dedup --backend".into());
    }
    if args.chunksize.is_some() {
        return Err("not implemented: pairtools dedup --chunksize".into());
    }
    if args.carryover.is_some() {
        return Err("not implemented: pairtools dedup --carryover".into());
    }
    if args.n_proc.is_some() {
        return Err("not implemented: pairtools dedup --n-proc".into());
    }
    if args.keep_parent_id {
        return Err("not implemented: pairtools dedup --keep-parent-id".into());
    }
    if !args.extra_col_pair.is_empty() {
        return Err("not implemented: pairtools dedup --extra-col-pair".into());
    }
    if args.s1.is_some() {
        return Err("not implemented: pairtools dedup --s1".into());
    }
    if args.s2.is_some() {
        return Err("not implemented: pairtools dedup --s2".into());
    }
    if args.yaml {
        return Err("not implemented: pairtools dedup --yaml".into());
    }
    if args.no_yaml {
        return Err("not implemented: pairtools dedup --no-yaml".into());
    }
    if !args.filter.is_empty() {
        return Err("not implemented: pairtools dedup --filter".into());
    }
    if args.engine.is_some() {
        return Err("not implemented: pairtools dedup --engine".into());
    }
    if args.chrom_subset.is_some() {
        return Err("not implemented: pairtools dedup --chrom-subset".into());
    }
    if args.startup_code.is_some() {
        return Err("not implemented: pairtools dedup --startup-code".into());
    }
    if !args.type_cast.is_empty() {
        return Err("not implemented: pairtools dedup --type-cast".into());
    }
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools dedup --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools dedup --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools dedup --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools dedup --cmd-out".into());
    }
    if args.mark_dups && args.no_mark_dups {
        return Err("pairtools dedup cannot use --mark-dups and --no-mark-dups together".into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum DedupMethod {
    Max,
    Sum,
}

impl DedupMethod {
    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value {
            "max" => Ok(Self::Max),
            "sum" => Ok(Self::Sum),
            other => Err(format!("not implemented: pairtools dedup --method {other}").into()),
        }
    }

    fn is_duplicate(self, p1_delta: i64, p2_delta: i64, max_mismatch: i64) -> bool {
        match self {
            Self::Max => p1_delta.max(p2_delta) <= max_mismatch,
            Self::Sum => p1_delta + p2_delta <= max_mismatch,
        }
    }
}

#[derive(Clone, Copy)]
enum HeaderDestination {
    Dups,
    Dedup,
    Both,
    None,
}

impl HeaderDestination {
    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value {
            "dups" => Ok(Self::Dups),
            "dedup" => Ok(Self::Dedup),
            "both" => Ok(Self::Both),
            "none" => Ok(Self::None),
            other => Err(format!("not implemented: pairtools dedup --send-header-to {other}").into()),
        }
    }

    fn send_dedup(self) -> bool {
        matches!(self, Self::Dedup | Self::Both)
    }

    fn send_dups(self) -> bool {
        matches!(self, Self::Dups | Self::Both)
    }

    fn send_unmapped(self) -> bool {
        matches!(self, Self::Dups | Self::Both)
    }
}

struct Columns {
    chrom1: usize,
    chrom2: usize,
    pos1: usize,
    pos2: usize,
    pair_type: usize,
    sam1: Option<usize>,
    sam2: Option<usize>,
}

impl Columns {
    fn from_headers(
        headers: &[String],
        args: &DedupArgs,
        sep: char,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
        let resolve = |value: Option<&String>, default: &str| -> Result<usize, Box<dyn std::error::Error>> {
            let value = value.map(String::as_str).unwrap_or(default);
            if let Ok(idx) = value.parse::<usize>() {
                return Ok(idx);
            }
            names
                .iter()
                .position(|column| *column == value)
                .ok_or_else(|| format!("Input .pairs/.pairsam header is missing {value} column").into())
        };
        let optional = |name: &str| names.iter().position(|column| *column == name);
        if sep != '\t' {
            return Err("not implemented: pairtools dedup --sep for non-tab separators".into());
        }
        Ok(Self {
            chrom1: resolve(args.c1.as_ref(), "chrom1")?,
            chrom2: resolve(args.c2.as_ref(), "chrom2")?,
            pos1: resolve(args.p1.as_ref(), "pos1")?,
            pos2: resolve(args.p2.as_ref(), "pos2")?,
            pair_type: optional("pair_type")
                .ok_or("Input .pairs/.pairsam header is missing pair_type column")?,
            sam1: optional("sam1"),
            sam2: optional("sam2"),
        })
    }
}

#[derive(Clone)]
struct ParentRecord {
    chrom1: String,
    chrom2: String,
    pos1: i64,
    pos2: i64,
}

impl ParentRecord {
    fn from_fields(fields: &[String], columns: &Columns) -> Self {
        Self {
            chrom1: field(fields, columns.chrom1).to_string(),
            chrom2: field(fields, columns.chrom2).to_string(),
            pos1: int_field(fields, columns.pos1),
            pos2: int_field(fields, columns.pos2),
        }
    }
}

#[derive(Default)]
struct DedupStats {
    total: u64,
    total_mapped: u64,
    total_unmapped: u64,
    total_dups: u64,
    total_nodups: u64,
}

fn process_line(
    line: String,
    columns: &Columns,
    sep: char,
    unmapped_chrom: &str,
    method: DedupMethod,
    max_mismatch: i64,
    mark_dups: bool,
    parents: &mut Vec<ParentRecord>,
    outputs: &mut Outputs,
    stats: &mut DedupStats,
) -> Result<(), Box<dyn std::error::Error>> {
    stats.total += 1;
    let mut fields: Vec<String> = line.split(sep).map(str::to_string).collect();
    if is_unmapped(&fields, columns, unmapped_chrom) {
        stats.total_unmapped += 1;
        outputs.write_unmapped(&line)?;
        return Ok(());
    }

    stats.total_mapped += 1;
    let current = ParentRecord::from_fields(&fields, columns);
    expire_parents(parents, &current, max_mismatch);
    if parents
        .iter()
        .any(|parent| is_duplicate(parent, &current, method, max_mismatch))
    {
        stats.total_dups += 1;
        if mark_dups {
            mark_duplicate(&mut fields, columns);
        }
        outputs.write_dup(&fields.join(&sep.to_string()))?;
    } else {
        stats.total_nodups += 1;
        parents.push(current);
        outputs.write_dedup(&line)?;
    }
    Ok(())
}

fn is_unmapped(fields: &[String], columns: &Columns, unmapped_chrom: &str) -> bool {
    let pair_type = field(fields, columns.pair_type);
    field(fields, columns.chrom1) == unmapped_chrom
        || field(fields, columns.chrom2) == unmapped_chrom
        || pair_type.contains('N')
        || pair_type == "WW"
}

fn expire_parents(parents: &mut Vec<ParentRecord>, current: &ParentRecord, max_mismatch: i64) {
    parents.retain(|parent| {
        parent.chrom1 == current.chrom1
            && parent.chrom2 == current.chrom2
            && current.pos1 - parent.pos1 <= max_mismatch
    });
}

fn is_duplicate(
    parent: &ParentRecord,
    current: &ParentRecord,
    method: DedupMethod,
    max_mismatch: i64,
) -> bool {
    parent.chrom1 == current.chrom1
        && parent.chrom2 == current.chrom2
        && method.is_duplicate(
            (parent.pos1 - current.pos1).abs(),
            (parent.pos2 - current.pos2).abs(),
            max_mismatch,
        )
}

fn mark_duplicate(fields: &mut [String], columns: &Columns) {
    fields[columns.pair_type] = "DD".to_string();
    for idx in [columns.sam1, columns.sam2].into_iter().flatten() {
        if let Some(value) = fields.get_mut(idx) {
            *value = mark_sam_column(value);
        }
    }
}

fn mark_sam_column(value: &str) -> String {
    if value == "." || value.is_empty() {
        return value.to_string();
    }
    value
        .split(INTER_SAM_SEP)
        .map(mark_one_sam_record)
        .collect::<Vec<_>>()
        .join(INTER_SAM_SEP)
}

fn mark_one_sam_record(record: &str) -> String {
    let mut fields: Vec<String> = record.split(SAM_SEP).map(str::to_string).collect();
    if let Some(flag) = fields.get_mut(1) {
        if let Ok(parsed) = flag.parse::<u16>() {
            *flag = (parsed | DUP_FLAG).to_string();
        }
    }
    let mut has_yt = false;
    for field in &mut fields {
        if field.starts_with("Yt:Z:") {
            *field = "Yt:Z:DD".to_string();
            has_yt = true;
        }
    }
    if !has_yt {
        fields.push("Yt:Z:DD".to_string());
    }
    fields.join(&SAM_SEP.to_string())
}

fn field(fields: &[String], idx: usize) -> &str {
    fields.get(idx).map(String::as_str).unwrap_or("")
}

fn int_field(fields: &[String], idx: usize) -> i64 {
    fields
        .get(idx)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

struct Outputs {
    dedup: Box<dyn Write>,
    dups: Option<Box<dyn Write>>,
    unmapped: Option<Box<dyn Write>>,
    dups_to_dedup: bool,
    unmapped_to_dedup: bool,
    unmapped_to_dups: bool,
}

impl Outputs {
    fn open(
        args: &DedupArgs,
        headers: &[String],
        send_header_to: HeaderDestination,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let dedup_path = args.output.as_deref();
        let dups_to_dedup = same_output(args.output_dups.as_deref(), dedup_path);
        let unmapped_to_dedup = same_output(args.output_unmapped.as_deref(), dedup_path);
        let unmapped_to_dups = !unmapped_to_dedup
            && same_output(args.output_unmapped.as_deref(), args.output_dups.as_deref());

        let mut dedup = open_output(dedup_path)?;
        let mut dups = if args.output_dups.is_some() && !dups_to_dedup {
            Some(open_output(args.output_dups.as_deref())?)
        } else {
            None
        };
        let mut unmapped = if args.output_unmapped.is_some()
            && !unmapped_to_dedup
            && !unmapped_to_dups
        {
            Some(open_output(args.output_unmapped.as_deref())?)
        } else {
            None
        };

        if send_header_to.send_dedup() {
            write_headers(dedup.as_mut(), headers)?;
        }
        if send_header_to.send_dups() {
            if dups_to_dedup {
                write_headers(dedup.as_mut(), headers)?;
            } else if let Some(out) = dups.as_mut() {
                write_headers(out.as_mut(), headers)?;
            }
        }
        if send_header_to.send_unmapped() {
            if unmapped_to_dedup {
                write_headers(dedup.as_mut(), headers)?;
            } else if unmapped_to_dups {
                if let Some(out) = dups.as_mut() {
                    write_headers(out.as_mut(), headers)?;
                }
            } else if let Some(out) = unmapped.as_mut() {
                write_headers(out.as_mut(), headers)?;
            }
        }

        Ok(Self {
            dedup,
            dups,
            unmapped,
            dups_to_dedup,
            unmapped_to_dedup,
            unmapped_to_dups,
        })
    }

    fn write_dedup(&mut self, line: &str) -> io::Result<()> {
        writeln!(self.dedup, "{line}")
    }

    fn write_dup(&mut self, line: &str) -> io::Result<()> {
        if self.dups_to_dedup {
            writeln!(self.dedup, "{line}")
        } else if let Some(out) = self.dups.as_mut() {
            writeln!(out, "{line}")
        } else {
            Ok(())
        }
    }

    fn write_unmapped(&mut self, line: &str) -> io::Result<()> {
        if self.unmapped_to_dedup {
            writeln!(self.dedup, "{line}")
        } else if self.unmapped_to_dups {
            if let Some(out) = self.dups.as_mut() {
                writeln!(out, "{line}")?;
            }
            Ok(())
        } else if let Some(out) = self.unmapped.as_mut() {
            writeln!(out, "{line}")
        } else {
            Ok(())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.dedup.flush()?;
        if let Some(out) = self.dups.as_mut() {
            out.flush()?;
        }
        if let Some(out) = self.unmapped.as_mut() {
            out.flush()?;
        }
        Ok(())
    }
}

fn same_output(a: Option<&Path>, b: Option<&Path>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        (Some(a), None) => a == Path::new("-"),
        _ => false,
    }
}

fn write_headers(out: &mut dyn Write, headers: &[String]) -> io::Result<()> {
    for header in headers {
        writeln!(out, "{header}")?;
    }
    Ok(())
}

fn write_stats(path: &Path, stats: &DedupStats) -> Result<(), Box<dyn std::error::Error>> {
    if has_suffix(path, ".gz") || has_suffix(path, ".lz4") {
        return Err("not implemented: compressed dedup stats output".into());
    }
    let mut out = BufWriter::new(OpenOptions::new().create(true).append(true).open(path)?);
    let fraction_dups = if stats.total_mapped == 0 {
        0.0
    } else {
        stats.total_dups as f64 / stats.total_mapped as f64
    };
    writeln!(out, "total\t{}", stats.total)?;
    writeln!(out, "total_mapped\t{}", stats.total_mapped)?;
    writeln!(out, "total_unmapped\t{}", stats.total_unmapped)?;
    writeln!(out, "total_dups\t{}", stats.total_dups)?;
    writeln!(out, "total_nodups\t{}", stats.total_nodups)?;
    writeln!(out, "fraction_dups\t{fraction_dups}")?;
    Ok(())
}

fn decode_sep(value: &str) -> Result<char, Box<dyn std::error::Error>> {
    match value {
        "\\t" => Ok('\t'),
        "\\v" => Ok('\u{0b}'),
        value => {
            let mut chars = value.chars();
            let Some(ch) = chars.next() else {
                return Err("pairtools dedup --sep cannot be empty".into());
            };
            if chars.next().is_some() {
                return Err("not implemented: pairtools dedup --sep with multi-character separators".into());
            }
            Ok(ch)
        }
    }
}

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed dedup input .lz4".into())
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
            Err("not implemented: compressed dedup output .lz4".into())
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
