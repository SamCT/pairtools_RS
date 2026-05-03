use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser, Subcommand};
use rust_htslib::bam::{self, Read};
use tempfile::NamedTempFile;

#[derive(Parser)]
#[command(name = "pairs-rs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Parse(ParseArgs),
    Sort(SortArgs),
    Parse2, Dedup, Flip, Merge, Split, Select, Stats, Restrict, Filterbycov, Phase, Markasdup,
}

#[derive(clap::Args)]
struct ParseArgs {
    #[arg(short='c', long="chroms-path")]
    chroms_path: PathBuf,
    #[arg(short='o', long)]
    output: Option<PathBuf>,
    #[arg(long="drop-sam", action=ArgAction::SetTrue)]
    drop_sam: bool,
    #[arg(long="min-mapq", default_value_t=1)]
    min_mapq: u8,
    #[arg(long="walks-policy", default_value="5unique")]
    walks_policy: String,
    #[arg(long="report-alignment-end", default_value="5prime")]
    report_alignment_end: String,
    #[arg(long="output-stats")]
    output_stats: Option<PathBuf>,
    input: Option<PathBuf>,
}

#[derive(clap::Args)]
struct SortArgs {
    #[arg(short='o', long)] output: Option<PathBuf>,
    #[arg(long="tmpdir", default_value="/tmp")] tmpdir: PathBuf,
    #[arg(long="max-lines", default_value_t=10000)] max_lines: usize,
    input: Option<PathBuf>,
}

#[derive(Clone)]
struct Aln { q: String, chr: String, pos: i64, strand: char, mapped: bool }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse(a) => cmd_parse(a),
        Commands::Sort(a) => cmd_sort(a),
        Commands::Parse2 | Commands::Dedup | Commands::Flip | Commands::Merge | Commands::Split |
        Commands::Select | Commands::Stats | Commands::Restrict | Commands::Filterbycov |
        Commands::Phase | Commands::Markasdup => Err("not implemented".into()),
    }
}

fn cmd_parse(args: ParseArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.walks_policy != "5unique" { return Err("not implemented".into()); }
    if args.report_alignment_end != "5prime" && args.report_alignment_end != "3prime" { return Err("not implemented".into()); }
    let order = read_chroms(&args.chroms_path)?;
    let mut out: Box<dyn Write> = if let Some(p) = args.output { Box::new(BufWriter::new(File::create(p)?)) } else { Box::new(BufWriter::new(io::stdout())) };
    writeln!(out, "## pairs format v1.0.0")?;
    writeln!(out, "#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type")?;

    let mut bam = if let Some(p) = args.input { bam::Reader::from_path(p)? } else { bam::Reader::from_stdin()? };
    let header = bam.header().to_owned();
    let mut pending: Option<Aln> = None;
    let mut total = 0usize;
    for rec in bam.records() {
        let r = rec?;
        total += 1;
        if r.is_secondary() || r.is_supplementary() { continue; }
        let q = String::from_utf8_lossy(r.qname()).to_string();
        let unmapped = r.is_unmapped();
        let mapped = !unmapped && r.mapq() >= args.min_mapq;
        let chr = if mapped {
            let tid = r.tid();
            String::from_utf8_lossy(header.tid2name(tid as u32)).to_string()
        } else { "!".to_string() };
        let pos5 = r.pos() + 1;
        let cigar_ref_len = r.cigar().end_pos() - r.pos();
        let pos = if mapped && args.report_alignment_end == "3prime" {
            if r.is_reverse() { pos5 } else { pos5 + cigar_ref_len - 1 }
        } else if mapped && r.is_reverse() {
            pos5 + cigar_ref_len - 1
        } else { pos5 };
        let aln = Aln { q, chr, pos, strand: if r.is_reverse() {'-'} else {'+'}, mapped };
        pending = match pending.take() {
            None => Some(aln),
            Some(p) if p.q == aln.q => { emit_pair(&mut out, p, aln, &order)?; None }
            Some(p) => { emit_unpaired(&mut out, p)?; Some(aln) }
        }
    }
    if let Some(p) = pending { emit_unpaired(&mut out, p)?; }
    if let Some(sp) = args.output_stats { std::fs::write(sp, format!("total\t{total}\n"))?; }
    let _ = args.drop_sam;
    Ok(())
}

