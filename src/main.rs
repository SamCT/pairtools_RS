mod cli;
mod parse;
mod sort;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Parse(a) => parse::cmd_parse(a),
        Commands::Sort(a) => sort::cmd_sort(a),
        Commands::Parse2
        | Commands::Dedup
        | Commands::Flip
        | Commands::Merge
        | Commands::Split
        | Commands::Select
        | Commands::Stats
        | Commands::Restrict
        | Commands::Filterbycov
        | Commands::Phase
        | Commands::Markasdup => Err("not implemented".into()),
    }
}
