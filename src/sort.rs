use crate::cli::SortArgs;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ffi::{c_void, CString};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::os::raw::{c_int, c_uint};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use tempfile::NamedTempFile;

const DEFAULT_NPROC: usize = 8;
const DEFAULT_CHUNK_LINES: usize = 10_000;
const PARALLEL_SORT_MIN_LINES: usize = 4_096;
const SORTED_HEADER: &str = "#sorted: chr1-chr2-pos1-pos2";

#[derive(Clone)]
struct SortColumns {
    chrom1: usize,
    chrom2: usize,
    pos1: usize,
    pos2: usize,
    pair_type: usize,
}

#[derive(Clone, Eq, PartialEq)]
struct SortKey {
    chrom1: String,
    chrom2: String,
    pos1: i64,
    pos2: i64,
    pair_type: String,
}

impl Ord for SortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.chrom1
            .cmp(&other.chrom1)
            .then(self.chrom2.cmp(&other.chrom2))
            .then(self.pos1.cmp(&other.pos1))
            .then(self.pos2.cmp(&other.pos2))
            .then(self.pair_type.cmp(&other.pair_type))
    }
}

impl PartialOrd for SortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct PairRecord {
    key: SortKey,
    ordinal: u64,
    line: String,
}

struct SpilledChunk {
    file: NamedTempFile,
    index: usize,
}

type WorkerResult = Result<SpilledChunk, String>;

#[derive(Eq)]
struct HeapItem {
    record: PairRecord,
    reader_idx: usize,
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_records(&other.record, &self.record)
            .then_with(|| other.reader_idx.cmp(&self.reader_idx))
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.reader_idx == other.reader_idx && compare_records(&self.record, &other.record).is_eq()
    }
}

impl Eq for PairRecord {}

impl PartialEq for PairRecord {
    fn eq(&self, other: &Self) -> bool {
        self.ordinal == other.ordinal && self.key == other.key
    }
}

pub fn cmd_sort(args: SortArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_sort_options(&args)?;
    let nproc = args.nproc.unwrap_or(DEFAULT_NPROC);
    if nproc == 0 {
        return Err("pairtools sort --nproc must be greater than zero".into());
    }

    let mut reader = open_input(args.input.as_deref())?;
    let (headers, first_body_line) = read_header(reader.as_mut())?;
    let columns = sort_columns_from_header(&headers)?;
    let command_line = std::env::args().collect::<Vec<_>>().join(" ");
    let headers = append_sort_pg(&headers, &command_line);
    let headers = sorted_headers(&headers)?;

    let tmpdir = args.tmpdir.clone();
    let mut files = sort_to_temp_files(reader.as_mut(), first_body_line, &columns, tmpdir, nproc)?;
    files.sort_by_key(|chunk| chunk.index);

    let mut out = open_output(args.output.as_deref())?;
    for header in &headers {
        writeln!(out, "{header}")?;
    }
    merge_files(&files, &columns, out.as_mut())?;
    out.flush()?;
    Ok(())
}

fn reject_unsupported_sort_options(args: &SortArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.c1.is_some() {
        return Err("not implemented: pairtools sort --c1".into());
    }
    if args.c2.is_some() {
        return Err("not implemented: pairtools sort --c2".into());
    }
    if args.p1.is_some() {
        return Err("not implemented: pairtools sort --p1".into());
    }
    if args.p2.is_some() {
        return Err("not implemented: pairtools sort --p2".into());
    }
    if args.pt.is_some() {
        return Err("not implemented: pairtools sort --pt".into());
    }
    if !args.extra_col.is_empty() {
        return Err("not implemented: pairtools sort --extra-col".into());
    }
    if args.memory.is_some() {
        return Err("not implemented: pairtools sort --memory".into());
    }
    if args.compress_program.is_some() {
        return Err("not implemented: pairtools sort --compress-program".into());
    }
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools sort --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools sort --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools sort --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools sort --cmd-out".into());
    }
    Ok(())
}

fn open_input(path: Option<&Path>) -> Result<Box<dyn BufRead + Send>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufReader::new(io::stdin()))),
        Some(path) => {
            reject_compressed_input(path)?;
            Ok(Box::new(BufReader::new(File::open(path)?)))
        }
        None => Ok(Box::new(BufReader::new(io::stdin()))),
    }
}

fn open_output(path: Option<&Path>) -> Result<Box<dyn Write>, Box<dyn std::error::Error>> {
    match path {
        Some(path) if path == Path::new("-") => Ok(Box::new(BufWriter::new(io::stdout()))),
        Some(path) if has_suffix(path, ".gz") => {
            Ok(Box::new(BufWriter::new(GzipWriter::create(path)?)))
        }
        Some(path) if has_suffix(path, ".lz4") => {
            Err("not implemented: compressed sort output .lz4".into())
        }
        Some(path) => Ok(Box::new(BufWriter::new(File::create(path)?))),
        None => Ok(Box::new(BufWriter::new(io::stdout()))),
    }
}

