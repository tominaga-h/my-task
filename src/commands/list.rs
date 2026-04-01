use chrono::{Datelike, Local};
use clap::Args;
use comfy_table::modifiers::UTF8_SOLID_INNER_BORDERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};

use crate::config;
use crate::db;
use crate::model::{SortKey, Status};

#[derive(Args)]
pub struct ListArgs {
    /// Show all tasks including done
    #[arg(short, long)]
    pub all: bool,

    /// Filter by project name
    #[arg(short = 'P', long)]
    pub project: Option<String>,

    /// Sort by: id, due, project, created
    #[arg(short, long, default_value = "id")]
    pub sort: String,
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

    let sort = match args.sort.as_str() {
        "id" => SortKey::Id,
        "due" => SortKey::Due,
        "project" => SortKey::Project,
        "created" | "age" => SortKey::Created,
        other => {
            eprintln!(
                "Error: unknown sort key '{}'. Use: id, due, project, created",
                other
            );
            std::process::exit(1);
        }
    };

    let tasks = match db::list_tasks(&conn, args.all, args.project.as_deref(), sort) {
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
    let done_count = tasks
        .iter()
        .filter(|t| t.status == Status::Done || t.status == Status::Closed)
        .count();

    let project_colors = build_project_color_map(&tasks);

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_SOLID_INNER_BORDERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Status").add_attribute(Attribute::Bold),
            Cell::new("Project").add_attribute(Attribute::Bold),
            Cell::new("Title").add_attribute(Attribute::Bold),
            Cell::new("Due").add_attribute(Attribute::Bold),
            Cell::new("Age").add_attribute(Attribute::Bold),
        ]);

    for task in &tasks {
        let is_done = task.status == Status::Done;
        let is_closed = task.status == Status::Closed;
        let is_inactive = is_done || is_closed;
        let is_overdue = !is_inactive && task.due.is_some_and(|d| d < today);
        let is_due_today = !is_inactive && task.due.is_some_and(|d| d == today);

        let id_text = format!("#{}", task.id);
        let project_text = task.project.as_deref().unwrap_or_default().to_string();
        let due_text = task
            .due
            .map(|d| format!("{}/{}", d.month(), d.day()))
            .unwrap_or_default();
        let age_text = if is_done {
            task.done_at
                .map(|d| format!("done {}/{}", d.month(), d.day()))
                .unwrap_or_default()
        } else if is_closed {
            "closed".to_string()
        } else {
            let days = (today - task.created).num_days();
            format!("{}d", days)
        };

        if is_done {
            let green = Color::Green;
            table.add_row(vec![
                Cell::new(id_text).fg(green),
                Cell::new("DONE").fg(green),
                Cell::new(&project_text).fg(project_colors
                    .get(project_text.as_str())
                    .copied()
                    .unwrap_or(Color::White)),
                Cell::new(&task.title).fg(green),
                Cell::new(due_text).fg(green),
                Cell::new(age_text).fg(green),
            ]);
        } else if is_closed {
            let grey = Color::DarkGrey;
            table.add_row(vec![
                Cell::new(id_text).fg(grey),
                Cell::new("CLOSED").fg(grey),
                Cell::new(project_text).fg(grey),
                Cell::new(&task.title).fg(grey),
                Cell::new(due_text).fg(grey),
                Cell::new(age_text).fg(grey),
            ]);
        } else {
            let title_cell = if is_overdue {
                Cell::new(&task.title).fg(Color::Red)
            } else {
                Cell::new(&task.title)
            };

            let due_cell = if is_overdue {
                Cell::new(due_text).fg(Color::Red)
            } else if is_due_today {
                Cell::new(due_text).fg(Color::Yellow)
            } else if task.due.is_some() {
                Cell::new(due_text).fg(Color::Green)
            } else {
                Cell::new(due_text)
            };

            let days = (today - task.created).num_days();
            let age_cell = if days > 30 {
                Cell::new(age_text).fg(Color::Red)
            } else if days > 7 {
                Cell::new(age_text).fg(Color::Yellow)
            } else {
                Cell::new(age_text)
            };

            table.add_row(vec![
                Cell::new(id_text).fg(Color::Cyan),
                Cell::new("OPEN").fg(Color::Blue),
                Cell::new(&project_text).fg(project_colors
                    .get(project_text.as_str())
                    .copied()
                    .unwrap_or(Color::White)),
                title_cell,
                due_cell,
                age_cell,
            ]);
        }
    }

    let id_col = table.column_mut(0).expect("id column");
    id_col.set_cell_alignment(CellAlignment::Right);

    let age_col = table.column_mut(5).expect("age column");
    age_col.set_cell_alignment(CellAlignment::Right);

    println!("{table}");

    println!();
    if args.all && done_count > 0 {
        println!("{} tasks ({} done)", tasks.len(), done_count);
    } else {
        println!("{} tasks", tasks.len());
    }
}

const PROJECT_PALETTE: &[Color] = &[
    Color::Rgb {
        r: 255,
        g: 107,
        b: 107,
    }, // coral red
    Color::Rgb {
        r: 255,
        g: 179,
        b: 71,
    }, // orange
    Color::Rgb {
        r: 255,
        g: 217,
        b: 61,
    }, // golden yellow
    Color::Rgb {
        r: 119,
        g: 221,
        b: 119,
    }, // pastel green
    Color::Rgb {
        r: 77,
        g: 208,
        b: 225,
    }, // teal
    Color::Rgb {
        r: 129,
        g: 140,
        b: 248,
    }, // periwinkle
    Color::Rgb {
        r: 192,
        g: 132,
        b: 252,
    }, // lavender
    Color::Rgb {
        r: 244,
        g: 114,
        b: 182,
    }, // pink
    Color::Rgb {
        r: 251,
        g: 146,
        b: 60,
    }, // tangerine
    Color::Rgb {
        r: 45,
        g: 212,
        b: 191,
    }, // turquoise
];

fn build_project_color_map(
    tasks: &[crate::model::Task],
) -> std::collections::HashMap<String, Color> {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut map = std::collections::HashMap::new();
    for task in tasks {
        if let Some(ref name) = task.project {
            if !name.is_empty() && !map.contains_key(name) {
                let idx = rng.random_range(0..PROJECT_PALETTE.len());
                map.insert(name.clone(), PROJECT_PALETTE[idx]);
            }
        }
    }
    map
}
