use clap::Args;

use crate::commands::list;
use crate::config;
use crate::db;

#[derive(Args)]
pub struct SearchArgs {
    /// Search keyword
    pub keyword: String,

    /// Show all tasks including done
    #[arg(short, long)]
    pub all: bool,

    /// Filter by project name
    #[arg(short, long)]
    pub project: Option<String>,
}

pub fn run(args: SearchArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let tasks = match db::search_tasks(&conn, &args.keyword, args.all, args.project.as_deref()) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if tasks.is_empty() {
        println!("No tasks found for keyword: \"{}\"", args.keyword);
        return;
    }

    list::print_task_table(&tasks, args.all, &conn);
}
