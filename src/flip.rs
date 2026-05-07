use crate::cli::FlipArgs;
use rust_htslib::htslib;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::os::raw::c_void;
use std::path::Path;

pub fn cmd_flip(args: FlipArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_flip_options(&args)?;
    let chrom_order = read_chrom_order(&args.chroms_path)?;
    let mut reader = open_input(args.input.as_deref())?;
    let (headers, first_body_line) = read_header(reader.as_mut())?;
    let columns = columns_from_header(&headers)?;
    let side_swaps = side_swap_indices(&columns.names);
    let command_line = std::env::args().collect::<Vec<_>>().join(" ");
    let headers = canonical_flip_headers(&append_flip_pg(&headers, &command_line));

    let mut out = open_output(args.output.as_deref())?;
    for header in &headers {
        writeln!(out, "{header}")?;
    }
    if let Some(line) = first_body_line {
        write_flipped_line(&mut out, &line, &columns, &side_swaps, &chrom_order)?;
    }
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = trim_line_end(&line).to_string();
        write_flipped_line(&mut out, &trimmed, &columns, &side_swaps, &chrom_order)?;
    }
    out.flush()?;
    Ok(())
}

fn reject_unsupported_flip_options(args: &FlipArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools flip --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools flip --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools flip --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools flip --cmd-out".into());
    }
    Ok(())
}

struct Columns {
    names: Vec<String>,
    chrom1: usize,
    pos1: usize,
    chrom2: usize,
    pos2: usize,
    pair_type: usize,
}

fn columns_from_header(headers: &[String]) -> Result<Columns, Box<dyn std::error::Error>> {
    let columns_line = headers
        .iter()
        .find(|line| line.starts_with("#columns:"))
        .ok_or("Input .pairs/.pairsam header is missing #columns")?;
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
            .ok_or_else(|| format!("Input .pairs/.pairsam header is missing {name} column").into())
    };
    Ok(Columns {
        chrom1: index("chrom1")?,
        pos1: index("pos1")?,
        chrom2: index("chrom2")?,
        pos2: index("pos2")?,
        pair_type: index("pair_type")?,
        names,
    })
}

fn side_swap_indices(names: &[String]) -> Vec<(usize, usize)> {
    let index: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.as_str(), idx))
        .collect();
    let mut swaps = Vec::new();
    for (idx, name) in names.iter().enumerate() {
        let Some(prefix) = name.strip_suffix('1') else {
            continue;
        };
        let mate = format!("{prefix}2");
        if let Some(&mate_idx) = index.get(mate.as_str()) {
            swaps.push((idx, mate_idx));
        }
    }
    swaps
}

fn write_flipped_line(
    out: &mut Box<dyn Write>,
    line: &str,
    columns: &Columns,
    side_swaps: &[(usize, usize)],
    chrom_order: &ChromOrder,
) -> Result<(), Box<dyn std::error::Error>> {
    if line.is_empty() {
        return Ok(());
    }
    let mut fields: Vec<String> = line.split('\t').map(str::to_string).collect();
    let needs_flip = should_flip(&fields, columns, chrom_order)?;
    if needs_flip {
        for (left, right) in side_swaps {
            if *left < fields.len() && *right < fields.len() {
                fields.swap(*left, *right);
            }
        }
        if columns.pair_type < fields.len() {
            fields[columns.pair_type] = fields[columns.pair_type].chars().rev().collect();
        }
    }
    writeln!(out, "{}", fields.join("\t"))?;
    Ok(())
}

fn should_flip(
    fields: &[String],
    columns: &Columns,
    chrom_order: &ChromOrder,
) -> Result<bool, Box<dyn std::error::Error>> {
    let chrom1 = fields
        .get(columns.chrom1)
        .ok_or("Input row is missing chrom1 column")?;
    let chrom2 = fields
        .get(columns.chrom2)
        .ok_or("Input row is missing chrom2 column")?;
    match chrom_order.compare(chrom1, chrom2) {
        Ordering::Less => Ok(false),
        Ordering::Greater => Ok(true),
        Ordering::Equal => {
            let pos1 = parse_pos(fields.get(columns.pos1), "pos1")?;
            let pos2 = parse_pos(fields.get(columns.pos2), "pos2")?;
            Ok(pos1 > pos2)
        }
    }
}

fn parse_pos(value: Option<&String>, name: &str) -> Result<i64, Box<dyn std::error::Error>> {
    value
        .ok_or_else(|| format!("Input row is missing {name} column"))?
        .parse::<i64>()
        .map_err(|_| format!("Input row has invalid {name} value").into())
}

