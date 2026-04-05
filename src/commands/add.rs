use chrono::Local;
use clap::Args;

use crate::config;
use crate::date_parser;
use crate::db;

#[derive(Args)]
pub struct AddArgs {
    /// Task title
    pub title: String,

    /// Project name
    #[arg(short, long)]
    pub project: Option<String>,

    /// Due date (YYYY-MM-DD, 今日, 明日, 来週, 月曜〜日曜, etc.)
    #[arg(short, long)]
    pub due: Option<String>,

    /// Remind date (YYYY-MM-DD, 今日, 明日, 来週, 月曜〜日曜, etc.)
    #[arg(short, long)]
    pub remind: Option<String>,
}

pub fn run(args: AddArgs) {
    if args.title.is_empty() {
        eprintln!("Error: title cannot be empty");
        std::process::exit(1);
    }

    let due = args.due.as_ref().map(|s| {
        date_parser::parse_fuzzy_date(s).unwrap_or_else(|| {
            eprintln!(
                "Error: invalid due date '{}'. Use: YYYY-MM-DD, 今日, 明日, 来週, 曜日名 etc.",
                s
            );
            std::process::exit(1);
        })
    });

    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let today = Local::now().date_naive();
    let id = match db::add_task(&conn, &args.title, args.project.as_deref(), due, today) {
        Ok(id) => id,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if let Some(ref remind_str) = args.remind {
        let remind_date = date_parser::parse_fuzzy_date(remind_str).unwrap_or_else(|| {
            eprintln!(
                "Error: invalid remind date '{}'. Use: YYYY-MM-DD, 今日, 明日, 来週, 曜日名 etc.",
                remind_str
            );
            std::process::exit(1);
        });
        if db::add_remind(&conn, id, remind_date).is_err() {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    }

    println!("Added: #{} {}", id, args.title);
}
