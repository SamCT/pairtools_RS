use crate::cli::ParseArgs;
use rust_htslib::bam::record::{Cigar, Record};
use rust_htslib::bam::{self, HeaderView, Read};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

const UNMAPPED_CHROM: &str = "!";
const UNMAPPED_POS: i64 = 0;
const UNMAPPED_STRAND: char = '-';

#[derive(Clone, Copy, Eq, PartialEq)]
enum ReportEnd {
    Five,
    Three,
}

impl ReportEnd {
    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value {
            "5" => Ok(Self::Five),
            "3" => Ok(Self::Three),
            _ => Err("not implemented: --report-alignment-end must be 5 or 3".into()),
        }
    }
}

#[derive(Clone, Debug)]
struct Aln {
    read_id: String,
    chrom: String,
    pos5: i64,
    pos3: i64,
    strand: char,
    is_mapped: bool,
    is_unique: bool,
    kind: char,
    dist_to_5: u32,
    ordinal: usize,
}

impl Aln {
    fn empty(read_id: &str) -> Self {
        Self {
            read_id: read_id.to_string(),
            chrom: UNMAPPED_CHROM.to_string(),
            pos5: UNMAPPED_POS,
            pos3: UNMAPPED_POS,
            strand: UNMAPPED_STRAND,
            is_mapped: false,
            is_unique: false,
            kind: 'X',
            dist_to_5: 0,
            ordinal: usize::MAX,
        }
    }

    fn reported_pos(&self, report_end: ReportEnd) -> i64 {
        match report_end {
            ReportEnd::Five => self.pos5,
            ReportEnd::Three => self.pos3,
        }
    }
}

struct Template {
    read_id: String,
    read1: Vec<Aln>,
    read2: Vec<Aln>,
}

