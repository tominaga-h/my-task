use chrono::{Datelike, Days, Local};
use clap::Args;

use crate::config;
use crate::db;

#[derive(Args)]
pub struct NotifyArgs {
    /// Include tasks due within N days from today (0 = today + overdue only)
    #[arg(short = 'D', long, default_value = "0")]
    pub days: u32,
}

pub fn run(args: NotifyArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to open database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let today = Local::now().date_naive();
    let target_date = today
        .checked_add_days(Days::new(args.days as u64))
        .unwrap_or(today);

    let tasks = match db::get_due_tasks(&conn, target_date) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if tasks.is_empty() {
        return;
    }

    println!("期限切れタスクがあります");
    for task in &tasks {
        let due = task.due.expect("due should exist for notified tasks");
        let due_label = format!("{}/{}", due.month(), due.day());
        let diff = (due - today).num_days();

        let detail = if diff < 0 {
            format!("\x1b[31m{}日超過\x1b[0m", -diff)
        } else if diff == 0 {
            "\x1b[33m今日\x1b[0m".to_string()
        } else {
            format!("あと{}日", diff)
        };

        println!(
            "  #{} {}（期限: {} - {}）",
            task.id, task.title, due_label, detail
        );
    }
}
