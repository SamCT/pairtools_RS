use crate::cli::MergeArgs;
use rust_htslib::htslib;
use std::cmp::Ordering;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::c_void;
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct Columns {
    chrom1: usize,
    chrom2: usize,
    pos1: usize,
    pos2: usize,
}

#[derive(Clone, Eq, PartialEq)]
struct MergeKey {
    chrom1: String,
    chrom2: String,
    pos1: i64,
    pos2: i64,
}

impl Ord for MergeKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.chrom1
            .cmp(&other.chrom1)
            .then(self.chrom2.cmp(&other.chrom2))
            .then(self.pos1.cmp(&other.pos1))
            .then(self.pos2.cmp(&other.pos2))
    }
}

impl PartialOrd for MergeKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct MergeRecord {
    key: MergeKey,
    input_idx: usize,
    ordinal: usize,
    line: String,
}

pub fn cmd_merge(args: MergeArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_merge_options(&args)?;
    let inputs = merge_inputs(&args.inputs);
    let mut parsed = Vec::new();
    for input in &inputs {
        parsed.push(read_pairs_file(input.as_deref())?);
    }
    if parsed.is_empty() {
        return Err("pairs-rs merge requires at least one input or stdin".into());
    }

    let columns = columns_from_header(&parsed[0].headers)?;
    let command_line = std::env::args().collect::<Vec<_>>().join(" ");
    let headers = merged_headers(&parsed, &command_line);
    let mut records = Vec::new();
    for (input_idx, file) in parsed.iter().enumerate() {
        for (ordinal, line) in file.body.iter().enumerate() {
            records.push(MergeRecord::new(line.clone(), &columns, input_idx, ordinal));
        }
    }
    records.sort_by(|a, b| {
        a.key
            .cmp(&b.key)
            .then(a.input_idx.cmp(&b.input_idx))
            .then(a.ordinal.cmp(&b.ordinal))
    });

    let mut out = open_output(args.output.as_deref())?;
    for header in headers {
        writeln!(out, "{header}")?;
    }
    for record in records {
        writeln!(out, "{}", record.line)?;
    }
    out.flush()?;
    Ok(())
}

fn reject_unsupported_merge_options(args: &MergeArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.max_nmerge.is_some() {
        return Err("not implemented: pairtools merge --max-nmerge".into());
    }
    if args.tmpdir.is_some() {
        return Err("not implemented: pairtools merge --tmpdir".into());
    }
    if args.memory.is_some() {
        return Err("not implemented: pairtools merge --memory".into());
    }
    if args.compress_program.is_some() {
        return Err("not implemented: pairtools merge --compress-program".into());
    }
    if args.nproc.is_some() {
        return Err("not implemented: pairtools merge --nproc".into());
    }
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools merge --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools merge --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools merge --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools merge --cmd-out".into());
    }
    if args.keep_first_header {
        return Err("not implemented: pairtools merge --keep-first-header".into());
    }
    if args.no_keep_first_header {
        return Err("not implemented: pairtools merge --no-keep-first-header".into());
    }
    if args.concatenate {
        return Err("not implemented: pairtools merge --concatenate".into());
    }
    if args.no_concatenate {
        return Err("not implemented: pairtools merge --no-concatenate".into());
    }
    Ok(())
}

fn merge_inputs(inputs: &[PathBuf]) -> Vec<Option<&Path>> {
    if inputs.is_empty() {
        vec![None]
    } else {
        inputs.iter().map(|path| Some(path.as_path())).collect()
    }
}

struct PairsFile {
    headers: Vec<String>,
    body: Vec<String>,
}

fn read_pairs_file(path: Option<&Path>) -> Result<PairsFile, Box<dyn std::error::Error>> {
    let mut reader = open_input(path)?;
    let mut headers = Vec::new();
    let mut body = Vec::new();
    let mut line = String::new();
    while reader.read_line(&mut line)? != 0 {
        let trimmed = trim_line_end(&line).to_string();
        if trimmed.starts_with('#') && body.is_empty() {
            headers.push(trimmed);
        } else if !trimmed.is_empty() {
            body.push(trimmed);
        }
        line.clear();
    }
    Ok(PairsFile { headers, body })
}

