mod commands;
mod config;
mod date_parser;
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
    /// Close a task
    Close(commands::close::CloseArgs),
    /// Mark a task as done
    Done(commands::done::DoneArgs),
    /// Edit a task
    Edit(commands::edit::EditArgs),
    /// List tasks
    #[command(alias = "ls")]
    List(commands::list::ListArgs),
    /// Show overdue and due-soon tasks
    Notify(commands::notify::NotifyArgs),
    /// Show task details
    Show(commands::show::ShowArgs),
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Add(args) => commands::add::run(args),
        Commands::Close(args) => commands::close::run(args),
        Commands::Done(args) => commands::done::run(args),
        Commands::Edit(args) => commands::edit::run(args),
        Commands::List(args) => commands::list::run(args),
        Commands::Notify(args) => commands::notify::run(args),
        Commands::Show(args) => commands::show::run(args),
    }
}
