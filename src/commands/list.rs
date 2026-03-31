use chrono::{Datelike, Local};
use clap::Args;

use crate::config;
use crate::db;
use crate::model::Status;

#[derive(Args)]
pub struct ListArgs {
    /// Show all tasks including done
    #[arg(short, long)]
    pub all: bool,

    /// Filter by project name
    #[arg(short = 'P', long)]
    pub project: Option<String>,
}

pub fn run(args: ListArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let tasks = match db::list_tasks(&conn, args.all, args.project.as_deref()) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if tasks.is_empty() {
        println!("No tasks. Add one with: my-task add \"task title\"");
        return;
    }

    let today = Local::now().date_naive();
    let done_count = tasks.iter().filter(|t| t.status == Status::Done).count();

    for task in &tasks {
        let title_display = if task.status == Status::Done {
            let truncated = truncate(&task.title, 28);
            format!("\u{2713} {}", truncated)
        } else {
            truncate(&task.title, 30)
        };

        let project_display = task
            .project
            .as_deref()
            .map(|p| truncate(p, 20))
            .unwrap_or_default();

        let due_display = task
            .due
            .map(|d| format!("\u{1f4c5} {}/{}", d.month(), d.day()))
            .unwrap_or_default();

        let age_display = if task.status == Status::Done {
            task.done_at
                .map(|d| format!("done {}/{}", d.month(), d.day()))
                .unwrap_or_default()
        } else {
            let days = (today - task.created).num_days();
            format!("{}d", days)
        };

        println!(
            " {:>3}  {:<30}  {:<20}  {:<8}  {:>8}",
            format!("#{}", task.id),
            title_display,
            project_display,
            due_display,
            age_display,
        );
    }

    println!();
    if args.all && done_count > 0 {
        println!("{} tasks ({} done)", tasks.len(), done_count);
    } else {
        println!("{} tasks", tasks.len());
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}
