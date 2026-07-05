use clap::Args;

use crate::commands::json_output::{print_json, TaskJson};
use crate::config;
use crate::db;

#[derive(Args)]
pub struct ShowArgs {
    /// Task ID to show
    pub id: u32,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: ShowArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
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

    let mut task = task;
    task.reminds = match db::get_reminds_for_task(&conn, task.id) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };
    let reminds = &task.reminds;

    if args.json {
        // Emit the same single-task JSON schema as `list`/`search` (pretty-printed,
        // `reminds` plural, `done_at`/`source` fields), just as a single object.
        print_json(&TaskJson::from(&task));
    } else {
        println!("ID: {}", task.id);
        println!("Title: {}", task.title);
        println!("Status: {}", task.status.as_str());
        println!("Project: {}", task.project.as_deref().unwrap_or("(none)"));
        println!(
            "Due: {}",
            task.due
                .map(|d| d.to_string())
                .unwrap_or_else(|| "(none)".to_string())
        );
        if reminds.is_empty() {
            println!("Remind: (none)");
        } else {
            let remind_strs: Vec<String> = reminds.iter().map(|d| d.to_string()).collect();
            println!("Remind: {}", remind_strs.join(", "));
        }
        println!("Important: {}", if task.important { "yes" } else { "no" });
        println!("Created: {}", task.created.format("%Y-%m-%d %H:%M:%S"));
        println!("Updated: {}", task.updated.format("%Y-%m-%d %H:%M:%S"));
    }
}
