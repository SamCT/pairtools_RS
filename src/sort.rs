use crate::cli::SortArgs;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use tempfile::NamedTempFile;

const DEFAULT_CHUNK_LINES: usize = 10_000;
const SORTED_HEADER: &str = "#sorted: chr1-chr2-pos1-pos2";

#[derive(Eq)]
struct HeapItem {
    line: String,
    idx: usize,
}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_rows(&other.line, &self.line)
    }
}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line
    }
}

pub fn cmd_sort(args: SortArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_sort_options(&args)?;
    let reader: Box<dyn BufRead> = if let Some(p) = args.input {
        reject_compressed_path(&p, "compressed sort input")?;
        Box::new(BufReader::new(File::open(p)?))
    } else {
        Box::new(BufReader::new(io::stdin()))
    };
    let mut headers = Vec::new();
    let mut chunk = Vec::new();
    let mut files = Vec::new();
    for l in reader.lines() {
        let s = l?;
        if s.starts_with('#') {
            headers.push(s);
            continue;
        }
        if s.is_empty() {
            continue;
        }
        chunk.push(s);
        if chunk.len() >= DEFAULT_CHUNK_LINES {
            spill_chunk(&mut chunk, &mut files, args.tmpdir.as_deref())?;
        }
    }
    if !chunk.is_empty() {
        spill_chunk(&mut chunk, &mut files, args.tmpdir.as_deref())?;
    }
    let mut out: Box<dyn Write> = if let Some(p) = args.output {
        reject_compressed_path(&p, "compressed sort output")?;
        Box::new(BufWriter::new(File::create(p)?))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };
    write_sort_headers(&headers, &mut out)?;
    merge_files(&files, &mut out)?;
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
    if args.nproc.is_some() {
        return Err("not implemented: pairtools sort --nproc".into());
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

fn reject_compressed_path(
    path: &std::path::Path,
    feature: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.to_string_lossy();
    if path.ends_with(".gz") || path.ends_with(".lz4") {
        return Err(format!("not implemented: {feature}").into());
    }
    Ok(())
}

fn write_sort_headers(headers: &[String], out: &mut Box<dyn Write>) -> io::Result<()> {
    let mut wrote_sorted = false;
    for header in headers {
        if header.starts_with("#sorted:") {
            if !wrote_sorted {
                writeln!(out, "{SORTED_HEADER}")?;
                wrote_sorted = true;
            }
            continue;
        }

        writeln!(out, "{header}")?;
        if !wrote_sorted && header.starts_with("## pairs format") {
            writeln!(out, "{SORTED_HEADER}")?;
            wrote_sorted = true;
        }
    }

    if !wrote_sorted {
        writeln!(out, "{SORTED_HEADER}")?;
    }
    Ok(())
}

fn merge_files(
    files: &[NamedTempFile],
    out: &mut Box<dyn Write>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut readers: Vec<BufReader<File>> = files
        .iter()
        .map(|f| BufReader::new(File::open(f.path()).unwrap()))
        .collect();
    let mut heap = BinaryHeap::new();
    for (i, r) in readers.iter_mut().enumerate() {
        let mut s = String::new();
        if r.read_line(&mut s)? > 0 {
            heap.push(HeapItem {
                line: s.trim_end().to_string(),
                idx: i,
            });
        }
    }
    while let Some(item) = heap.pop() {
        writeln!(out, "{}", item.line)?;
        let r = &mut readers[item.idx];
        let mut s = String::new();
        if r.read_line(&mut s)? > 0 {
            heap.push(HeapItem {
                line: s.trim_end().to_string(),
                idx: item.idx,
            });
        }
    }
    Ok(())
}

fn spill_chunk(
    chunk: &mut Vec<String>,
    files: &mut Vec<NamedTempFile>,
    tmp: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    chunk.sort_by(|a, b| cmp_rows(a, b));
    let mut f = if let Some(tmp) = tmp {
        NamedTempFile::new_in(tmp)?
    } else {
        NamedTempFile::new()?
    };
    for r in chunk.iter() {
        writeln!(f, "{r}")?;
    }
    chunk.clear();
    files.push(f);
    Ok(())
}
fn cmp_rows(a: &str, b: &str) -> Ordering {
    let fa: Vec<&str> = a.split('\t').collect();
    let fb: Vec<&str> = b.split('\t').collect();
    let (c1, p1, c2, p2, t1) = extract(&fa);
    let (d1, q1, d2, q2, t2) = extract(&fb);
    c1.cmp(d1)
        .then(c2.cmp(d2))
        .then(p1.cmp(&q1))
        .then(p2.cmp(&q2))
        .then(t1.cmp(t2))
}
fn extract<'a>(f: &[&'a str]) -> (&'a str, i64, &'a str, i64, &'a str) {
    let o = if f.len() > 7 { 1 } else { 0 };
    (
        f.get(o).copied().unwrap_or(""),
        f.get(o + 1).and_then(|x| x.parse().ok()).unwrap_or(0),
        f.get(o + 2).copied().unwrap_or(""),
        f.get(o + 3).and_then(|x| x.parse().ok()).unwrap_or(0),
        f.get(o + 6).copied().unwrap_or(""),
    )
}
