use chrono::{Datelike, Local, NaiveDate};
use clap::Args;
use comfy_table::modifiers::UTF8_SOLID_INNER_BORDERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use terminal_size::{terminal_size, Width};

use crate::config;
use crate::date_parser;
use crate::db;
use crate::model::{SortKey, SortOrder, Status, Task};

const NARROW_THRESHOLD: u16 = 60;

#[derive(Args)]
pub struct ListArgs {
    /// Show all tasks including done
    #[arg(short, long)]
    pub all: bool,

    /// Filter by project name
    #[arg(short, long)]
    pub project: Option<String>,

    /// Filter by project category
    #[arg(short = 'c', long)]
    pub category: Option<String>,

    /// Filter by due date (YYYY-MM-DD, 今日, 明日, 来週, 月曜〜日曜, etc.)
    #[arg(short, long)]
    pub due: Option<String>,

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

    /// Follow mode: full-screen, auto-refreshing view (q to quit; requires a TTY)
    #[arg(short = 'f', long)]
    pub follow: bool,

    /// Polling interval in seconds for follow mode
    #[arg(long, default_value_t = 2, value_name = "SECS")]
    pub interval: u64,
}

/// Resolved query parameters extracted from `ListArgs`.
pub struct ListQuery {
    pub all: bool,
    pub project: Option<String>,
    pub category: Option<String>,
    pub due: Option<NaiveDate>,
    pub sorts: Vec<SortKey>,
    pub order: SortOrder,
    pub important_only: bool,
}

/// Resolve sort keys / order from `ListArgs`. Exits the process on an unknown sort key.
pub fn resolve_query(args: &ListArgs) -> ListQuery {
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

    let due = args.due.as_ref().map(|s| {
        date_parser::parse_fuzzy_date(s).unwrap_or_else(|| {
            eprintln!(
                "Error: invalid due date '{}'. Use: YYYY-MM-DD, 今日, 明日, 来週, 曜日名 etc.",
                s
            );
            std::process::exit(1);
        })
    });

    ListQuery {
        all: args.all,
        project: args.project.clone(),
        category: args.category.clone(),
        due,
        sorts,
        order,
        important_only: args.important_only,
    }
}

