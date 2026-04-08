use chrono::{Datelike, Days, Local};
use clap::Args;
use comfy_table::modifiers::UTF8_SOLID_INNER_BORDERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use terminal_size::{terminal_size, Width};

use crate::config;
use crate::db;

#[derive(Args)]
pub struct NotifyArgs {
    /// Include tasks due within N days from today (0 = today + overdue only)
    #[arg(short = 'd', long, default_value = "0")]
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

    let due_tasks = match db::get_due_tasks(&conn, target_date) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let remind_tasks = match db::get_tasks_with_remind_today(&conn, today) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if due_tasks.is_empty() && remind_tasks.is_empty() {
        return;
    }

    let term_width = terminal_size().map(|(Width(w), _)| w).unwrap_or(80);

    if !due_tasks.is_empty() {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_SOLID_INNER_BORDERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_width(term_width);

        table.set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Project").add_attribute(Attribute::Bold),
            Cell::new("Title").add_attribute(Attribute::Bold),
            Cell::new("Due").add_attribute(Attribute::Bold),
            Cell::new("Status").add_attribute(Attribute::Bold),
        ]);

        for task in &due_tasks {
            let due = task.due.expect("due should exist for notified tasks");
            let due_label = format!("{}/{}", due.month(), due.day());
            let diff = (due - today).num_days();

            let (status_text, status_color) = if diff < 0 {
                (format!("{}日超過", -diff), Color::Red)
            } else if diff == 0 {
                ("今日".to_string(), Color::Yellow)
            } else {
                (format!("あと{}日", diff), Color::Green)
            };

            let id_cell = Cell::new(format!("#{}", task.id)).fg(Color::Cyan);
            let project_text = task.project.as_deref().unwrap_or_default();
            let project_cell = Cell::new(project_text);
            let title_cell = if diff < 0 {
                let cell = Cell::new(&task.title).fg(Color::Red);
                if task.important {
                    cell.add_attribute(Attribute::Bold)
                } else {
                    cell
                }
            } else if task.important {
                Cell::new(&task.title)
                    .fg(Color::Magenta)
                    .add_attribute(Attribute::Bold)
            } else {
                Cell::new(&task.title)
            };
            let due_cell = if diff < 0 {
                Cell::new(&due_label).fg(Color::Red)
            } else if diff == 0 {
                Cell::new(&due_label).fg(Color::Yellow)
            } else {
                Cell::new(&due_label).fg(Color::Green)
            };
            let status_cell = Cell::new(&status_text).fg(status_color);

            table.add_row(vec![
                id_cell,
                project_cell,
                title_cell,
                due_cell,
                status_cell,
            ]);
        }

        let id_col = table.column_mut(0).expect("id column");
        id_col.set_cell_alignment(CellAlignment::Right);

        println!("期限切れタスクがあります");
        println!("{table}");
        println!();
        println!("{} tasks", due_tasks.len());
    }

    if !remind_tasks.is_empty() {
        if !due_tasks.is_empty() {
            println!();
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_SOLID_INNER_BORDERS)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_width(term_width);

        table.set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Project").add_attribute(Attribute::Bold),
            Cell::new("Title").add_attribute(Attribute::Bold),
            Cell::new("Remind").add_attribute(Attribute::Bold),
        ]);

        for task in &remind_tasks {
            let remind_label = format!("{}/{}", today.month(), today.day());
            let id_cell = Cell::new(format!("#{}", task.id)).fg(Color::Cyan);
            let project_text = task.project.as_deref().unwrap_or_default();
            let project_cell = Cell::new(project_text);
            let title_cell = if task.important {
                Cell::new(&task.title)
                    .fg(Color::Magenta)
                    .add_attribute(Attribute::Bold)
            } else {
                Cell::new(&task.title)
            };
            let remind_cell = Cell::new(&remind_label).fg(Color::Yellow);

            table.add_row(vec![id_cell, project_cell, title_cell, remind_cell]);
        }

        let id_col = table.column_mut(0).expect("id column");
        id_col.set_cell_alignment(CellAlignment::Right);

        println!("リマインドタスクがあります");
        println!("{table}");
        println!();
        println!("{} tasks", remind_tasks.len());
    }
}
