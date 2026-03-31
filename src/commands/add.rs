use chrono::{Local, NaiveDate};
use clap::Args;

use crate::config;
use crate::db;

#[derive(Args)]
pub struct AddArgs {
    /// Task title
    pub title: String,

    /// Project name
    #[arg(short, long)]
    pub project: Option<String>,

    /// Due date (YYYY-MM-DD)
    #[arg(short, long)]
    pub due: Option<NaiveDate>,
}

pub fn run(args: AddArgs) {
    if args.title.is_empty() {
        eprintln!("Error: title cannot be empty");
        std::process::exit(1);
    }

    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let today = Local::now().date_naive();
    let id = match db::add_task(&conn, &args.title, args.project.as_deref(), args.due, today) {
        Ok(id) => id,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    println!("Added: #{} {}", id, args.title);
}
