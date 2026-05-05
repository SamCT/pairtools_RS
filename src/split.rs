use crate::cli::SplitArgs;
use rust_htslib::htslib;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::c_void;
use std::path::Path;

const PAIRSAM_SEP: char = '\x19';

pub fn cmd_split(args: SplitArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_split_options(&args)?;
    let mut reader = open_input(args.input.as_deref())?;
    let (headers, first_body_line) = read_header(reader.as_mut())?;
    let columns = columns_from_header(&headers)?;
    let command_line = std::env::args().collect::<Vec<_>>().join(" ");

    let mut pairs_out = match args.output_pairs.as_deref() {
        Some(path) => Some(open_output(path, "pairs")?),
        None => None,
    };
    let mut sam_out = match args.output_sam.as_deref() {
        Some(path) => Some(open_output(path, "sam")?),
        None => None,
    };

    if let Some(out) = pairs_out.as_mut() {
        for header in split_pairs_headers(&headers, &columns, &command_line) {
            writeln!(out, "{header}")?;
        }
    }
    if let Some(out) = sam_out.as_mut() {
        for header in split_sam_headers(&headers, &command_line) {
            writeln!(out, "{header}")?;
        }
    }

    if let Some(line) = first_body_line {
        write_split_record(&line, &columns, pairs_out.as_mut(), sam_out.as_mut())?;
    }
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = trim_line_end(&line).to_string();
        write_split_record(&trimmed, &columns, pairs_out.as_mut(), sam_out.as_mut())?;
    }

    if let Some(out) = pairs_out.as_mut() {
        out.flush()?;
    }
    if let Some(out) = sam_out.as_mut() {
        out.flush()?;
    }
    Ok(())
}

fn reject_unsupported_split_options(args: &SplitArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools split --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools split --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools split --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools split --cmd-out".into());
    }
    if let Some(path) = args.output_sam.as_deref() {
        if has_suffix(path, ".bam") {
            return Err("not implemented: pairtools split --output-sam .bam".into());
        }
    }
    Ok(())
}

struct Columns {
    names: Vec<String>,
    sam1: usize,
    sam2: usize,
}

fn columns_from_header(headers: &[String]) -> Result<Columns, Box<dyn std::error::Error>> {
    let columns_line = headers
        .iter()
        .find(|line| line.starts_with("#columns:"))
        .ok_or("Input .pairsam header is missing #columns")?;
    let names: Vec<String> = columns_line
        .split_once(':')
        .map(|(_, rest)| rest)
        .unwrap_or("")
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let index = |name: &str| -> Result<usize, Box<dyn std::error::Error>> {
        names
            .iter()
            .position(|column| column == name)
            .ok_or_else(|| format!("Input .pairsam header is missing {name} column").into())
    };
    Ok(Columns {
        sam1: index("sam1")?,
        sam2: index("sam2")?,
        names,
    })
}

fn write_split_record(
    line: &str,
    columns: &Columns,
    pairs_out: Option<&mut Box<dyn Write>>,
    sam_out: Option<&mut Box<dyn Write>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if line.is_empty() {
        return Ok(());
    }
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < columns.names.len() {
        return Err(format!("Input .pairsam row has too few columns: {line}").into());
    }
    if let Some(out) = pairs_out {
        let pairs_fields: Vec<&str> = fields
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| {
                if idx == columns.sam1 || idx == columns.sam2 {
                    None
                } else {
                    Some(*value)
                }
            })
            .collect();
        writeln!(out, "{}", pairs_fields.join("\t"))?;
    }
    if let Some(out) = sam_out {
        write_sam_field(out, fields.get(columns.sam1).copied().unwrap_or(""))?;
        write_sam_field(out, fields.get(columns.sam2).copied().unwrap_or(""))?;
    }
    Ok(())
}

fn write_sam_field(out: &mut Box<dyn Write>, field: &str) -> io::Result<()> {
    if field.is_empty() || field == "." {
        return Ok(());
    }
    let sam = field.replace(PAIRSAM_SEP, "\t");
    writeln!(out, "{sam}")
}

fn split_pairs_headers(headers: &[String], columns: &Columns, command_line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for header in headers {
        if header.starts_with("#columns:") {
            continue;
        }
        out.push(header.clone());
    }
    out.push(format!("#samheader: {}", split_pg_record(command_line)));
    let pairs_columns: Vec<&str> = columns
        .names
        .iter()
        .enumerate()
        .filter_map(|(idx, name)| {
            if idx == columns.sam1 || idx == columns.sam2 {
                None
            } else {
                Some(name.as_str())
            }
        })
        .collect();
    out.push(format!("#columns: {}", pairs_columns.join(" ")));
    canonical_split_headers(&out)
}

fn split_sam_headers(headers: &[String], command_line: &str) -> Vec<String> {
    let mut out: Vec<String> = headers
        .iter()
        .filter_map(|header| header.strip_prefix("#samheader: ").map(str::to_string))
        .collect();
    out.push(split_pg_record(command_line));
    out
}

fn split_pg_record(command_line: &str) -> String {
    format!(
        "@PG\tID:pairtools_split\tPN:pairtools_split\tCL:{command_line}\tVN:1.1.3"
    )
}

fn canonical_split_headers(headers: &[String]) -> Vec<String> {
    let mut primary = Vec::new();
    let mut chroms = Vec::new();
    let mut samheaders = Vec::new();
    let mut columns = Vec::new();
    for header in headers {
        if header.starts_with("#samheader:") {
            samheaders.push(header.clone());
        } else if header.starts_with("#chromosomes:") || header.starts_with("#chromsize:") {
            chroms.push(header.clone());
        } else if header.starts_with("#columns:") {
            columns.push(header.clone());
        } else {
            primary.push(header.clone());
        }
    }
    let mut out = Vec::with_capacity(headers.len());
    out.extend(primary);
    out.extend(chroms);
    out.extend(samheaders);
    out.extend(columns);
    out
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

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed split input .lz4".into())
        }
        Some(path) => Ok(Box::new(BufReader::new(File::open(path)?))),
        None => Ok(Box::new(BufReader::new(io::stdin()))),
    }
}

fn open_output(path: &Path, kind: &str) -> Result<Box<dyn Write>, Box<dyn std::error::Error>> {
    match path {
        path if path == Path::new("-") => Ok(Box::new(BufWriter::new(io::stdout()))),
        path if has_suffix(path, ".gz") => Ok(Box::new(BufWriter::new(BgzfWriter::create(path)?))),
        path if has_suffix(path, ".lz4") => {
            Err(format!("not implemented: compressed split {kind} output .lz4").into())
        }
        path => Ok(Box::new(BufWriter::new(File::create(path)?))),
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
        let read =
            unsafe { htslib::bgzf_read(self.handle, buf.as_mut_ptr() as *mut c_void, buf.len()) };
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
