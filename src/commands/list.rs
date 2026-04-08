use chrono::{Datelike, Local};
use clap::Args;
use comfy_table::modifiers::UTF8_SOLID_INNER_BORDERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use terminal_size::{terminal_size, Width};

use crate::config;
use crate::db;
use crate::model::{SortKey, SortOrder, Status};

const NARROW_THRESHOLD: u16 = 60;

#[derive(Args)]
pub struct ListArgs {
    /// Show all tasks including done
    #[arg(short, long)]
    pub all: bool,

    /// Filter by project name
    #[arg(short, long)]
    pub project: Option<String>,

    /// Sort by: id, due, project, created (repeatable)
    #[arg(short, long, default_value = "id")]
    pub sort: Vec<String>,

    /// Sort ascending
    #[arg(long, conflicts_with = "desc")]
    pub asc: bool,

    /// Sort descending
    #[arg(long, conflicts_with = "asc")]
    pub desc: bool,

    /// Show only important tasks
    #[arg(long)]
    pub important_only: bool,
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

    let sorts: Vec<SortKey> = args
        .sort
        .iter()
        .map(|s| match s.as_str() {
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
        })
        .collect();

    let order = if args.desc {
        SortOrder::Desc
    } else {
        SortOrder::Asc
    };

    let tasks = match db::list_tasks(
        &conn,
        args.all,
        args.project.as_deref(),
        &sorts,
        order,
        args.important_only,
    ) {
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

    // Fill reminds for each task
    let mut tasks = tasks;
    for task in &mut tasks {
        task.reminds = db::get_reminds_for_task(&conn, task.id).unwrap_or_default();
    }

    let today = Local::now().date_naive();
    let done_count = tasks
        .iter()
        .filter(|t| t.status == Status::Done || t.status == Status::Closed)
        .count();

    let project_colors = build_project_color_map(&tasks);

    let term_width = terminal_size().map(|(Width(w), _)| w).unwrap_or(80);
    let compact = term_width < NARROW_THRESHOLD;

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_SOLID_INNER_BORDERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(term_width);

    if compact {
        table.set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Title").add_attribute(Attribute::Bold),
            Cell::new("Due").add_attribute(Attribute::Bold),
        ]);
    } else {
        table.set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Status").add_attribute(Attribute::Bold),
            Cell::new("Project").add_attribute(Attribute::Bold),
            Cell::new("Title").add_attribute(Attribute::Bold),
            Cell::new("Due").add_attribute(Attribute::Bold),
            Cell::new("Remind").add_attribute(Attribute::Bold),
            Cell::new("Age").add_attribute(Attribute::Bold),
        ]);
    }

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
        let remind_text: String = task
            .reminds
            .iter()
            .map(|d| format!("{}/{}", d.month(), d.day()))
            .collect::<Vec<_>>()
            .join(", ");
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

        if compact {
            let id_cell = if is_done {
                Cell::new(&id_text).fg(Color::Green)
            } else if is_closed {
                Cell::new(&id_text).fg(Color::DarkGrey)
            } else {
                Cell::new(&id_text).fg(Color::Cyan)
            };

            let title_cell = if is_done {
                Cell::new(&task.title).fg(Color::Green)
            } else if is_closed {
                Cell::new(&task.title).fg(Color::DarkGrey)
            } else if is_overdue {
                let cell = Cell::new(&task.title).fg(Color::Red);
                if task.important {
                    cell.add_attribute(Attribute::Bold)
                } else {
                    cell
                }
            } else {
                let cell = Cell::new(&task.title);
                if task.important {
                    cell.add_attribute(Attribute::Bold)
                } else {
                    cell
                }
            };

            let due_cell = if is_inactive {
                Cell::new(&due_text).fg(if is_done {
                    Color::Green
                } else {
                    Color::DarkGrey
                })
            } else if is_overdue {
                Cell::new(&due_text).fg(Color::Red)
            } else if is_due_today {
                Cell::new(&due_text).fg(Color::Yellow)
            } else if task.due.is_some() {
                Cell::new(&due_text).fg(Color::Green)
            } else {
                Cell::new(&due_text)
            };

            table.add_row(vec![id_cell, title_cell, due_cell]);
        } else if is_done {
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
                Cell::new(&remind_text).fg(green),
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
                Cell::new(&remind_text).fg(grey),
                Cell::new(age_text).fg(grey),
            ]);
        } else {
            let title_cell = if is_overdue {
                let cell = Cell::new(&task.title).fg(Color::Red);
                if task.important {
                    cell.add_attribute(Attribute::Bold)
                } else {
                    cell
                }
            } else {
                let cell = Cell::new(&task.title);
                if task.important {
                    cell.add_attribute(Attribute::Bold)
                } else {
                    cell
                }
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

            let remind_cell = Cell::new(&remind_text);

            table.add_row(vec![
                Cell::new(id_text).fg(Color::Cyan),
                Cell::new("OPEN").fg(Color::Blue),
                Cell::new(&project_text).fg(project_colors
                    .get(project_text.as_str())
                    .copied()
                    .unwrap_or(Color::White)),
                title_cell,
                due_cell,
                remind_cell,
                age_cell,
            ]);
        }
    }

    let id_col = table.column_mut(0).expect("id column");
    id_col.set_cell_alignment(CellAlignment::Right);

    if !compact {
        let age_col = table.column_mut(6).expect("age column");
        age_col.set_cell_alignment(CellAlignment::Right);
    }

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