impl Template {
    fn new(read_id: String) -> Self {
        Self {
            read_id,
            read1: Vec::new(),
            read2: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.read1.is_empty() && self.read2.is_empty()
    }
}

#[derive(Clone)]
struct ChromInfo {
    name: String,
    len: u64,
}

type ChromOrder = (Vec<ChromInfo>, HashMap<String, usize>);

pub fn cmd_parse(args: ParseArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_parse_options(&args)?;
    if args.walks_policy != "5unique" {
        return Err("not implemented: only --walks-policy 5unique is supported".into());
    }
    let report_end = ReportEnd::parse(&args.report_alignment_end)?;
    if !args.drop_sam {
        return Err("not implemented: pairsam output requires SAM columns".into());
    }
    if let Some(output) = &args.output {
        let path = output.to_string_lossy();
        if path.ends_with(".gz") || path.ends_with(".lz4") {
            return Err("not implemented: compressed parse output".into());
        }
    }

    let mut bam = if let Some(p) = &args.input {
        bam::Reader::from_path(p)?
    } else {
        bam::Reader::from_stdin()?
    };
    let header = bam.header().to_owned();
    let (chroms, order) = read_chrom_order(&args.chroms_path, &header)?;

    let mut out: Box<dyn Write> = if let Some(p) = args.output {
        Box::new(BufWriter::new(File::create(p)?))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };

    write_pairs_header(out.as_mut(), &header, &chroms, args.assembly.as_deref())?;

    let mut current: Option<Template> = None;
    for (ordinal, rec) in bam.records().enumerate() {
        let record = rec?;
        let qname = String::from_utf8_lossy(record.qname()).to_string();
        if current
            .as_ref()
            .is_some_and(|template| template.read_id != qname)
        {
            let template = current.take().expect("template exists");
            emit_template(out.as_mut(), template, &order, report_end)?;
        }

        if current.is_none() {
            current = Some(Template::new(qname.clone()));
        }

        let aln = parse_record(&record, &header, &qname, args.min_mapq, ordinal)?;
        let template = current.as_mut().expect("template exists");
        if record.flags() & 0x40 != 0 {
            template.read1.push(aln);
        } else {
            template.read2.push(aln);
        }
    }

    if let Some(template) = current {
        emit_template(out.as_mut(), template, &order, report_end)?;
    }

    Ok(())
}

fn reject_unsupported_parse_options(args: &ParseArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.max_molecule_size.is_some() {
        return Err("not implemented: pairtools parse --max-molecule-size".into());
    }
    if args.drop_readid {
        return Err("not implemented: pairtools parse --drop-readid".into());
    }
    if args.drop_seq {
        return Err("not implemented: pairtools parse --drop-seq".into());
    }
    if args.add_pair_index {
        return Err("not implemented: pairtools parse --add-pair-index".into());
    }
    if args.add_columns.is_some() {
        return Err("not implemented: pairtools parse --add-columns".into());
    }
    if args.output_parsed_alignments.is_some() {
        return Err("not implemented: pairtools parse --output-parsed-alignments".into());
    }
    if args.output_stats.is_some() {
        return Err("not implemented: pairtools parse --output-stats".into());
    }
    if args.max_inter_align_gap.is_some() {
        return Err("not implemented: pairtools parse --max-inter-align-gap".into());
    }
    if args.readid_transform.is_some() {
        return Err("not implemented: pairtools parse --readid-transform".into());
    }
    let _explicit_flip = args.flip;
    if args.no_flip {
        return Err("not implemented: pairtools parse --no-flip".into());
    }
    if args.nproc_in.is_some() {
        return Err("not implemented: pairtools parse --nproc-in".into());
    }
    if args.nproc_out.is_some() {
        return Err("not implemented: pairtools parse --nproc-out".into());
    }
    if args.cmd_in.is_some() {
        return Err("not implemented: pairtools parse --cmd-in".into());
    }
    if args.cmd_out.is_some() {
        return Err("not implemented: pairtools parse --cmd-out".into());
    }
    Ok(())
}

fn write_pairs_header(
    out: &mut dyn Write,
    header: &HeaderView,
    chroms: &[ChromInfo],
    assembly: Option<&str>,
) -> io::Result<()> {
    writeln!(out, "## pairs format v1.0.0")?;
    writeln!(out, "#shape: upper triangle")?;
    writeln!(out, "#genome_assembly: {}", assembly.unwrap_or("unknown"))?;
    for chrom in chroms {
        writeln!(out, "#chromsize: {} {}", chrom.name, chrom.len)?;
    }
    let header_bytes = bam::Header::from_template(header).to_bytes();
    let sam_header = String::from_utf8_lossy(&header_bytes);
    for line in sam_header.lines().filter(|line| !line.is_empty()) {
        writeln!(out, "#samheader: {line}")?;
    }
    writeln!(
        out,
        "#columns: readID chrom1 pos1 chrom2 pos2 strand1 strand2 pair_type"
    )
}

fn parse_record(
    record: &Record,
    header: &HeaderView,
    read_id: &str,
    min_mapq: u8,
    ordinal: usize,
) -> Result<Aln, Box<dyn std::error::Error>> {
    let mapped = !record.is_unmapped();
    let unique = mapped && record.mapq() >= min_mapq;
    let cigar = cigar_metrics(record);
    let reverse = record.is_reverse();
    let strand = if reverse { '-' } else { '+' };
    let dist_to_5 = if reverse {
        cigar.clip3_ref
    } else {
        cigar.clip5_ref
    };

    if unique {
        let tid = record.tid();
        if tid < 0 {
            return Err("mapped record is missing a reference id".into());
        }
        let chrom = String::from_utf8_lossy(header.tid2name(tid as u32)).to_string();
        let left = record.pos() + 1;
        let right = left + i64::from(cigar.ref_span) - 1;
        let (pos5, pos3) = if reverse {
            (right, left)
        } else {
            (left, right)
        };
        Ok(Aln {
            read_id: read_id.to_string(),
            chrom,
            pos5,
            pos3,
            strand,
            is_mapped: true,
            is_unique: true,
            kind: 'U',
            dist_to_5,
            ordinal,
        })
    } else {
        Ok(Aln {
            read_id: read_id.to_string(),
            chrom: UNMAPPED_CHROM.to_string(),
            pos5: UNMAPPED_POS,
            pos3: UNMAPPED_POS,
            strand: UNMAPPED_STRAND,
            is_mapped: mapped,
            is_unique: false,
            kind: if mapped { 'M' } else { 'N' },
            dist_to_5,
            ordinal,
        })
    }
}

#[derive(Default)]
struct CigarMetrics {
    ref_span: u32,
    matched_bp: u32,
    clip5_ref: u32,
    clip3_ref: u32,
}

fn cigar_metrics(record: &Record) -> CigarMetrics {
    let mut metrics = CigarMetrics::default();
    for op in record.cigar().iter() {
        match *op {
            Cigar::Match(len) | Cigar::Equal(len) | Cigar::Diff(len) => {
                metrics.matched_bp += len;
                metrics.ref_span += len;
            }
            Cigar::Ins(_) | Cigar::Pad(_) => {}
            Cigar::Del(len) | Cigar::RefSkip(len) => {
                metrics.ref_span += len;
            }
            Cigar::SoftClip(len) | Cigar::HardClip(len) => {
                if metrics.matched_bp == 0 {
                    metrics.clip5_ref = len;
                } else {
                    metrics.clip3_ref = len;
                }
            }
        }
    }
    metrics
}

fn emit_template(
    out: &mut dyn Write,
    template: Template,
    order: &HashMap<String, usize>,
    report_end: ReportEnd,
) -> io::Result<()> {
    if template.is_empty() {
        return Ok(());
    }
    let left = select_5unique(&template.read1, &template.read_id);
    let right = select_5unique(&template.read2, &template.read_id);
    emit_pair(out, left, right, order, report_end)
}

fn select_5unique(alns: &[Aln], read_id: &str) -> Aln {
    if alns.is_empty() {
        return Aln::empty(read_id);
    }
    let mut sorted: Vec<&Aln> = alns.iter().collect();
    sorted.sort_by_key(|aln| (aln.dist_to_5, aln.ordinal));
    sorted
        .iter()
        .copied()
        .find(|aln| aln.is_mapped && aln.is_unique)
        .unwrap_or(sorted[0])
        .clone()
}

fn emit_pair(
    out: &mut dyn Write,
    a: Aln,
    b: Aln,
    order: &HashMap<String, usize>,
    report_end: ReportEnd,
) -> io::Result<()> {
    let (x, y) = if should_flip(&a, &b, order, report_end) {
        (b, a)
    } else {
        (a, b)
    };
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}{}",
        x.read_id,
        x.chrom,
        x.reported_pos(report_end),
        y.chrom,
        y.reported_pos(report_end),
        x.strand,
        y.strand,
        x.kind,
        y.kind
    )
}

