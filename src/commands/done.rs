use chrono::Local;
use clap::Args;

use crate::config;
use crate::db;
use crate::model::Status;

#[derive(Args)]
pub struct DoneArgs {
    /// Task IDs to mark as done
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<u32>,
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

    let today = Local::now().date_naive();
    let mut has_error = false;

    for id in &args.ids {
        let task = match db::find_task(&conn, *id) {
            Ok(Some(t)) => t,
            Ok(None) => {
                eprintln!("Error: task #{} not found", id);
                has_error = true;
                continue;
            }
            Err(_) => {
                eprintln!("Error: failed to read database: {}", db_path.display());
                has_error = true;
                continue;
            }
        };

        if task.status == Status::Done {
            eprintln!("Error: task #{} is already done", id);
            has_error = true;
            continue;
        }

        if db::complete_task(&conn, *id, today).is_err() {
            eprintln!("Error: failed to write database: {}", db_path.display());
            has_error = true;
            continue;
        }

        println!("Done: #{} {}", task.id, task.title);
    }

    if has_error {
        std::process::exit(1);
    }
}
