use crate::cli::ParseArgs;
use rust_htslib::bam::record::{Cigar, Record};
use rust_htslib::bam::{self, HeaderView, Read};
use rust_htslib::htslib;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::{ptr, slice};

const UNMAPPED_CHROM: &str = "!";
const UNMAPPED_POS: i64 = 0;
const UNMAPPED_STRAND: char = '-';
const SAM_SEP: char = '\x19';
const INTER_SAM_SEP: &str = "\x19NEXT_SAM\x19";
const DEFAULT_MAX_INTER_ALIGN_GAP: u64 = 20;
const DEFAULT_MAX_MOLECULE_SIZE: i64 = 750;
const SUPPORTED_ADD_COLUMNS: &[&str] = &["mapq", "pos5", "pos3", "cigar", "read_len"];

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WalksPolicy {
    Mask,
    FiveAny,
    FiveUnique,
    ThreeAny,
    ThreeUnique,
    All,
}

impl WalksPolicy {
    fn parse(value: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match value {
            "mask" => Ok(Self::Mask),
            "5any" => Ok(Self::FiveAny),
            "5unique" => Ok(Self::FiveUnique),
            "3any" => Ok(Self::ThreeAny),
            "3unique" => Ok(Self::ThreeUnique),
            "all" => Ok(Self::All),
            other => Err(format!("not implemented: pairtools parse --walks-policy {other}").into()),
        }
    }
}

#[derive(Clone, Debug)]
struct Aln {
    chrom: String,
    pos5: i64,
    pos3: i64,
    strand: char,
    mapq: u8,
    is_mapped: bool,
    is_unique: bool,
    kind: char,
    dist_to_5: u32,
    dist_to_3: u32,
    cigar: String,
    algn_read_span: u32,
    read_len: u32,
}

impl Aln {
    fn empty(kind: char) -> Self {
        Self {
            chrom: UNMAPPED_CHROM.to_string(),
            pos5: UNMAPPED_POS,
            pos3: UNMAPPED_POS,
            strand: UNMAPPED_STRAND,
            mapq: 0,
            is_mapped: false,
            is_unique: false,
            kind,
            dist_to_5: 0,
            dist_to_3: 0,
            cigar: "*".to_string(),
            algn_read_span: 0,
            read_len: 0,
        }
    }

    fn gap(dist_to_5: u32, span: u32, read_len: u32, next_dist_to_5: u32) -> Self {
        let mut aln = Self::empty('N');
        aln.dist_to_5 = dist_to_5;
        aln.algn_read_span = span;
        aln.read_len = read_len;
        aln.dist_to_3 = read_len.saturating_sub(next_dist_to_5);
        aln
    }

    fn reported_pos(&self, report_end: ReportEnd) -> i64 {
        match report_end {
            ReportEnd::Five => self.pos5,
            ReportEnd::Three => self.pos3,
        }
    }

    fn extra_value(&self, column: &str) -> String {
        match column {
            "mapq" => self.mapq.to_string(),
            "pos5" => self.pos5.to_string(),
            "pos3" => self.pos3.to_string(),
            "cigar" => self.cigar.clone(),
            "read_len" => self.read_len.to_string(),
            _ => String::new(),
        }
    }

    fn masked(&self) -> Self {
        let mut masked = self.clone();
        masked.chrom = UNMAPPED_CHROM.to_string();
        masked.pos5 = UNMAPPED_POS;
        masked.pos3 = UNMAPPED_POS;
        masked.strand = UNMAPPED_STRAND;
        masked.is_mapped = false;
        masked.is_unique = false;
        masked.kind = 'W';
        masked
    }

    fn opposite_walk_end(&self) -> Self {
        let mut opposite = self.clone();
        std::mem::swap(&mut opposite.pos5, &mut opposite.pos3);
        opposite.strand = match opposite.strand {
            '+' => '-',
            '-' => '+',
            other => other,
        };
        opposite
    }
}

#[derive(Clone)]
struct SamEntry {
    side: u8,
    query_alignment_start: u32,
    ordinal: usize,
    record: Record,
    sam: String,
}

struct Template {
    read_id: String,
    entries: Vec<SamEntry>,
}