fn columns_from_header(headers: &[String]) -> Result<Columns, Box<dyn std::error::Error>> {
    let columns_line = headers
        .iter()
        .find(|line| line.starts_with("#columns:"))
        .ok_or("Input .pairs/.pairsam header is missing #columns")?;
    let columns: Vec<&str> = columns_line
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or("")
        .split_whitespace()
        .collect();
    let index = |name: &str| -> Result<usize, Box<dyn std::error::Error>> {
        columns
            .iter()
            .position(|column| *column == name)
            .ok_or_else(|| format!("Input .pairs/.pairsam header is missing {name} column").into())
    };
    Ok(Columns {
        chrom1: index("chrom1")?,
        chrom2: index("chrom2")?,
        pos1: index("pos1")?,
        pos2: index("pos2")?,
    })
}

impl MergeRecord {
    fn new(line: String, columns: &Columns, input_idx: usize, ordinal: usize) -> Self {
        let fields: Vec<&str> = line.split('\t').collect();
        let key = MergeKey {
            chrom1: text_field(&fields, columns.chrom1).to_string(),
            chrom2: text_field(&fields, columns.chrom2).to_string(),
            pos1: int_field(&fields, columns.pos1),
            pos2: int_field(&fields, columns.pos2),
        };
        Self {
            key,
            input_idx,
            ordinal,
            line,
        }
    }
}

fn text_field<'a>(fields: &[&'a str], idx: usize) -> &'a str {
    fields.get(idx).copied().unwrap_or("")
}

fn int_field(fields: &[&str], idx: usize) -> i64 {
    fields
        .get(idx)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0)
}

fn merged_headers(files: &[PairsFile], command_line: &str) -> Vec<String> {
    let first = &files[0].headers;
    let mut primary = Vec::new();
    let mut chroms = Vec::new();
    let mut sq = Vec::new();
    let mut pg = Vec::new();
    let mut columns = Vec::new();
    for header in first {
        if header.starts_with("#samheader: @SQ\t") {
            sq.push(header.clone());
        } else if header.starts_with("#samheader: @PG\t") {
            continue;
        } else if header.starts_with("#samheader:") {
            sq.push(header.clone());
        } else if header.starts_with("#chromosomes:") || header.starts_with("#chromsize:") {
            chroms.push(header.clone());
        } else if header.starts_with("#columns:") {
            columns.push(header.clone());
        } else {
            primary.push(header.clone());
        }
    }
    chroms.sort();

    for (idx, file) in files.iter().enumerate() {
        let suffix = idx + 1;
        let mut terminal = None;
        for header in &file.headers {
            if !header.starts_with("#samheader: @PG\t") {
                continue;
            }
            let suffixed = suffix_pg_record(header, suffix);
            terminal = pg_id_from_header(&suffixed);
            pg.push(suffixed);
        }
        if let Some(pp) = terminal {
            pg.push(format!(
                "#samheader: @PG\tID:pairtools_merge-{suffix}.2\tPN:pairtools_merge\tCL:{command_line}\tPP:{pp}\tVN:1.1.3"
            ));
        }
    }

    let mut out = Vec::new();
    out.extend(primary);
    out.extend(chroms);
    out.extend(sq);
    out.extend(pg);
    out.extend(columns);
    out
}

fn suffix_pg_record(header: &str, suffix: usize) -> String {
    let Some(rest) = header.strip_prefix("#samheader: @PG\t") else {
        return header.to_string();
    };
    let fields = rest
        .split('\t')
        .map(|field| {
            if let Some(value) = field.strip_prefix("ID:") {
                format!("ID:{value}-{suffix}")
            } else if let Some(value) = field.strip_prefix("PP:") {
                format!("PP:{value}-{suffix}")
            } else {
                field.to_string()
            }
        })
        .collect::<Vec<_>>();
    format!("#samheader: @PG\t{}", fields.join("\t"))
}

fn pg_id_from_header(header: &str) -> Option<String> {
    header
        .strip_prefix("#samheader: @PG\t")?
        .split('\t')
        .find_map(|field| field.strip_prefix("ID:").map(str::to_string))
}

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed merge input .lz4".into())
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
            Err("not implemented: compressed merge output .lz4".into())
        }
        Some(path) => Ok(Box::new(BufWriter::new(File::create(path)?))),
        None => Ok(Box::new(BufWriter::new(io::stdout()))),
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
