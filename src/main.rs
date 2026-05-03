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

#[derive(Debug, Clone)]
struct Config {
    print_header: bool,
    walks_policy: String,
    drop_readid: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = parse_args()?;

    if cfg.walks_policy != "5unique" {
        return Err(format!(
            "unsupported --walks-policy={} for parse-lite; use --walks-policy 5unique for parity",
            cfg.walks_policy
        )
        .into());
    }

    if cfg.print_header {
        if cfg.drop_readid {
            println!("## pairs format v1.0.0");
            println!("#columns: chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type");
        } else {
            println!("## pairs format v1.0.0");
            println!("#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type");
        }
    }

    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes)?;
    if bytes.starts_with(b"BAM\x01") {
        let sam = bam_to_sam(&bytes)?;
        parse_sam_records(sam.as_bytes(), &cfg)?;
    } else {
        parse_sam_records(&bytes, &cfg)?;
    }
    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn std::error::Error>> {
    let mut print_header = true;
    let mut walks_policy = String::from("5unique");
    let mut drop_readid = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-header" => print_header = false,
            "--drop-readid" => drop_readid = true,
            "--walks-policy" => {
                walks_policy = args
                    .next()
                    .ok_or("missing value after --walks-policy")?;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    Ok(Config {
        print_header,
        walks_policy,
        drop_readid,
    })
}

fn print_help() {
    eprintln!(
        "pairs-rs parse-lite\n\nUsage: pairs-rs [--no-header] [--drop-readid] [--walks-policy 5unique]\n\nThis parse-lite implementation currently supports only --walks-policy 5unique for pairtools parity tests."
    );
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

fn parse_sam_records(data: &[u8], cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
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

        let rec = LiteRec {
            qname,
            chrom: rname,
            pos,
            strand,
            mapped,
        };
        pending = flush_or_set_pending(pending, rec, cfg);
    }
    if let Some(last) = pending {
        emit_unpaired(&last, cfg);
    }
    Ok(())
}

fn flush_or_set_pending(pending: Option<LiteRec>, current: LiteRec, cfg: &Config) -> Option<LiteRec> {
    match pending {
        None => Some(current),
        Some(prev) => {
            if prev.qname == current.qname {
                emit_pair(&prev, &current, cfg);
                None
            } else {
                emit_unpaired(&prev, cfg);
                Some(current)
            }
        }
    }
}

fn emit_unpaired(r: &LiteRec, cfg: &Config) {
    let (chrom1, pos1, strand1) = map_side(r);
    let pair_type = if r.mapped { "MU" } else { "NN" };
    if cfg.drop_readid {
        println!("{}\t{}\t!\t0\t{}\t.\t{}", chrom1, pos1, strand1, pair_type);
    } else {
        println!("{}\t{}\t{}\t!\t0\t{}\t.\t{}", r.qname, chrom1, pos1, strand1, pair_type);
    }
}

fn emit_pair(a: &LiteRec, b: &LiteRec, cfg: &Config) {
    let (chrom1, pos1, strand1) = map_side(a);
    let (chrom2, pos2, strand2) = map_side(b);
    let pair_type = match (a.mapped, b.mapped) {
        (true, true) => "UU",
        (true, false) => "UM",
        (false, true) => "MU",
        (false, false) => "NN",
    };
    if cfg.drop_readid {
        println!("{}\t{}\t{}\t{}\t{}\t{}\t{}", chrom1, pos1, chrom2, pos2, strand1, strand2, pair_type);
    } else {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            a.qname, chrom1, pos1, chrom2, pos2, strand1, strand2, pair_type
        );
    }
}

fn map_side(r: &LiteRec) -> (String, i64, char) {
    if r.mapped {
        (r.chrom.clone(), r.pos, r.strand)
    } else {
        ("!".to_string(), 0, '.')
    }
}
