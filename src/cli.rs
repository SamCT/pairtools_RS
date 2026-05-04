use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pairs-rs", version)]
pub struct Cli {
    #[arg(long = "post-mortem", action = ArgAction::SetTrue)]
    pub post_mortem: bool,
    #[arg(long = "output-profile")]
    pub output_profile: Option<PathBuf>,
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    pub verbose: u8,
    #[arg(short = 'd', long = "debug", action = ArgAction::SetTrue)]
    pub debug: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Parse(ParseArgs),
    Sort(SortArgs),
    Select(SelectArgs),
    Parse2(UnsupportedArgs),
    Dedup(UnsupportedArgs),
    Flip(UnsupportedArgs),
    Merge(UnsupportedArgs),
    Split(UnsupportedArgs),
    Stats(UnsupportedArgs),
    Restrict(UnsupportedArgs),
    Filterbycov(UnsupportedArgs),
    Phase(UnsupportedArgs),
    Markasdup(UnsupportedArgs),
    Sample(UnsupportedArgs),
    Header(UnsupportedArgs),
    Scaling(UnsupportedArgs),
}

#[derive(clap::Args, Clone)]
#[command(disable_help_flag = true)]
pub struct UnsupportedArgs {
    #[arg(num_args = 0.., trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(clap::Args, Clone)]
pub struct ParseArgs {
    #[arg(short = 'c', long = "chroms-path")]
    pub chroms_path: PathBuf,
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    #[arg(long = "assembly")]
    pub assembly: Option<String>,
    #[arg(long = "drop-sam", action = ArgAction::SetTrue)]
    pub drop_sam: bool,
    #[arg(long = "min-mapq", default_value_t = 1)]
    pub min_mapq: u8,
    #[arg(long = "max-molecule-size")]
    pub max_molecule_size: Option<u64>,
    #[arg(long = "drop-readid", action = ArgAction::SetTrue)]
    pub drop_readid: bool,
    #[arg(long = "drop-seq", action = ArgAction::SetTrue)]
    pub drop_seq: bool,
    #[arg(long = "add-pair-index", action = ArgAction::SetTrue)]
    pub add_pair_index: bool,
    #[arg(long = "add-columns")]
    pub add_columns: Option<String>,
    #[arg(long = "output-parsed-alignments")]
    pub output_parsed_alignments: Option<PathBuf>,
    #[arg(long = "output-stats")]
    pub output_stats: Option<PathBuf>,
    #[arg(long = "walks-policy", default_value = "5unique")]
    pub walks_policy: String,
    #[arg(long = "report-alignment-end", default_value = "5")]
    pub report_alignment_end: String,
    #[arg(long = "max-inter-align-gap")]
    pub max_inter_align_gap: Option<u64>,
    #[arg(long = "readid-transform")]
    pub readid_transform: Option<String>,
    #[arg(long = "flip", action = ArgAction::SetTrue)]
    pub flip: bool,
    #[arg(long = "no-flip", action = ArgAction::SetTrue)]
    pub no_flip: bool,
    #[arg(long = "nproc-in")]
    pub nproc_in: Option<usize>,
    #[arg(long = "nproc-out")]
    pub nproc_out: Option<usize>,
    #[arg(long = "cmd-in")]
    pub cmd_in: Option<String>,
    #[arg(long = "cmd-out")]
    pub cmd_out: Option<String>,
    pub input: Option<PathBuf>,
}

#[derive(clap::Args, Clone)]
pub struct SortArgs {
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    #[arg(long = "tmpdir")]
    pub tmpdir: Option<PathBuf>,
    #[arg(long = "memory")]
    pub memory: Option<String>,
    #[arg(long = "c1")]
    pub c1: Option<String>,
    #[arg(long = "c2")]
    pub c2: Option<String>,
    #[arg(long = "p1")]
    pub p1: Option<String>,
    #[arg(long = "p2")]
    pub p2: Option<String>,
    #[arg(long = "pt")]
    pub pt: Option<String>,
    #[arg(long = "extra-col")]
    pub extra_col: Vec<String>,
    #[arg(long = "nproc")]
    pub nproc: Option<usize>,
    #[arg(long = "compress-program")]
    pub compress_program: Option<String>,
    #[arg(long = "nproc-in")]
    pub nproc_in: Option<usize>,
    #[arg(long = "nproc-out")]
    pub nproc_out: Option<usize>,
    #[arg(long = "cmd-in")]
    pub cmd_in: Option<String>,
    #[arg(long = "cmd-out")]
    pub cmd_out: Option<String>,
    pub input: Option<PathBuf>,
}

#[derive(clap::Args, Clone)]
pub struct SelectArgs {
    pub condition: String,
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    #[arg(long = "output-rest")]
    pub output_rest: Option<PathBuf>,
    #[arg(long = "chrom-subset")]
    pub chrom_subset: Option<String>,
    #[arg(long = "startup-code")]
    pub startup_code: Option<String>,
    #[arg(short = 't', long = "type-cast")]
    pub type_cast: Vec<String>,
    #[arg(short = 'r', long = "remove-columns")]
    pub remove_columns: Option<String>,
    #[arg(long = "nproc-in")]
    pub nproc_in: Option<usize>,
    #[arg(long = "nproc-out")]
    pub nproc_out: Option<usize>,
    #[arg(long = "cmd-in")]
    pub cmd_in: Option<String>,
    #[arg(long = "cmd-out")]
    pub cmd_out: Option<String>,
    pub input: Option<PathBuf>,
}