fn reject_compressed_input(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if has_suffix(path, ".gz") || has_suffix(path, ".lz4") {
        return Err("not implemented: compressed sort input".into());
    }
    Ok(())
}

fn has_suffix(path: &Path, suffix: &str) -> bool {
    path.to_string_lossy().ends_with(suffix)
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

fn sorted_headers(headers: &[String]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if headers.is_empty() || !headers[0].starts_with("##") {
        return Err("Input file is not valid .pairs/.pairsam, has no header or is empty.".into());
    }

    let has_sorted = headers.iter().any(|line| line.starts_with("#sorted"));
    let mut sorted = Vec::with_capacity(headers.len() + usize::from(!has_sorted));
    for (idx, header) in headers.iter().enumerate() {
        if header.starts_with("#chromosomes") {
            sorted.push(sort_chromosomes_header(header));
        } else {
            sorted.push(header.clone());
        }

        if idx == 0 && !has_sorted {
            sorted.push(SORTED_HEADER.to_string());
        }
    }
    Ok(sorted)
}

fn append_sort_pg(headers: &[String], command_line: &str) -> Vec<String> {
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
                "pairtools_sort".to_string()
            } else {
                format!(
                    "pairtools_sort-{}.{}",
                    idx + 1,
                    pg_chain_len(terminal, &pg_records) + 1
                )
            };
            format!(
                "#samheader: @PG\tID:{id}\tPN:pairtools_sort\tCL:{command_line}\tPP:{}\tVN:1.1.3",
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

fn sort_chromosomes_header(header: &str) -> String {
    let Some((name, value)) = header.split_once(':') else {
        return header.to_string();
    };
    let mut chroms: Vec<&str> = value.split_whitespace().collect();
    chroms.sort_unstable();
    format!("{name}: {}", chroms.join(" "))
}

fn sort_columns_from_header(headers: &[String]) -> Result<SortColumns, Box<dyn std::error::Error>> {
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

    Ok(SortColumns {
        chrom1: column_index(&columns, "chrom1")?,
        chrom2: column_index(&columns, "chrom2")?,
        pos1: column_index(&columns, "pos1")?,
        pos2: column_index(&columns, "pos2")?,
        pair_type: column_index(&columns, "pair_type")?,
    })
}

fn column_index(columns: &[&str], name: &str) -> Result<usize, Box<dyn std::error::Error>> {
    columns
        .iter()
        .position(|column| *column == name)
        .ok_or_else(|| {
            format!("Input .pairs/.pairsam header is missing required column {name}").into()
        })
}

fn sort_to_temp_files(
    reader: &mut (dyn BufRead + Send),
    first_body_line: Option<String>,
    columns: &SortColumns,
    tmpdir: Option<PathBuf>,
    nproc: usize,
) -> Result<Vec<SpilledChunk>, Box<dyn std::error::Error>> {
    let pool = ThreadPoolBuilder::new().num_threads(nproc).build()?;
    let (tx, rx) = mpsc::channel::<WorkerResult>();
    let columns = columns.clone();

    let read_result: Result<usize, String> = pool.scope(|scope| {
        let mut chunk = Vec::with_capacity(DEFAULT_CHUNK_LINES);
        let mut ordinal = 0_u64;
        let mut chunk_index = 0_usize;

        if let Some(line) = first_body_line {
            push_record(&mut chunk, line, &mut ordinal, &columns)?;
        }

        let mut line = String::new();
        loop {
            line.clear();
            let bytes = reader.read_line(&mut line).map_err(|err| err.to_string())?;
            if bytes == 0 {
                break;
            }
            push_record(
                &mut chunk,
                trim_line_end(&line).to_string(),
                &mut ordinal,
                &columns,
            )?;
            if chunk.len() >= DEFAULT_CHUNK_LINES {
                spawn_chunk(
                    scope,
                    tx.clone(),
                    take_chunk(&mut chunk),
                    chunk_index,
                    tmpdir.clone(),
                    nproc,
                );
                chunk_index += 1;
            }
        }

        if !chunk.is_empty() {
            spawn_chunk(scope, tx.clone(), chunk, chunk_index, tmpdir.clone(), nproc);
            chunk_index += 1;
        }

        Ok(chunk_index)
    });

    let chunk_count = read_result.map_err(|err| -> Box<dyn std::error::Error> { err.into() })?;
    drop(tx);

    let mut files = Vec::with_capacity(chunk_count);
    for _ in 0..chunk_count {
        match rx.recv()? {
            Ok(file) => files.push(file),
            Err(err) => return Err(err.into()),
        }
    }
    Ok(files)
}

fn push_record(
    chunk: &mut Vec<PairRecord>,
    line: String,
    ordinal: &mut u64,
    columns: &SortColumns,
) -> Result<(), String> {
    let record = PairRecord::from_line(line, *ordinal, columns);
    *ordinal = (*ordinal)
        .checked_add(1)
        .ok_or_else(|| "too many input rows to preserve stable sort order".to_string())?;
    chunk.push(record);
    Ok(())
}

fn take_chunk(chunk: &mut Vec<PairRecord>) -> Vec<PairRecord> {
    std::mem::replace(chunk, Vec::with_capacity(DEFAULT_CHUNK_LINES))
}

fn spawn_chunk<'scope>(
    scope: &rayon::Scope<'scope>,
    tx: Sender<WorkerResult>,
    chunk: Vec<PairRecord>,
    index: usize,
    tmpdir: Option<PathBuf>,
    nproc: usize,
) {
    scope.spawn(move |_| {
        let result =
            sort_and_spill_chunk(chunk, index, tmpdir, nproc).map_err(|err| err.to_string());
        let _ = tx.send(result);
    });
}

fn sort_and_spill_chunk(
    mut chunk: Vec<PairRecord>,
    index: usize,
    tmpdir: Option<PathBuf>,
    nproc: usize,
) -> io::Result<SpilledChunk> {
    if nproc > 1 && chunk.len() >= PARALLEL_SORT_MIN_LINES {
        chunk.par_sort_unstable_by(compare_records);
    } else {
        chunk.sort_unstable_by(compare_records);
    }

    let mut file = if let Some(tmpdir) = tmpdir {
        NamedTempFile::new_in(tmpdir)?
    } else {
        NamedTempFile::new()?
    };
    for record in &chunk {
        writeln!(file, "{}\t{}", record.ordinal, record.line)?;
    }
    file.flush()?;

    Ok(SpilledChunk { file, index })
}

fn merge_files(
    files: &[SpilledChunk],
    columns: &SortColumns,
    out: &mut dyn Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut readers: Vec<BufReader<File>> = files
        .iter()
        .map(|chunk| File::open(chunk.file.path()).map(BufReader::new))
        .collect::<io::Result<_>>()?;
    let mut heap = BinaryHeap::new();

    for (reader_idx, reader) in readers.iter_mut().enumerate() {
        if let Some(record) = read_temp_record(reader, columns)? {
            heap.push(HeapItem { record, reader_idx });
        }
    }

    while let Some(item) = heap.pop() {
        writeln!(out, "{}", item.record.line)?;
        if let Some(record) = read_temp_record(&mut readers[item.reader_idx], columns)? {
            heap.push(HeapItem {
                record,
                reader_idx: item.reader_idx,
            });
        }
    }

    Ok(())
}

fn read_temp_record(
    reader: &mut dyn BufRead,
    columns: &SortColumns,
) -> io::Result<Option<PairRecord>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let trimmed = trim_line_end(&line);
    let (ordinal, body) = trimmed
        .split_once('\t')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed sort chunk"))?;
    let ordinal = ordinal
        .parse::<u64>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "malformed sort ordinal"))?;
    Ok(Some(PairRecord::from_line(
        body.to_string(),
        ordinal,
        columns,
    )))
}

