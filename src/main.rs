mod cli;
mod dedup;
mod flip;
mod merge;
mod parse;
mod select;
mod sort;
mod split;
mod stats;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    reject_unsupported_global_options(&cli)?;
    match cli.command {
        Commands::Parse(a) => parse::cmd_parse(a),
        Commands::Sort(a) => sort::cmd_sort(a),
        Commands::Select(a) => select::cmd_select(a),
        Commands::Merge(a) => merge::cmd_merge(a),
        Commands::Dedup(a) => dedup::cmd_dedup(a),
        Commands::Stats(a) => stats::cmd_stats(a),
        Commands::Split(a) => split::cmd_split(a),
        Commands::Parse2(_) => unsupported_command("parse2"),
        Commands::Flip(a) => flip::cmd_flip(a),
        Commands::Restrict(_) => unsupported_command("restrict"),
        Commands::Filterbycov(_) => unsupported_command("filterbycov"),
        Commands::Phase(_) => unsupported_command("phase"),
        Commands::Markasdup(_) => unsupported_command("markasdup"),
        Commands::Sample(_) => unsupported_command("sample"),
        Commands::Header(_) => unsupported_command("header"),
        Commands::Scaling(_) => unsupported_command("scaling"),
    }
}

fn reject_unsupported_global_options(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    if cli.post_mortem {
        return Err("not implemented: top-level --post-mortem".into());
    }
    if cli.output_profile.is_some() {
        return Err("not implemented: top-level --output-profile".into());
    }
    if cli.verbose > 0 {
        return Err("not implemented: top-level --verbose".into());
    }
    if cli.debug {
        return Err("not implemented: top-level --debug".into());
    }
    Ok(())
}

fn unsupported_command(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err(format!("not implemented: pairtools {name} compatibility is not implemented yet").into())
}
