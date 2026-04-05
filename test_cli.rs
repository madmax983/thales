use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "thales-cli", version, about = "Agent trading toolkit CLI", after_help = "Note: Experimental 'Nova' commands (e.g., simulate-black-swan) require the `--features nova` flag to be compiled and visible.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Test,
}

fn main() {
    let cli = Cli::parse();
    println!("{:?}", cli);
}
