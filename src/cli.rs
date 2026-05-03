use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pairs-rs")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Parse(ParseArgs),
    Sort(SortArgs),
    Parse2,
    Dedup,
    Flip,
    Merge,
    Split,
    Select,
    Stats,
    Restrict,
    Filterbycov,
    Phase,
    Markasdup,
}

#[derive(clap::Args, Clone)]
pub struct ParseArgs {
    #[arg(short = 'c', long = "chroms-path")]
    pub chroms_path: PathBuf,
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    #[arg(long="drop-sam", action=ArgAction::SetTrue)]
    pub drop_sam: bool,
    #[arg(long = "min-mapq", default_value_t = 1)]
    pub min_mapq: u8,
    #[arg(long = "walks-policy", default_value = "5unique")]
    pub walks_policy: String,
    #[arg(long = "report-alignment-end", default_value = "5")]
    pub report_alignment_end: String,
    #[arg(long = "output-stats")]
    pub output_stats: Option<PathBuf>,
    pub input: Option<PathBuf>,
}

#[derive(clap::Args, Clone)]
pub struct SortArgs {
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    #[arg(long = "tmpdir", default_value = "/tmp")]
    pub tmpdir: PathBuf,
    #[arg(long = "memory", default_value_t = 10000)]
    pub max_lines: usize,
    #[arg(long = "c1", default_value = "chrom1")]
    pub c1: String,
    #[arg(long = "c2", default_value = "chrom2")]
    pub c2: String,
    #[arg(long = "p1", default_value = "pos1")]
    pub p1: String,
    #[arg(long = "p2", default_value = "pos2")]
    pub p2: String,
    #[arg(long = "pt", default_value = "pair_type")]
    pub pt: String,
    #[arg(long = "extra-col")]
    pub extra_col: Vec<String>,
    #[arg(long = "nproc", default_value_t = 1)]
    pub nproc: usize,
    #[arg(long = "compress-program")]
    pub compress_program: Option<String>,
    #[arg(long = "cmd-in")]
    pub cmd_in: Option<String>,
    #[arg(long = "cmd-out")]
    pub cmd_out: Option<String>,
    pub input: Option<PathBuf>,
}
