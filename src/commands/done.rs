use chrono::Local;
use clap::Args;

use crate::config;
use crate::db;
use crate::model::Status;

#[derive(Args)]
pub struct DoneArgs {
    /// Task ID to mark as done
    pub id: u32,
}

pub fn run(args: DoneArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let task = match db::find_task(&conn, args.id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            eprintln!("Error: task #{} not found", args.id);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if task.status == Status::Done {
        eprintln!("Error: task #{} is already done", args.id);
        std::process::exit(1);
    }

    let today = Local::now().date_naive();
    if db::complete_task(&conn, args.id, today).is_err() {
        eprintln!("Error: failed to write database: {}", db_path.display());
        std::process::exit(1);
    }

    println!("Done: #{} {}", task.id, task.title);
}