impl PairRecord {
    fn from_line(line: String, ordinal: u64, columns: &SortColumns) -> Self {
        let fields: Vec<&str> = line.split('\t').collect();
        let key = SortKey {
            chrom1: text_field(&fields, columns.chrom1).to_string(),
            chrom2: text_field(&fields, columns.chrom2).to_string(),
            pos1: int_field(&fields, columns.pos1),
            pos2: int_field(&fields, columns.pos2),
            pair_type: text_field(&fields, columns.pair_type).to_string(),
        };
        Self { key, ordinal, line }
    }
}

fn compare_records(a: &PairRecord, b: &PairRecord) -> Ordering {
    a.key.cmp(&b.key).then(a.ordinal.cmp(&b.ordinal))
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

fn trim_line_end(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

struct GzipWriter {
    handle: libz_sys::gzFile,
}

impl GzipWriter {
    fn create(path: &Path) -> io::Result<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "output path contains NUL byte")
        })?;
        let mode = CString::new("wb").expect("static gzip mode has no NUL bytes");
        let handle = unsafe { libz_sys::gzopen(path.as_ptr(), mode.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }

    fn close(&mut self) -> io::Result<()> {
        if self.handle.is_null() {
            return Ok(());
        }
        let status = unsafe { libz_sys::gzclose(self.handle) };
        self.handle = std::ptr::null_mut();
        if status == libz_sys::Z_OK as c_int {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to close gzip stream, zlib status {status}"),
            ))
        }
    }
}

impl Write for GzipWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let len = buf.len().min(c_uint::MAX as usize);
        let written =
            unsafe { libz_sys::gzwrite(self.handle, buf.as_ptr() as *mut c_void, len as c_uint) };
        if written <= 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write gzip stream",
            ))
        } else {
            Ok(written as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let status = unsafe { libz_sys::gzflush(self.handle, libz_sys::Z_SYNC_FLUSH as c_int) };
        if status == libz_sys::Z_OK as c_int {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to flush gzip stream, zlib status {status}"),
            ))
        }
    }
}

impl Drop for GzipWriter {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
