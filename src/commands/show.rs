use clap::Args;

use crate::config;
use crate::db;

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

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

    let reminds = match db::get_reminds_for_task(&conn, task.id) {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if args.json {
        let project = task
            .project
            .as_ref()
            .map(|p| format!("\"{}\"", escape_json(p)))
            .unwrap_or_else(|| "null".to_string());
        let due = task
            .due
            .map(|d| format!("\"{}\"", d))
            .unwrap_or_else(|| "null".to_string());
        let remind_strs: Vec<String> = reminds.iter().map(|d| format!("\"{}\"", d)).collect();
        println!(
            "{{\"id\":{},\"title\":\"{}\",\"status\":\"{}\",\"project\":{},\"due\":{},\"remind\":[{}],\"important\":{},\"created\":\"{}\",\"updated\":\"{}\"}}",
            task.id,
            escape_json(&task.title),
            task.status.as_str(),
            project,
            due,
            remind_strs.join(","),
            task.important,
            task.created,
            task.updated,
        );
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
        println!("Created: {}", task.created);
        println!("Updated: {}", task.updated);
    }
}