fn emit_unpaired(out: &mut Box<dyn Write>, r: Aln) -> io::Result<()> {
    let (c,p,s)=side(&r);
    writeln!(out, "{}\t{}\t{}\t!\t0\t{}\t.\tMU", r.q,c,p,s)
}
fn emit_pair(out: &mut Box<dyn Write>, a: Aln, b: Aln, order: &HashMap<String, usize>) -> io::Result<()> {
    let (x,y)=if should_flip(&a,&b,order){(b,a)}else{(a,b)};
    let (c1,p1,s1)=side(&x); let(c2,p2,s2)=side(&y);
    let pt = match (x.mapped,y.mapped){(true,true)=>"UU",(true,false)=>"UM",(false,true)=>"MU",(false,false)=>"NN"};
    writeln!(out,"{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",x.q,c1,p1,c2,p2,s1,s2,pt)
}
fn side(r:&Aln)->(String,i64,char){ if r.mapped {(r.chr.clone(),r.pos,r.strand)} else {("!".into(),0,'.')} }
fn should_flip(a:&Aln,b:&Aln,o:&HashMap<String,usize>)->bool{ let oa=*o.get(&a.chr).unwrap_or(&usize::MAX); let ob=*o.get(&b.chr).unwrap_or(&usize::MAX); oa>ob || (oa==ob && (a.pos>b.pos || (a.pos==b.pos && a.strand>b.strand))) }
fn read_chroms(p:&Path)->Result<HashMap<String,usize>,Box<dyn std::error::Error>>{ let mut m=HashMap::new(); for (i,l) in BufReader::new(File::open(p)?).lines().enumerate(){ let s=l?; if let Some(c)=s.split('\t').next(){ if !c.is_empty(){m.insert(c.into(),i);} } } Ok(m)}

fn cmd_sort(args: SortArgs) -> Result<(), Box<dyn std::error::Error>> {
    let reader: Box<dyn BufRead> = if let Some(p)=args.input { Box::new(BufReader::new(File::open(p)?)) } else { Box::new(BufReader::new(io::stdin())) };
    let mut headers = Vec::new();
    let mut chunk = Vec::new();
    let mut files: Vec<NamedTempFile> = Vec::new();
    for l in reader.lines() {
        let s=l?;
        if s.starts_with('#') { headers.push(s); continue; }
        if s.is_empty() { continue; }
        chunk.push(s);
        if chunk.len() >= args.max_lines { spill_chunk(&mut chunk, &mut files, &args.tmpdir)?; }
    }
    if !chunk.is_empty() { spill_chunk(&mut chunk, &mut files, &args.tmpdir)?; }
    let mut all = Vec::new();
    for f in &files { for l in BufReader::new(File::open(f.path())?).lines(){ all.push(l?); } }
    all.sort_by(|a,b| cmp_rows(a,b));
    let mut out: Box<dyn Write> = if let Some(p)=args.output { Box::new(BufWriter::new(File::create(p)?)) } else { Box::new(BufWriter::new(io::stdout())) };
    for h in headers { writeln!(out,"{h}")?; }
    for r in all { writeln!(out,"{r}")?; }
    Ok(())
}
fn spill_chunk(chunk: &mut Vec<String>, files: &mut Vec<NamedTempFile>, tmp: &Path) -> Result<(), Box<dyn std::error::Error>> {
    chunk.sort_by(|a,b| cmp_rows(a,b));
    let mut f = NamedTempFile::new_in(tmp)?;
    for r in chunk.iter() { writeln!(f, "{r}")?; }
    chunk.clear();
    files.push(f);
    Ok(())
}
fn cmp_rows(a:&str,b:&str)->Ordering{ let fa:Vec<&str>=a.split('\t').collect(); let fb:Vec<&str>=b.split('\t').collect(); let(c1,p1,c2,p2,t1)=extract(&fa); let(d1,q1,d2,q2,t2)=extract(&fb); c1.cmp(d1).then(c2.cmp(d2)).then(p1.cmp(&q1)).then(p2.cmp(&q2)).then(t1.cmp(t2)) }
fn extract<'a>(f:&[&'a str])->(&'a str,i64,&'a str,i64,&'a str){ let o=if f.len()>7 {1}else{0}; (f.get(o).copied().unwrap_or(""), f.get(o+1).and_then(|x|x.parse().ok()).unwrap_or(0), f.get(o+2).copied().unwrap_or(""), f.get(o+3).and_then(|x|x.parse().ok()).unwrap_or(0), f.get(o+6).copied().unwrap_or("")) }
