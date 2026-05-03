use crate::cli::ParseArgs;
use rust_htslib::bam::{self, Read};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

#[derive(Clone)]
struct Aln {
    q: String,
    chr: String,
    pos: i64,
    strand: char,
    mapped: bool,
    uniq: bool,
}

pub fn cmd_parse(args: ParseArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(
        args.walks_policy.as_str(),
        "mask" | "5any" | "5unique" | "3any" | "3unique" | "all"
    ) {
        return Err("not implemented".into());
    }
    if args.report_alignment_end != "5" && args.report_alignment_end != "3" {
        return Err("not implemented".into());
    }
    if !args.drop_sam {
        return Err("not implemented: pairsam output requires SAM columns".into());
    }
    let order = read_chroms(&args.chroms_path)?;
    let mut out: Box<dyn Write> = if let Some(p) = args.output {
        Box::new(BufWriter::new(File::create(p)?))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };
    writeln!(out, "## pairs format v1.0.0")?;
    writeln!(
        out,
        "#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type"
    )?;

    let mut bam = if let Some(p) = args.input {
        bam::Reader::from_path(p)?
    } else {
        bam::Reader::from_stdin()?
    };
    let header = bam.header().to_owned();
    let mut by_qname: Vec<Aln> = Vec::new();
    let mut current_q = String::new();
    let mut total = 0usize;
    for rec in bam.records() {
        let r = rec?;
        total += 1;
        let q = String::from_utf8_lossy(r.qname()).to_string();
        if current_q.is_empty() {
            current_q = q.clone();
        }
        if q != current_q {
            emit_template(&mut out, &by_qname, &order)?;
            by_qname.clear();
            current_q = q.clone();
        }
        let mapped = !r.is_unmapped() && r.mapq() >= args.min_mapq;
        let chr = if mapped {
            String::from_utf8_lossy(header.tid2name(r.tid() as u32)).to_string()
        } else {
            "!".into()
        };
        let left = r.pos() + 1;
        let reflen = r.cigar().end_pos() - r.pos();
        let right = left + reflen - 1;
        let five = if r.is_reverse() { right } else { left };
        let three = if r.is_reverse() { left } else { right };
        let pos = if args.report_alignment_end == "3" {
            three
        } else {
            five
        };
        by_qname.push(Aln {
            q,
            chr,
            pos,
            strand: if r.is_reverse() { '-' } else { '+' },
            mapped,
            uniq: r.mapq() >= args.min_mapq,
        });
    }
    if !by_qname.is_empty() {
        emit_template(&mut out, &by_qname, &order)?;
    }
    if let Some(sp) = args.output_stats {
        std::fs::write(sp, format!("total\t{total}\n"))?;
    }
    Ok(())
}

fn emit_template(
    out: &mut Box<dyn Write>,
    t: &[Aln],
    order: &HashMap<String, usize>,
) -> io::Result<()> {
    let mapped: Vec<Aln> = t.iter().filter(|x| x.mapped).cloned().collect();
    let pick = if mapped.len() >= 2 {
        vec![mapped[0].clone(), mapped[1].clone()]
    } else if mapped.len() == 1 {
        vec![
            mapped[0].clone(),
            Aln {
                q: mapped[0].q.clone(),
                chr: "!".into(),
                pos: 0,
                strand: '.',
                mapped: false,
                uniq: false,
            },
        ]
    } else if t.len() >= 2 {
        vec![t[0].clone(), t[1].clone()]
    } else {
        vec![
            t[0].clone(),
            Aln {
                q: t[0].q.clone(),
                chr: "!".into(),
                pos: 0,
                strand: '.',
                mapped: false,
                uniq: false,
            },
        ]
    };
    emit_pair(out, pick[0].clone(), pick[1].clone(), order)
}

fn emit_pair(
    out: &mut Box<dyn Write>,
    a: Aln,
    b: Aln,
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
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        x.q, c1, p1, c2, p2, s1, s2, pt
    )
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
    oa > ob || (oa == ob && (a.pos > b.pos || (a.pos == b.pos && a.strand > b.strand)))
}
fn read_chroms(p: &Path) -> Result<HashMap<String, usize>, Box<dyn std::error::Error>> {
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
