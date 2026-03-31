mod commands;
mod config;
mod db;
mod model;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "my-task", version, about = "Simple task manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new task
    Add(commands::add::AddArgs),
    /// Mark a task as done
    Done(commands::done::DoneArgs),
    /// List tasks
    List(commands::list::ListArgs),
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Add(args) => commands::add::run(args),
        Commands::Done(args) => commands::done::run(args),
        Commands::List(args) => commands::list::run(args),
    }
}
