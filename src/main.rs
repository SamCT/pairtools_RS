use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Err("usage: pairs-rs <command> [options]".into());
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "parse" => cmd_parse(args),
        "sort" => cmd_sort(args),
        "parse2" | "dedup" | "flip" | "merge" | "split" | "select" | "stats" | "restrict"
        | "filterbycov" | "phase" | "markasdup" => Err(format!(
            "command '{cmd}' is recognized but not implemented yet; failing loudly for compatibility tracking"
        )
        .into()),
        _ => Err(format!("unknown command: {cmd}").into()),
    }
}

#[derive(Clone)]
struct Aln {
    q: String,
    chr: String,
    pos: i64,
    strand: char,
    mapped: bool,
}

fn cmd_parse(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut chroms: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut min_mapq: u8 = 1;
    let mut report = "5prime".to_string();
    let mut drop_readid = false;
    let mut input: Option<PathBuf> = None;
    let mut _threads: usize = 1;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--chroms-path" => {
                i += 1;
                chroms = Some(PathBuf::from(args.get(i).ok_or("missing chroms path")?));
            }
            "-o" | "--output" => {
                i += 1;
                output = Some(PathBuf::from(args.get(i).ok_or("missing output")?));
            }
            "--threads" | "-@" => {
                i += 1;
                _threads = args.get(i).ok_or("missing threads value")?.parse()?;
                if _threads == 0 {
                    return Err("--threads/-@ must be >= 1".into());
                }
            }
            "--min-mapq" => {
                i += 1;
                min_mapq = args.get(i).ok_or("missing min-mapq")?.parse()?;
            }
            "--report-alignment-end" => {
                i += 1;
                report = args.get(i).ok_or("missing report-alignment-end")?.clone();
                if report != "5prime" && report != "3prime" {
                    return Err("--report-alignment-end must be 5prime or 3prime".into());
                }
            }
            "--drop-readid" => drop_readid = true,
            s if s.starts_with('-') => return Err(format!("unsupported parse option: {s}").into()),
            p => input = Some(PathBuf::from(p)),
        }
        i += 1;
    }

    let order = read_chroms(chroms.ok_or("parse requires -c/--chroms-path")?)?;
    let reader: Box<dyn BufRead> = if let Some(p) = input {
        Box::new(BufReader::new(File::open(p)?))
    } else {
        Box::new(BufReader::new(io::stdin()))
    };
    let mut out: Box<dyn Write> = if let Some(p) = output {
        Box::new(BufWriter::new(File::create(p)?))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };

    writeln!(out, "## pairs format v1.0.0")?;
    writeln!(
        out,
        "#columns: {}chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type",
        if drop_readid { "" } else { "readID " }
    )?;

    let mut pending: Option<Aln> = None;
    for l in reader.lines() {
        let line = l?;
        if line.starts_with('@') || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        let flag: u16 = f[1].parse().unwrap_or(0);
        let mapq: u8 = f[4].parse().unwrap_or(0);
        let mapped = (flag & 0x4) == 0 && f[2] != "*" && mapq >= min_mapq;
        let pos = i64::from(f[3].parse::<i32>().unwrap_or(0));
        let adj = if mapped && report == "3prime" { 1 } else { 0 };
        let rec = Aln {
            q: f[0].into(),
            chr: f[2].into(),
            pos: pos + adj,
            strand: if flag & 16 != 0 { '-' } else { '+' },
            mapped,
        };

        pending = match pending {
            None => Some(rec),
            Some(p) => {
                if p.q == rec.q {
                    emit_pair(&mut out, p, rec, drop_readid, &order)?;
                    None
                } else {
                    emit_unpaired(&mut out, p, drop_readid)?;
                    Some(rec)
                }
            }
        };
    }
    if let Some(p) = pending {
        emit_unpaired(&mut out, p, drop_readid)?;
    }
    Ok(())
}

fn emit_unpaired(out: &mut Box<dyn Write>, r: Aln, drop_id: bool) -> io::Result<()> {
    let (c, p, s) = side(&r);
    if drop_id {
        writeln!(out, "{}\t{}\t!\t0\t{}\t.\tMU", c, p, s)
    } else {
        writeln!(out, "{}\t{}\t{}\t!\t0\t{}\t.\tMU", r.q, c, p, s)
    }
}

fn emit_pair(
    out: &mut Box<dyn Write>,
    a: Aln,
    b: Aln,
    drop_id: bool,
    order: &HashMap<String, usize>,
) -> io::Result<()> {
    let (x, y) = if should_flip(&a, &b, order) {
        (b, a)
    } else {
        (a, b)
    };
    let (c1, p1, s1) = side(&x);
    let (c2, p2, s2) = side(&y);
    let pt = match (x.mapped, y.mapped) {
        (true, true) => "UU",
        (true, false) => "UM",
        (false, true) => "MU",
        (false, false) => "NN",
    };
    if drop_id {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            c1, p1, c2, p2, s1, s2, pt
        )
    } else {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            x.q, c1, p1, c2, p2, s1, s2, pt
        )
    }
}

fn side(r: &Aln) -> (String, i64, char) {
    if r.mapped {
        (r.chr.clone(), r.pos, r.strand)
    } else {
        ("!".into(), 0, '.')
    }
}

fn should_flip(a: &Aln, b: &Aln, o: &HashMap<String, usize>) -> bool {
    let oa = *o.get(&a.chr).unwrap_or(&usize::MAX);
    let ob = *o.get(&b.chr).unwrap_or(&usize::MAX);
    oa > ob || (oa == ob && a.pos > b.pos)
}

fn read_chroms(p: PathBuf) -> Result<HashMap<String, usize>, Box<dyn std::error::Error>> {
    let mut m = HashMap::new();
    for (i, l) in BufReader::new(File::open(p)?).lines().enumerate() {
        let s = l?;
        if let Some(c) = s.split('\t').next() {
            if !c.is_empty() {
                m.insert(c.into(), i);
            }
        }
    }
    Ok(m)
}

fn cmd_sort(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut _threads: usize = 1;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output = Some(PathBuf::from(args.get(i).ok_or("missing output")?));
            }
            "--nproc" | "--threads" | "-@" => {
                i += 1;
                _threads = args.get(i).ok_or("missing threads value")?.parse()?;
                if _threads == 0 {
                    return Err("--nproc/--threads/-@ must be >= 1".into());
                }
            }
            s if s.starts_with('-') => return Err(format!("unsupported sort option: {s}").into()),
            p => input = Some(PathBuf::from(p)),
        }
        i += 1;
    }

    let reader: Box<dyn BufRead> = if let Some(p) = input {
        Box::new(BufReader::new(File::open(p)?))
    } else {
        Box::new(BufReader::new(io::stdin()))
    };
    let mut h = Vec::new();
    let mut rows = Vec::new();
    for l in reader.lines() {
        let s = l?;
        if s.starts_with('#') {
            h.push(s)
        } else if !s.is_empty() {
            rows.push(s)
        }
    }
    rows.sort_by(|a, b| cmp_rows(a, b));

    let mut out: Box<dyn Write> = if let Some(p) = output {
        Box::new(BufWriter::new(File::create(p)?))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };
    for x in h {
        writeln!(out, "{x}")?;
    }
    for x in rows {
        writeln!(out, "{x}")?;
    }
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
