use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
struct LiteRec {
    qname: String,
    chrom: String,
    pos: i64,
    strand: char,
    mapped: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let print_header = !std::env::args().any(|a| a == "--no-header");
    if print_header {
        println!("## pairs format v1.0.0");
        println!("#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type");
    }

    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes)?;
    if bytes.starts_with(b"BAM\x01") {
        let sam = bam_to_sam(&bytes)?;
        parse_sam_records(sam.as_bytes())?;
    } else {
        parse_sam_records(&bytes)?;
    }
    Ok(())
}

fn bam_to_sam(bam_bytes: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut child = Command::new("samtools")
        .args(["view", "-h", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    child.stdin.as_mut().unwrap().write_all(bam_bytes)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err("samtools view failed while decoding BAM stdin".into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn parse_sam_records(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let reader = BufReader::new(data);
    let mut pending: Option<LiteRec> = None;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('@') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 11 {
            continue;
        }
        let qname = fields[0].to_string();
        let flag: u16 = fields[1].parse().unwrap_or(0);
        let rname = fields[2].to_string();
        let pos: i64 = fields[3].parse().unwrap_or(0);
        let mapped = (flag & 0x4) == 0 && rname != "*";
        let strand = if (flag & 0x10) != 0 { '-' } else { '+' };

        let rec = LiteRec { qname, chrom: rname, pos, strand, mapped };
        pending = flush_or_set_pending(pending, rec);
    }
    if let Some(last) = pending {
        emit_unpaired(&last);
    }
    Ok(())
}

fn flush_or_set_pending(pending: Option<LiteRec>, current: LiteRec) -> Option<LiteRec> {
    match pending {
        None => Some(current),
        Some(prev) => {
            if prev.qname == current.qname {
                emit_pair(&prev, &current);
                None
            } else {
                emit_unpaired(&prev);
                Some(current)
            }
        }
    }
}

fn emit_unpaired(r: &LiteRec) {
    let (chrom1, pos1, strand1) = map_side(r);
    let pair_type = if r.mapped { "MU" } else { "NN" };
    println!("{}\t{}\t{}\t!\t0\t{}\t.\t{}", r.qname, chrom1, pos1, strand1, pair_type);
}

fn emit_pair(a: &LiteRec, b: &LiteRec) {
    let (chrom1, pos1, strand1) = map_side(a);
    let (chrom2, pos2, strand2) = map_side(b);
    let pair_type = match (a.mapped, b.mapped) {
        (true, true) => "UU",
        (true, false) => "UM",
        (false, true) => "MU",
        (false, false) => "NN",
    };
    println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", a.qname, chrom1, pos1, chrom2, pos2, strand1, strand2, pair_type);
}

fn map_side(r: &LiteRec) -> (String, i64, char) {
    if r.mapped {
        (r.chrom.clone(), r.pos, r.strand)
    } else {
        ("!".to_string(), 0, '.')
    }
}
