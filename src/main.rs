mod cli;
mod parse;
mod sort;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    reject_unsupported_global_options(&cli)?;
    match cli.command {
        Commands::Parse(a) => parse::cmd_parse(a),
        Commands::Sort(a) => sort::cmd_sort(a),
        Commands::Parse2(_) => unsupported_command("parse2"),
        Commands::Dedup(_) => unsupported_command("dedup"),
        Commands::Flip(_) => unsupported_command("flip"),
        Commands::Merge(_) => unsupported_command("merge"),
        Commands::Split(_) => unsupported_command("split"),
        Commands::Select(_) => unsupported_command("select"),
        Commands::Stats(_) => unsupported_command("stats"),
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