impl Template {
    fn new(read_id: String) -> Self {
        Self {
            read_id,
            entries: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone)]
struct ParsedTemplate {
    read_id: String,
    alns1: Vec<Aln>,
    alns2: Vec<Aln>,
    sams1: Vec<String>,
    sams2: Vec<String>,
}

#[derive(Clone)]
struct Pair {
    left: Aln,
    right: Aln,
    sams_left: Vec<String>,
    sams_right: Vec<String>,
}

#[derive(Clone)]
struct ChromInfo {
    name: String,
    len: u64,
}

type ChromOrder = (Vec<ChromInfo>, HashMap<String, usize>);

struct EmitConfig<'a> {
    header: &'a HeaderView,
    order: &'a HashMap<String, usize>,
    report_end: ReportEnd,
    drop_sam: bool,
    add_columns: &'a [String],
    min_mapq: u8,
    max_inter_align_gap: u64,
    max_molecule_size: i64,
    walks_policy: WalksPolicy,
}

pub fn cmd_parse(args: ParseArgs) -> Result<(), Box<dyn std::error::Error>> {
    reject_unsupported_parse_options(&args)?;
    let walks_policy = WalksPolicy::parse(&args.walks_policy)?;
    let report_end = ReportEnd::parse(&args.report_alignment_end)?;
    let add_columns = parse_add_columns(args.add_columns.as_deref())?;
    reject_compressed_output(args.output.as_deref(), "compressed parse output")?;
    reject_compressed_output(
        args.output_stats.as_deref(),
        "compressed parse stats output",
    )?;

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

    write_pairs_header(
        out.as_mut(),
        &header,
        &chroms,
        args.assembly.as_deref(),
        args.drop_sam,
        &add_columns,
    )?;

    let max_inter_align_gap = args
        .max_inter_align_gap
        .unwrap_or(DEFAULT_MAX_INTER_ALIGN_GAP);
    let max_molecule_size = args
        .max_molecule_size
        .unwrap_or(DEFAULT_MAX_MOLECULE_SIZE as u64)
        .try_into()
        .map_err(|_| "pairtools parse --max-molecule-size is too large")?;
    let config = EmitConfig {
        header: &header,
        order: &order,
        report_end,
        drop_sam: args.drop_sam,
        add_columns: &add_columns,
        min_mapq: args.min_mapq,
        max_inter_align_gap,
        max_molecule_size,
        walks_policy,
    };
    let mut stats = StatsCounter::new();
    let mut current: Option<Template> = None;
    let mut emitted_read_ids = HashSet::new();

    for (ordinal, rec) in bam.records().enumerate() {
        let record = rec?;
        let qname = String::from_utf8_lossy(record.qname()).to_string();
        if current
            .as_ref()
            .is_some_and(|template| template.read_id != qname)
        {
            let template = current.take().expect("template exists");
            emitted_read_ids.insert(template.read_id.clone());
            emit_template(out.as_mut(), template, &config, &mut stats)?;
        }

        if current.is_none() {
            if emitted_read_ids.contains(&qname) {
                return Err(format!(
                    "not implemented: pairs-rs parse requires query-name grouped input; read {qname} appears in non-adjacent records"
                )
                .into());
            }
            current = Some(Template::new(qname));
        }

        let sam = record_to_sam_string(&record, &header)?;
        let entry = SamEntry {
            side: if record.flags() & 0x40 != 0 { 1 } else { 2 },
            query_alignment_start: query_alignment_start(&record),
            ordinal,
            record,
            sam,
        };
        current
            .as_mut()
            .expect("template exists")
            .entries
            .push(entry);
    }

    if let Some(template) = current {
        emit_template(out.as_mut(), template, &config, &mut stats)?;
    }

    if let Some(path) = args.output_stats {
        let mut stats_out = BufWriter::new(File::create(path)?);
        stats.write(&mut stats_out)?;
    }

    Ok(())
}