fn should_flip(a: &Aln, b: &Aln, order: &HashMap<String, usize>, report_end: ReportEnd) -> bool {
    let mut correct_order = (a.is_mapped, a.is_unique) <= (b.is_mapped, b.is_unique);
    if a.chrom != UNMAPPED_CHROM && b.chrom != UNMAPPED_CHROM {
        let a_key = (
            *order.get(&a.chrom).unwrap_or(&usize::MAX),
            a.reported_pos(report_end),
        );
        let b_key = (
            *order.get(&b.chrom).unwrap_or(&usize::MAX),
            b.reported_pos(report_end),
        );
        correct_order = a_key <= b_key;
    }
    !correct_order
}

fn read_chrom_order(
    chroms_path: &Path,
    header: &HeaderView,
) -> Result<ChromOrder, Box<dyn std::error::Error>> {
    let mut sam_chroms = HashMap::new();
    for tid in 0..header.target_count() {
        let name = String::from_utf8_lossy(header.tid2name(tid)).to_string();
        let len = header.target_len(tid).unwrap_or(0);
        sam_chroms.insert(name, len);
    }

    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for line in BufReader::new(File::open(chroms_path)?).lines() {
        let line = line?;
        let Some(chrom) = line.split('\t').next().filter(|chrom| !chrom.is_empty()) else {
            continue;
        };
        let Some(&len) = sam_chroms.get(chrom) else {
            continue;
        };
        if seen.insert(chrom.to_string()) {
            ordered.push(ChromInfo {
                name: chrom.to_string(),
                len,
            });
        }
    }

    let mut remaining: Vec<_> = sam_chroms
        .iter()
        .filter(|(name, _)| !seen.contains(*name))
        .map(|(name, len)| ChromInfo {
            name: name.clone(),
            len: *len,
        })
        .collect();
    remaining.sort_by_key(|chrom| chrom.name.clone());
    ordered.extend(remaining);

    let mut order = HashMap::new();
    order.insert(UNMAPPED_CHROM.to_string(), 0);
    for (idx, chrom) in ordered.iter().enumerate() {
        order.insert(chrom.name.clone(), idx + 1);
    }

    Ok((ordered, order))
}