pub fn run(args: ListArgs) {
    if args.follow {
        crate::commands::follow::run_follow(&args);
        return;
    }

    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let query = resolve_query(&args);

    let tasks = match db::list_tasks(
        &conn,
        query.all,
        query.project.as_deref(),
        query.category.as_deref(),
        query.due,
        &query.sorts,
        query.order,
        query.important_only,
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

    print_task_table(&tasks, args.all, &conn);
}

pub fn print_task_table(tasks: &[Task], all: bool, conn: &rusqlite::Connection) {
    // Fill reminds for each task
    let mut tasks = tasks.to_vec();
    for task in &mut tasks {
        task.reminds = db::get_reminds_for_task(conn, task.id).unwrap_or_default();
    }

    let project_colors = build_project_color_map(&tasks);
    let term_width = terminal_size().map(|(Width(w), _)| w).unwrap_or(80);

    let rendered = build_task_table_string(&tasks, all, &project_colors, term_width);
    println!("{rendered}");
}

/// Build the rendered task table (table + footer) as a pure string.
///
/// This function performs no I/O and contains no randomness: the project color
/// map is supplied by the caller, so given the same inputs it always returns the
/// same output. The non-follow path generates a fresh (random) color map per
/// call to preserve historical behavior, while the follow loop generates one map
/// up front and reuses it to avoid flicker.
pub fn build_task_table_string(
    tasks: &[Task],
    all: bool,
    project_colors: &std::collections::HashMap<String, Color>,
    term_width: u16,
) -> String {
    let today = Local::now().date_naive();
    let done_count = tasks
        .iter()
        .filter(|t| t.status == Status::Done || t.status == Status::Closed)
        .count();

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

    for task in tasks {
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
            let days = (today - task.created.date()).num_days();
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
            } else if task.important {
                Cell::new(&task.title)
                    .fg(Color::Magenta)
                    .add_attribute(Attribute::Bold)
            } else {
                Cell::new(&task.title)
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
            } else if task.important {
                Cell::new(&task.title)
                    .fg(Color::Magenta)
                    .add_attribute(Attribute::Bold)
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

            let days = (today - task.created.date()).num_days();
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

    let footer = if all && done_count > 0 {
        format!("{} tasks ({} done)", tasks.len(), done_count)
    } else {
        format!("{} tasks", tasks.len())
    };

    format!("{table}\n\n{footer}")
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

pub fn build_project_color_map(
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::collections::HashMap;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn make_task(id: u32, title: &str, status: Status) -> Task {
        Task {
            id,
            title: title.to_string(),
            status,
            source: "manual".to_string(),
            created: day(2026, 6, 1).and_hms_opt(0, 0, 0).unwrap(),
            project: None,
            due: None,
            done_at: None,
            updated: day(2026, 6, 1).and_hms_opt(0, 0, 0).unwrap(),
            reminds: Vec::new(),
            important: false,
        }
    }

    // Wide enough that the full (non-compact) layout is used.
    const WIDE: u16 = 120;

    #[test]
    fn test_build_string_contains_core_fields() {
        let mut open = make_task(1, "Write report", Status::Open);
        open.project = Some("alpha".to_string());
        open.due = Some(day(2026, 7, 10));
        open.reminds = vec![day(2026, 7, 8)];

        let done = make_task(2, "Old chore", Status::Done);
        let closed = make_task(3, "Abandoned", Status::Closed);

        let tasks = vec![open, done, closed];
        let colors: HashMap<String, Color> = HashMap::new();

        let out = build_task_table_string(&tasks, true, &colors, WIDE);

        assert!(out.contains("#1"));
        assert!(out.contains("#2"));
        assert!(out.contains("#3"));
        assert!(out.contains("Write report"));
        assert!(out.contains("Old chore"));
        assert!(out.contains("Abandoned"));
        assert!(out.contains("OPEN"));
        assert!(out.contains("DONE"));
        assert!(out.contains("CLOSED"));
        // due 7/10 and remind 7/8
        assert!(out.contains("7/10"));
        assert!(out.contains("7/8"));
    }

    #[test]
    fn test_build_string_footer_plain() {
        let tasks = vec![
            make_task(1, "One", Status::Open),
            make_task(2, "Two", Status::Open),
        ];
        let colors: HashMap<String, Color> = HashMap::new();

        let out = build_task_table_string(&tasks, false, &colors, WIDE);
        assert!(out.contains("2 tasks"));
        assert!(!out.contains("done)"));
    }

    #[test]
    fn test_build_string_footer_with_done_count() {
        let tasks = vec![
            make_task(1, "Open one", Status::Open),
            make_task(2, "Done one", Status::Done),
            make_task(3, "Closed one", Status::Closed),
        ];
        let colors: HashMap<String, Color> = HashMap::new();

        // all = true and there are inactive tasks -> "N tasks (M done)"
        let out = build_task_table_string(&tasks, true, &colors, WIDE);
        assert!(out.contains("3 tasks (2 done)"));
    }

    #[test]
    fn test_build_string_empty_tasks() {
        let tasks: Vec<Task> = Vec::new();
        let colors: HashMap<String, Color> = HashMap::new();

        let out = build_task_table_string(&tasks, false, &colors, WIDE);
        // No rows -> footer reports zero tasks.
        assert!(out.contains("0 tasks"));
    }

    #[test]
    fn test_build_string_compact_layout() {
        // term_width below NARROW_THRESHOLD switches to the 3-column compact view.
        let narrow = NARROW_THRESHOLD - 1;
        let mut t = make_task(1, "Compact task", Status::Open);
        t.project = Some("proj".to_string());
        let tasks = vec![t];
        let colors: HashMap<String, Color> = HashMap::new();

        let out = build_task_table_string(&tasks, false, &colors, narrow);
        // Compact header has ID/Title/Due but not Status/Project/Remind/Age columns.
        assert!(out.contains("Title"));
        assert!(out.contains("Due"));
        assert!(!out.contains("Status"));
        assert!(!out.contains("Remind"));
        assert!(!out.contains("Age"));
        assert!(out.contains("Compact task"));
        assert!(out.contains("1 tasks"));
    }

    #[test]
    fn test_build_string_is_deterministic() {
        let mut t = make_task(1, "Stable", Status::Open);
        t.project = Some("alpha".to_string());
        let tasks = vec![t];

        let mut colors: HashMap<String, Color> = HashMap::new();
        colors.insert("alpha".to_string(), Color::Cyan);

        let a = build_task_table_string(&tasks, false, &colors, WIDE);
        let b = build_task_table_string(&tasks, false, &colors, WIDE);
        assert_eq!(a, b);
    }
}