fn reject_unsupported_parse_options(args: &ParseArgs) -> Result<(), Box<dyn std::error::Error>> {
    if args.drop_readid {
        return Err("not implemented: pairtools parse --drop-readid".into());
    }
    if args.drop_seq {
        return Err("not implemented: pairtools parse --drop-seq".into());
    }
    if args.add_pair_index {
        return Err("not implemented: pairtools parse --add-pair-index".into());
    }
    if args.output_parsed_alignments.is_some() {
        return Err("not implemented: pairtools parse --output-parsed-alignments".into());
    }
    if args.readid_transform.is_some() {
        return Err("not implemented: pairtools parse --readid-transform".into());
    }
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

fn parse_add_columns(value: Option<&str>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let mut columns = Vec::new();
    for column in value.split(',') {
        if !SUPPORTED_ADD_COLUMNS.contains(&column) {
            return Err(format!("not implemented: pairtools parse --add-columns {column}").into());
        }
        columns.push(column.to_string());
    }
    Ok(columns)
}

fn reject_compressed_output(
    path: Option<&Path>,
    feature: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = path {
        let path = path.to_string_lossy();
        if path.ends_with(".gz") || path.ends_with(".lz4") {
            return Err(format!("not implemented: {feature}").into());
        }
    }
    Ok(())
}

fn write_pairs_header(
    out: &mut dyn Write,
    header: &HeaderView,
    chroms: &[ChromInfo],
    assembly: Option<&str>,
    drop_sam: bool,
    add_columns: &[String],
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

    let mut columns = vec![
        "readID".to_string(),
        "chrom1".to_string(),
        "pos1".to_string(),
        "chrom2".to_string(),
        "pos2".to_string(),
        "strand1".to_string(),
        "strand2".to_string(),
        "pair_type".to_string(),
    ];
    if !drop_sam {
        columns.push("sam1".to_string());
        columns.push("sam2".to_string());
    }
    for column in add_columns {
        columns.push(format!("{column}1"));
        columns.push(format!("{column}2"));
    }
    writeln!(out, "#columns: {}", columns.join(" "))
}

fn emit_template(
    out: &mut dyn Write,
    template: Template,
    config: &EmitConfig<'_>,
    stats: &mut StatsCounter,
) -> Result<(), Box<dyn std::error::Error>> {
    if template.is_empty() {
        return Ok(());
    }

    let parsed = parse_template(
        template,
        config.header,
        config.min_mapq,
        config.max_inter_align_gap,
    )?;
    if config.walks_policy == WalksPolicy::All {
        for pair in emit_all_walk_pairs(&parsed) {
            emit_pair(out, &parsed.read_id, pair, config, stats)?;
        }
    } else {
        let pair = select_pair(&parsed, config.walks_policy, config.max_molecule_size);
        emit_pair(out, &parsed.read_id, pair, config, stats)?;
    }
    Ok(())
}

fn parse_template(
    mut template: Template,
    header: &HeaderView,
    min_mapq: u8,
    max_inter_align_gap: u64,
) -> Result<ParsedTemplate, Box<dyn std::error::Error>> {
    template
        .entries
        .sort_by_key(|entry| (entry.side == 2, entry.query_alignment_start, entry.ordinal));

    let mut sams1 = Vec::new();
    let mut sams2 = Vec::new();
    let mut alns1 = Vec::new();
    let mut alns2 = Vec::new();
    for entry in &template.entries {
        let aln = parse_record(&entry.record, header, min_mapq)?;
        if entry.side == 1 {
            sams1.push(entry.sam.clone());
            alns1.push(aln);
        } else {
            sams2.push(entry.sam.clone());
            alns2.push(aln);
        }
    }

    if alns1.is_empty() || alns2.is_empty() {
        return Ok(ParsedTemplate {
            read_id: template.read_id,
            alns1: vec![Aln::empty('X')],
            alns2: vec![Aln::empty('X')],
            sams1,
            sams2,
        });
    }

    normalize_alignment_list(&mut alns1, max_inter_align_gap);
    normalize_alignment_list(&mut alns2, max_inter_align_gap);

    Ok(ParsedTemplate {
        read_id: template.read_id,
        alns1,
        alns2,
        sams1,
        sams2,
    })
}

fn parse_record(
    record: &Record,
    header: &HeaderView,
    min_mapq: u8,
) -> Result<Aln, Box<dyn std::error::Error>> {
    let mapped = !record.is_unmapped();
    let unique = mapped && record.mapq() >= min_mapq;
    let cigar = cigar_metrics(record);
    let reverse = record.is_reverse();
    let (strand, dist_to_5, dist_to_3) = if reverse {
        ('-', cigar.clip3_ref, cigar.clip5_ref)
    } else {
        ('+', cigar.clip5_ref, cigar.clip3_ref)
    };

    if mapped && unique {
        let tid = record.tid();
        if tid < 0 {
            return Err("mapped record is missing a reference id".into());
        }
        let chrom = String::from_utf8_lossy(header.tid2name(tid as u32)).to_string();
        let left = record.pos() + 1;
        let right = left + i64::from(cigar.algn_ref_span) - 1;
        let (pos5, pos3) = if reverse {
            (right, left)
        } else {
            (left, right)
        };
        Ok(Aln {
            chrom,
            pos5,
            pos3,
            strand,
            mapq: record.mapq(),
            is_mapped: true,
            is_unique: true,
            kind: 'U',
            dist_to_5,
            dist_to_3,
            cigar: cigar.cigar,
            algn_read_span: cigar.algn_read_span,
            read_len: cigar.read_len,
        })
    } else {
        Ok(Aln {
            chrom: UNMAPPED_CHROM.to_string(),
            pos5: UNMAPPED_POS,
            pos3: UNMAPPED_POS,
            strand: UNMAPPED_STRAND,
            mapq: record.mapq(),
            is_mapped: mapped,
            is_unique: false,
            kind: if mapped { 'M' } else { 'N' },
            dist_to_5: if mapped { dist_to_5 } else { 0 },
            dist_to_3: if mapped { dist_to_3 } else { 0 },
            cigar: cigar.cigar,
            algn_read_span: cigar.algn_read_span,
            read_len: cigar.read_len,
        })
    }
}

struct CigarMetrics {
    cigar: String,
    algn_ref_span: u32,
    algn_read_span: u32,
    read_len: u32,
    matched_bp: u32,
    clip5_ref: u32,
    clip3_ref: u32,
}

fn cigar_metrics(record: &Record) -> CigarMetrics {
    let mut metrics = CigarMetrics {
        cigar: if record.cigar().is_empty() {
            "None".to_string()
        } else {
            record.cigar().to_string()
        },
        algn_ref_span: 0,
        algn_read_span: 0,
        read_len: 0,
        matched_bp: 0,
        clip5_ref: 0,
        clip3_ref: 0,
    };

    for op in record.cigar().iter() {
        match *op {
            Cigar::Match(len) | Cigar::Equal(len) | Cigar::Diff(len) => {
                metrics.matched_bp += len;
                metrics.algn_ref_span += len;
                metrics.algn_read_span += len;
                metrics.read_len += len;
            }
            Cigar::Ins(len) => {
                metrics.algn_read_span += len;
                metrics.read_len += len;
            }
            Cigar::Del(len) | Cigar::RefSkip(len) => {
                metrics.algn_ref_span += len;
            }
            Cigar::SoftClip(len) | Cigar::HardClip(len) => {
                metrics.read_len += len;
                if metrics.matched_bp == 0 {
                    metrics.clip5_ref = len;
                } else {
                    metrics.clip3_ref = len;
                }
            }
            Cigar::Pad(_) => {}
        }
    }
    metrics
}

fn normalize_alignment_list(alns: &mut Vec<Aln>, max_inter_align_gap: u64) {
    if alns.is_empty() {
        alns.push(Aln::empty('N'));
        return;
    }
    alns.sort_by_key(|aln| aln.dist_to_5);

    if alns.len() == 1 && !alns[0].is_mapped {
        return;
    }

    let mut normalized = Vec::with_capacity(alns.len());
    let mut last_5_pos = 0u32;
    for aln in alns.drain(..) {
        let gap = aln.dist_to_5.saturating_sub(last_5_pos);
        if u64::from(gap) > max_inter_align_gap {
            normalized.push(Aln::gap(last_5_pos, gap, aln.read_len, aln.dist_to_5));
        }
        last_5_pos = last_5_pos.max(aln.dist_to_5.saturating_add(aln.algn_read_span));
        normalized.push(aln);
    }
    *alns = normalized;
}

fn select_pair(parsed: &ParsedTemplate, policy: WalksPolicy, max_molecule_size: i64) -> Pair {
    let mut alns1 = parsed.alns1.clone();
    let mut alns2 = parsed.alns2.clone();

    let mut left = alns1[0].clone();
    let mut right = alns2[0].clone();

    if alns1.len() > 1 || alns2.len() > 1 {
        let rescued_linear_side = rescue_walk(&mut alns1, &mut alns2, max_molecule_size);
        if rescued_linear_side.is_some() {
            left = alns1[0].clone();
            right = alns2[0].clone();
        } else {
            (left, right) = apply_walks_policy(&alns1, &alns2, policy);
        }
    }

    Pair {
        left,
        right,
        sams_left: parsed.sams1.clone(),
        sams_right: parsed.sams2.clone(),
    }
}

fn apply_walks_policy(alns1: &[Aln], alns2: &[Aln], policy: WalksPolicy) -> (Aln, Aln) {
    match policy {
        WalksPolicy::Mask => (select_5any(alns1).masked(), select_5any(alns2).masked()),
        WalksPolicy::FiveAny => (select_5any(alns1), select_5any(alns2)),
        WalksPolicy::FiveUnique => (select_5unique(alns1), select_5unique(alns2)),
        WalksPolicy::ThreeAny => (select_3any(alns1), select_3any(alns2)),
        WalksPolicy::ThreeUnique => (select_3unique(alns1), select_3unique(alns2)),
        WalksPolicy::All => unreachable!("all policy emits multiple walk pairs"),
    }
}

fn emit_all_walk_pairs(parsed: &ParsedTemplate) -> Vec<Pair> {
    let mut pairs = Vec::new();

    for window in parsed.alns1.windows(2) {
        let (sams_left, sams_right) = if parsed.alns2.len() == 1 && window[0].kind != 'U' {
            (parsed.sams2.clone(), parsed.sams1.clone())
        } else {
            (parsed.sams1.clone(), parsed.sams2.clone())
        };
        pairs.push(Pair {
            left: window[0].clone(),
            right: window[1].opposite_walk_end(),
            sams_left,
            sams_right,
        });
    }

    if let (Some(left), Some(right)) = (parsed.alns1.last(), parsed.alns2.last()) {
        let (sams_left, sams_right) =
            if parsed.alns2.len() == 1 && parsed.alns1.iter().any(|aln| aln.kind != 'U') {
                (parsed.sams2.clone(), parsed.sams1.clone())
            } else {
                (parsed.sams1.clone(), parsed.sams2.clone())
            };
        pairs.push(Pair {
            left: left.clone(),
            right: right.clone(),
            sams_left,
            sams_right,
        });
    }

    for window in parsed.alns2.windows(2) {
        let (sams_left, sams_right) = if parsed.alns1.len() == 1 {
            (parsed.sams1.clone(), parsed.sams2.clone())
        } else {
            (parsed.sams2.clone(), parsed.sams1.clone())
        };
        pairs.push(Pair {
            left: window[0].clone(),
            right: window[1].opposite_walk_end(),
            sams_left,
            sams_right,
        });
    }

    pairs
}

fn select_5any(alns: &[Aln]) -> Aln {
    alns[0].clone()
}

fn select_5unique(alns: &[Aln]) -> Aln {
    alns.iter()
        .find(|aln| aln.is_mapped && aln.is_unique)
        .unwrap_or(&alns[0])
        .clone()
}

fn select_3any(alns: &[Aln]) -> Aln {
    alns.last().unwrap_or(&alns[0]).clone()
}

fn select_3unique(alns: &[Aln]) -> Aln {
    alns.iter()
        .rev()
        .find(|aln| aln.is_mapped && aln.is_unique)
        .unwrap_or_else(|| alns.last().unwrap_or(&alns[0]))
        .clone()
}

fn rescue_walk(alns1: &mut [Aln], alns2: &mut [Aln], max_molecule_size: i64) -> Option<u8> {
    let n_algns1 = alns1.len();
    let n_algns2 = alns2.len();
    if n_algns1 <= 1 && n_algns2 <= 1 {
        return None;
    }
    if !((n_algns1 == 1 && n_algns2 == 2) || (n_algns1 == 2 && n_algns2 == 1)) {
        return None;
    }

    let first_read_is_chimeric = n_algns1 > 1;
    let (chim5_algn, chim3_algn, linear_algn) = if first_read_is_chimeric {
        (&alns1[0], &alns1[1], &alns2[0])
    } else {
        (&alns2[0], &alns2[1], &alns1[0])
    };

    if !(linear_algn.is_mapped && linear_algn.is_unique) {
        return None;
    }

    let mut can_rescue = true;
    if chim3_algn.is_mapped && chim5_algn.is_unique {
        can_rescue &= chim3_algn.chrom == linear_algn.chrom;
        can_rescue &= chim3_algn.strand != linear_algn.strand;
        if linear_algn.strand == '+' {
            can_rescue &= linear_algn.pos5 < chim3_algn.pos5;
        } else {
            can_rescue &= linear_algn.pos5 > chim3_algn.pos5;
        }

        let molecule_size = if linear_algn.strand == '+' {
            chim3_algn.pos5 - linear_algn.pos5
                + i64::from(chim3_algn.dist_to_5)
                + i64::from(linear_algn.dist_to_5)
        } else {
            linear_algn.pos5 - chim3_algn.pos5
                + i64::from(chim3_algn.dist_to_5)
                + i64::from(linear_algn.dist_to_5)
        };
        can_rescue &= molecule_size <= max_molecule_size;
    }

    if can_rescue {
        if first_read_is_chimeric {
            alns1[1].kind = 'X';
            alns2[0].kind = 'R';
            Some(1)
        } else {
            alns1[0].kind = 'R';
            alns2[1].kind = 'X';
            Some(2)
        }
    } else {
        None
    }
}

fn emit_pair(
    out: &mut dyn Write,
    read_id: &str,
    pair: Pair,
    config: &EmitConfig<'_>,
    stats: &mut StatsCounter,
) -> io::Result<()> {
    let (x, y, sams_x, sams_y) =
        if should_flip(&pair.left, &pair.right, config.order, config.report_end) {
            (pair.right, pair.left, pair.sams_right, pair.sams_left)
        } else {
            (pair.left, pair.right, pair.sams_left, pair.sams_right)
        };
    let pair_type = format!("{}{}", x.kind, y.kind);

    let mut columns = vec![
        read_id.to_string(),
        x.chrom.clone(),
        x.reported_pos(config.report_end).to_string(),
        y.chrom.clone(),
        y.reported_pos(config.report_end).to_string(),
        x.strand.to_string(),
        y.strand.to_string(),
        pair_type.clone(),
    ];

    if !config.drop_sam {
        columns.push(format_sam_column(&sams_x, &pair_type));
        columns.push(format_sam_column(&sams_y, &pair_type));
    }

    for column in config.add_columns {
        columns.push(x.extra_value(column));
        columns.push(y.extra_value(column));
    }

    stats.add_pair(&x, &y, config.report_end, &pair_type);
    writeln!(out, "{}", columns.join("\t"))
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

fn format_sam_column(sams: &[String], pair_type: &str) -> String {
    sams.iter()
        .map(|sam| {
            let escaped = sam.replace('\t', &SAM_SEP.to_string());
            format!("{escaped}{SAM_SEP}Yt:Z:{pair_type}")
        })
        .collect::<Vec<_>>()
        .join(INTER_SAM_SEP)
}

fn query_alignment_start(record: &Record) -> u32 {
    for op in record.cigar().iter() {
        match *op {
            Cigar::SoftClip(len) => return len,
            Cigar::HardClip(_) => continue,
            _ => return 0,
        }
    }
    0
}

fn record_to_sam_string(
    record: &Record,
    header: &HeaderView,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut sam = htslib::kstring_t {
        l: 0,
        m: 0,
        s: ptr::null_mut(),
    };
    let status = unsafe { htslib::sam_format1(header.inner_ptr(), record.inner(), &mut sam) };
    if status < 0 {
        return Err("failed to format BAM record as SAM".into());
    }
    let bytes = unsafe { slice::from_raw_parts(sam.s as *const u8, sam.l) };
    let out = String::from_utf8_lossy(bytes).to_string();
    unsafe {
        htslib::free(sam.s as *mut std::os::raw::c_void);
    }
    Ok(out)
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

struct StatsCounter {
    total: u64,
    total_unmapped: u64,
    total_single_sided_mapped: u64,
    total_mapped: u64,
    total_dups: u64,
    total_nodups: u64,
    cis: u64,
    trans: u64,
    pair_types: BTreeMap<String, u64>,
    pair_type_order: Vec<String>,
    cis_1kb: u64,
    cis_2kb: u64,
    cis_4kb: u64,
    cis_10kb: u64,
    cis_20kb: u64,
    cis_40kb: u64,
    chrom_freq: BTreeMap<(String, String), u64>,
    chrom_freq_order: Vec<(String, String)>,
    dist_bins: Vec<i64>,
    dist_freq: HashMap<String, Vec<u64>>,
}

impl StatsCounter {
    fn new() -> Self {
        let dist_bins = dist_bins();
        let mut dist_freq = HashMap::new();
        for strands in ["+-", "-+", "--", "++"] {
            dist_freq.insert(strands.to_string(), vec![0; dist_bins.len()]);
        }
        Self {
            total: 0,
            total_unmapped: 0,
            total_single_sided_mapped: 0,
            total_mapped: 0,
            total_dups: 0,
            total_nodups: 0,
            cis: 0,
            trans: 0,
            pair_types: BTreeMap::new(),
            pair_type_order: Vec::new(),
            cis_1kb: 0,
            cis_2kb: 0,
            cis_4kb: 0,
            cis_10kb: 0,
            cis_20kb: 0,
            cis_40kb: 0,
            chrom_freq: BTreeMap::new(),
            chrom_freq_order: Vec::new(),
            dist_bins,
            dist_freq,
        }
    }

    fn add_pair(&mut self, a: &Aln, b: &Aln, report_end: ReportEnd, pair_type: &str) {
        self.total += 1;
        if !self.pair_types.contains_key(pair_type) {
            self.pair_type_order.push(pair_type.to_string());
        }
        *self.pair_types.entry(pair_type.to_string()).or_insert(0) += 1;

        if a.chrom == UNMAPPED_CHROM && b.chrom == UNMAPPED_CHROM {
            self.total_unmapped += 1;
            return;
        }

        if a.chrom != UNMAPPED_CHROM && b.chrom != UNMAPPED_CHROM {
            self.total_mapped += 1;
            if pair_type == "DD" {
                self.total_dups += 1;
                return;
            }

            self.total_nodups += 1;
            let chrom_key = (a.chrom.clone(), b.chrom.clone());
            if !self.chrom_freq.contains_key(&chrom_key) {
                self.chrom_freq_order.push(chrom_key.clone());
            }
            *self.chrom_freq.entry(chrom_key).or_insert(0) += 1;

            if a.chrom == b.chrom {
                self.cis += 1;
                let dist = (b.reported_pos(report_end) - a.reported_pos(report_end)).abs();
                let bin_idx = self.dist_bin_idx(dist);
                let strands = format!("{}{}", a.strand, b.strand);
                if let Some(freqs) = self.dist_freq.get_mut(&strands) {
                    freqs[bin_idx] += 1;
                }
                if dist >= 1_000 {
                    self.cis_1kb += 1;
                }
                if dist >= 2_000 {
                    self.cis_2kb += 1;
                }
                if dist >= 4_000 {
                    self.cis_4kb += 1;
                }
                if dist >= 10_000 {
                    self.cis_10kb += 1;
                }
                if dist >= 20_000 {
                    self.cis_20kb += 1;
                }
                if dist >= 40_000 {
                    self.cis_40kb += 1;
                }
            } else {
                self.trans += 1;
            }
        } else {
            self.total_single_sided_mapped += 1;
        }
    }

    fn dist_bin_idx(&self, dist: i64) -> usize {
        match self.dist_bins.binary_search(&dist) {
            Ok(idx) => idx,
            Err(0) => 0,
            Err(idx) => idx - 1,
        }
    }

    fn write(&self, out: &mut dyn Write) -> io::Result<()> {
        writeln!(out, "total\t{}", self.total)?;
        writeln!(out, "total_unmapped\t{}", self.total_unmapped)?;
        writeln!(
            out,
            "total_single_sided_mapped\t{}",
            self.total_single_sided_mapped
        )?;
        writeln!(out, "total_mapped\t{}", self.total_mapped)?;
        writeln!(out, "total_dups\t{}", self.total_dups)?;
        writeln!(out, "total_nodups\t{}", self.total_nodups)?;
        writeln!(out, "cis\t{}", self.cis)?;
        writeln!(out, "trans\t{}", self.trans)?;
        for pair_type in &self.pair_type_order {
            writeln!(
                out,
                "pair_types/{pair_type}\t{}",
                self.pair_types[pair_type]
            )?;
        }
        writeln!(out, "cis_1kb+\t{}", self.cis_1kb)?;
        writeln!(out, "cis_2kb+\t{}", self.cis_2kb)?;
        writeln!(out, "cis_4kb+\t{}", self.cis_4kb)?;
        writeln!(out, "cis_10kb+\t{}", self.cis_10kb)?;
        writeln!(out, "cis_20kb+\t{}", self.cis_20kb)?;
        writeln!(out, "cis_40kb+\t{}", self.cis_40kb)?;
        self.write_summary(out)?;
        for key in &self.chrom_freq_order {
            writeln!(
                out,
                "chrom_freq/{}/{}\t{}",
                key.0, key.1, self.chrom_freq[key]
            )?;
        }
        self.write_dist_freq(out)
    }

    fn write_summary(&self, out: &mut dyn Write) -> io::Result<()> {
        for (name, count) in [
            ("frac_cis", self.cis),
            ("frac_cis_1kb+", self.cis_1kb),
            ("frac_cis_2kb+", self.cis_2kb),
            ("frac_cis_4kb+", self.cis_4kb),
            ("frac_cis_10kb+", self.cis_10kb),
            ("frac_cis_20kb+", self.cis_20kb),
            ("frac_cis_40kb+", self.cis_40kb),
        ] {
            writeln!(
                out,
                "summary/{name}\t{}",
                format_ratio_or_zero(count, self.total_nodups)
            )?;
        }
        writeln!(
            out,
            "summary/frac_dups\t{}",
            format_ratio_or_zero(self.total_dups, self.total_mapped)
        )?;
        writeln!(
            out,
            "summary/complexity_naive\t{}",
            if self.total_mapped == 0 { "0" } else { "nan" }
        )?;
        self.write_convergence(out)
    }

    fn write_convergence(&self, out: &mut dyn Write) -> io::Result<()> {
        let all_strands = ["++", "--", "-+", "+-"];
        let mut idx_maxs = HashMap::new();
        for strands in all_strands {
            idx_maxs.insert(strands, 0usize);
            for idx in 0..self.dist_bins.len() {
                let avg = all_strands
                    .iter()
                    .map(|s| self.dist_freq[*s][idx] as f64)
                    .sum::<f64>()
                    / 4.0;
                let rel = if avg == 0.0 {
                    0.0
                } else {
                    ((self.dist_freq[strands][idx] as f64) - avg).abs() / avg
                };
                if rel > 0.05 {
                    idx_maxs.insert(strands, idx);
                }
            }
        }

        let mut convergence_idx = 0usize;
        let mut convergence_strands = "??";
        for strands in all_strands {
            let idx = idx_maxs[strands];
            if idx > convergence_idx {
                convergence_idx = idx;
                convergence_strands = strands;
            }
        }
        let convergence_dist = if convergence_idx + 1 < self.dist_bins.len() {
            self.dist_bins[convergence_idx + 1]
        } else {
            i64::MAX
        };

        writeln!(
            out,
            "summary/dist_freq_convergence/convergence_dist\t{}",
            if convergence_strands == "??" {
                0
            } else {
                convergence_dist
            }
        )?;
        writeln!(
            out,
            "summary/dist_freq_convergence/strands_w_max_convergence_dist\t{}",
            convergence_strands
        )?;
        writeln!(
            out,
            "summary/dist_freq_convergence/convergence_rel_diff_threshold\t0.05"
        )?;

        let mut below = HashMap::new();
        let mut above = HashMap::new();
        for strands in all_strands {
            let freqs = &self.dist_freq[strands];
            let below_sum: u64 = freqs.iter().take(convergence_idx + 1).sum();
            let above_sum: u64 = freqs.iter().skip(convergence_idx + 1).sum();
            below.insert(strands, below_sum);
            above.insert(strands, above_sum);
            writeln!(
                out,
                "summary/dist_freq_convergence/n_cis_pairs_below_convergence_dist/{strands}\t{below_sum}"
            )?;
        }
        let below_all: u64 = below.values().sum();
        let above_all: u64 = above.values().sum();
        writeln!(
            out,
            "summary/dist_freq_convergence/n_cis_pairs_below_convergence_dist_all_strands\t{below_all}"
        )?;
        writeln!(
            out,
            "summary/dist_freq_convergence/n_cis_pairs_above_convergence_dist_all_strands\t{above_all}"
        )?;

        for (norm_name, norm) in [
            ("cis", self.cis),
            ("total_mapped", self.total_mapped),
            ("total_nodups", self.total_nodups),
        ] {
            for strands in all_strands {
                writeln!(
                    out,
                    "summary/dist_freq_convergence/frac_{norm_name}_in_cis_below_convergence_dist/{strands}\t{}",
                    format_ratio_nan(below[strands], norm)
                )?;
            }
            writeln!(
                out,
                "summary/dist_freq_convergence/frac_{norm_name}_in_cis_below_convergence_dist_all_strands\t{}",
                format_ratio_nan(below_all, norm)
            )?;
            writeln!(
                out,
                "summary/dist_freq_convergence/frac_{norm_name}_in_cis_above_convergence_dist_all_strands\t{}",
                format_ratio_nan(above_all, norm)
            )?;
        }

        Ok(())
    }

    fn write_dist_freq(&self, out: &mut dyn Write) -> io::Result<()> {
        for idx in 0..self.dist_bins.len() {
            for strands in ["+-", "-+", "--", "++"] {
                if idx < self.dist_bins.len() - 1 {
                    writeln!(
                        out,
                        "dist_freq/{}-{}/{strands}\t{}",
                        self.dist_bins[idx],
                        self.dist_bins[idx + 1],
                        self.dist_freq[strands][idx]
                    )?;
                } else {
                    writeln!(
                        out,
                        "dist_freq/{}+/{strands}\t{}",
                        self.dist_bins[idx], self.dist_freq[strands][idx]
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn dist_bins() -> Vec<i64> {
    let mut bins = Vec::new();
    bins.push(0);
    for step in 0..=(9 * 8) {
        let value = 10f64.powf(step as f64 / 8.0).round() as i64;
        if bins.last().copied() != Some(value) {
            bins.push(value);
        }
    }
    bins
}

fn format_ratio_or_zero(numerator: u64, denominator: u64) -> String {
    if denominator == 0 {
        "0".to_string()
    } else {
        format_float(numerator as f64 / denominator as f64)
    }
}

fn format_ratio_nan(numerator: u64, denominator: u64) -> String {
    if denominator == 0 {
        "nan".to_string()
    } else {
        format_float(numerator as f64 / denominator as f64)
    }
}

fn format_float(value: f64) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}
