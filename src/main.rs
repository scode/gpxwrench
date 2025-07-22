mod commands;
mod gpxxml;

use clap::{Parser, Subcommand};
use commands::trim::trim_command;
use commands::trim_to_activity::trim_to_activity_command;
use std::error::Error;

#[derive(Parser)]
#[command(name = "gpxwrench", about = "A CLI tool for processing GPX files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Trim GPX track points using duration or timestamp ranges")]
    Trim {
        #[arg(help = "Range specification: DUR1,DUR2 (e.g. 5s,10s) or TS1,TS2 (e.g. 00:05,01:30)")]
        range: String,
    },
    #[command(about = "Trim GPX to detected activity period based on speed analysis")]
    TrimToActivity {
        #[arg(
            long,
            short,
            default_value = "1.0",
            help = "Minimum speed (m/s) to consider as activity"
        )]
        speed_threshold: f64,
        #[arg(
            long,
            short,
            default_value = "30",
            help = "Buffer time (seconds) to add before/after detected activity"
        )]
        buffer: u64,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::Trim { range } => trim_command(&range),
        Commands::TrimToActivity {
            speed_threshold,
            buffer,
        } => trim_to_activity_command(speed_threshold, buffer),
    }
}