struct ChromOrder {
    known: HashMap<String, usize>,
}

impl ChromOrder {
    fn compare(&self, left: &str, right: &str) -> Ordering {
        match (self.rank(left), self.rank(right)) {
            (ChromRank::Unmapped, ChromRank::Unmapped) => Ordering::Equal,
            (ChromRank::Unmapped, _) => Ordering::Less,
            (_, ChromRank::Unmapped) => Ordering::Greater,
            (ChromRank::Known(a), ChromRank::Known(b)) => a.cmp(&b),
            (ChromRank::Known(_), ChromRank::Unknown(_)) => Ordering::Less,
            (ChromRank::Unknown(_), ChromRank::Known(_)) => Ordering::Greater,
            (ChromRank::Unknown(a), ChromRank::Unknown(b)) => a.cmp(b),
        }
    }

    fn rank<'a>(&'a self, chrom: &'a str) -> ChromRank<'a> {
        if chrom == "!" {
            ChromRank::Unmapped
        } else if let Some(rank) = self.known.get(chrom) {
            ChromRank::Known(*rank)
        } else {
            ChromRank::Unknown(chrom)
        }
    }
}

enum ChromRank<'a> {
    Unmapped,
    Known(usize),
    Unknown(&'a str),
}

fn read_chrom_order(path: &Path) -> Result<ChromOrder, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut known = HashMap::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(chrom) = trimmed.split_whitespace().next() {
            if chrom != "!" && !known.contains_key(chrom) {
                known.insert(chrom.to_string(), known.len());
            }
        }
    }
    Ok(ChromOrder { known })
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

fn append_flip_pg(headers: &[String], command_line: &str) -> Vec<String> {
    let pg_records = samheader_pg_records(headers);
    if pg_records.is_empty() {
        return headers.to_vec();
    }

    let pp_ids: Vec<&str> = pg_records
        .iter()
        .filter_map(|record| record.pp.as_deref())
        .collect();
    let mut terminals: Vec<&PgRecord> = pg_records
        .iter()
        .filter(|record| !pp_ids.contains(&record.id.as_str()))
        .collect();
    if terminals.is_empty() {
        terminals = pg_records.iter().collect();
    }

    let branch_count = terminals.len();
    let new_records: Vec<String> = terminals
        .iter()
        .enumerate()
        .map(|(idx, terminal)| {
            let id = if branch_count == 1 {
                "pairtools_flip".to_string()
            } else {
                format!(
                    "pairtools_flip-{}.{}",
                    idx + 1,
                    pg_chain_len(terminal, &pg_records) + 1
                )
            };
            format!(
                "#samheader: @PG\tID:{id}\tPN:pairtools_flip\tCL:{command_line}\tPP:{}\tVN:1.1.3",
                terminal.id
            )
        })
        .collect();

    let insert_at = headers
        .iter()
        .rposition(|line| line.starts_with("#samheader:"))
        .map(|idx| idx + 1)
        .unwrap_or(headers.len());
    let mut out = Vec::with_capacity(headers.len() + new_records.len());
    out.extend_from_slice(&headers[..insert_at]);
    out.extend(new_records);
    out.extend_from_slice(&headers[insert_at..]);
    out
}

fn canonical_flip_headers(headers: &[String]) -> Vec<String> {
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

struct PgRecord {
    id: String,
    pp: Option<String>,
}

fn samheader_pg_records(headers: &[String]) -> Vec<PgRecord> {
    let mut records = Vec::new();
    for line in headers {
        let Some(sam) = line.strip_prefix("#samheader: ") else {
            continue;
        };
        if !sam.starts_with("@PG\t") {
            continue;
        }
        let mut id = None;
        let mut pp = None;
        for field in sam.split('\t').skip(1) {
            if let Some(value) = field.strip_prefix("ID:") {
                id = Some(value.to_string());
            } else if let Some(value) = field.strip_prefix("PP:") {
                pp = Some(value.to_string());
            }
        }
        if let Some(id) = id {
            records.push(PgRecord { id, pp });
        }
    }
    records
}

fn pg_chain_len(terminal: &PgRecord, records: &[PgRecord]) -> usize {
    let mut len = 1;
    let mut parent = terminal.pp.as_deref();
    while let Some(parent_id) = parent {
        let Some(record) = records.iter().find(|record| record.id == parent_id) else {
            break;
        };
        len += 1;
        parent = record.pp.as_deref();
    }
    len
}

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufReader::new(BgzfReader::open(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed flip input .lz4".into())
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
            Err("not implemented: compressed flip output .lz4".into())
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
