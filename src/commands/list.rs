use chrono::{Datelike, Local};
use clap::Args;
use comfy_table::modifiers::UTF8_SOLID_INNER_BORDERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{CellAlignment, ContentArrangement, Table};

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

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_SOLID_INNER_BORDERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["ID", "Title", "Project", "Due", "Age"]);

    for task in &tasks {
        let id_cell = format!("#{}", task.id);

        let title_display = if task.status == Status::Done {
            format!("\u{2713} {}", task.title)
        } else {
            task.title.clone()
        };

        let project_display = task.project.as_deref().unwrap_or_default().to_string();

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

        table.add_row(vec![
            &id_cell,
            &title_display,
            &project_display,
            &due_display,
            &age_display,
        ]);
    }

    let id_col = table.column_mut(0).expect("id column");
    id_col.set_cell_alignment(CellAlignment::Right);

    let age_col = table.column_mut(4).expect("age column");
    age_col.set_cell_alignment(CellAlignment::Right);

    println!("{table}");

    println!();
    if args.all && done_count > 0 {
        println!("{} tasks ({} done)", tasks.len(), done_count);
    } else {
        println!("{} tasks", tasks.len());
    }
}
