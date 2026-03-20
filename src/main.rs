mod analysis;
mod model;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(bin_name = "cargo")]
enum Cli {
    #[command(name = "consolidate")]
    Consolidate(Args),
}

#[derive(Parser)]
#[command(
    version,
    about = "Identify shared workspace dependencies and version mismatches"
)]
struct Args {
    /// Path to the workspace root directory
    #[arg(long, default_value = ".")]
    path: PathBuf,

    /// Only show version mismatches
    #[arg(long)]
    mismatches_only: bool,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,
}

fn run(args: Args) -> Result<model::WorkspaceReport> {
    let start_path = if args.path.is_absolute() {
        args.path.clone()
    } else {
        std::env::current_dir()?.join(&args.path)
    };

    let workspace_root = analysis::find_workspace_root(&start_path)?;
    analysis::analyze_workspace(&workspace_root)
}

fn main() -> ExitCode {
    let Cli::Consolidate(args) = Cli::parse();

    let no_color = args.no_color;
    let mismatches_only = args.mismatches_only;

    match run(args) {
        Ok(report) => {
            let config = output::OutputConfig::new(no_color, mismatches_only);
            output::print_report(&report, &config);

            if report.mismatch_count > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("Error: {e:#}");
            ExitCode::from(2)
        }
    }
}
