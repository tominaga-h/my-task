use chrono::{Local, NaiveDate};
use clap::Args;
use std::collections::HashMap;
use std::io::{Read, Write};

use crate::config;
use crate::db;
use crate::model::{SortKey, Task};

#[derive(Args)]
pub struct EditArgs {
    /// Task ID to edit (optional with -i)
    pub id: Option<u32>,

    /// New title
    #[arg(short, long)]
    pub title: Option<String>,

    /// New project name
    #[arg(short, long)]
    pub project: Option<String>,

    /// New due date (YYYY-MM-DD)
    #[arg(short, long)]
    pub due: Option<NaiveDate>,

    /// Open in editor (like git rebase -i)
    #[arg(short = 'i', long)]
    pub interactive: bool,

    /// Filter by project (with -i)
    #[arg(short = 'P', long = "filter-project")]
    pub filter_project: Option<String>,
}

pub fn run(args: EditArgs) {
    if args.interactive {
        if args.title.is_some() || args.project.is_some() || args.due.is_some() {
            eprintln!("Error: --interactive cannot be used with --title, --project, --due");
            std::process::exit(1);
        }
        run_interactive(args.id, args.filter_project);
    } else {
        run_flag(args);
    }
}

fn run_flag(args: EditArgs) {
    let id = match args.id {
        Some(id) => id,
        None => {
            eprintln!("Error: task ID is required");
            std::process::exit(1);
        }
    };

    if args.title.is_none() && args.project.is_none() && args.due.is_none() {
        eprintln!("Error: specify at least one field to edit (--title, --project, --due)");
        std::process::exit(1);
    }

    if let Some(ref t) = args.title {
        if t.is_empty() {
            eprintln!("Error: title cannot be empty");
            std::process::exit(1);
        }
    }

    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    match db::find_task(&conn, id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            eprintln!("Error: task #{} not found", id);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    }

    let today = Local::now().date_naive();
    if db::update_task(
        &conn,
        id,
        args.title.as_deref(),
        args.project.as_deref(),
        args.due,
        today,
    )
    .is_err()
    {
        eprintln!("Error: failed to write database: {}", db_path.display());
        std::process::exit(1);
    }

    let task = db::find_task(&conn, id).unwrap().unwrap();
    println!("Updated: #{} {}", task.id, task.title);
}

fn run_interactive(id: Option<u32>, filter_project: Option<String>) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let tasks: Vec<Task> = if let Some(id) = id {
        match db::find_task(&conn, id) {
            Ok(Some(t)) => vec![t],
            Ok(None) => {
                eprintln!("Error: task #{} not found", id);
                std::process::exit(1);
            }
            Err(_) => {
                eprintln!("Error: failed to read database: {}", db_path.display());
                std::process::exit(1);
            }
        }
    } else {
        match db::list_tasks(&conn, false, filter_project.as_deref(), SortKey::Id) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("Error: failed to read database: {}", db_path.display());
                std::process::exit(1);
            }
        }
    };

    if tasks.is_empty() {
        println!("No tasks to edit");
        return;
    }

    let original = tasks_to_yaml(&tasks);

    let mut tmpfile = tempfile::Builder::new()
        .suffix(".yml")
        .tempfile()
        .unwrap_or_else(|_| {
            eprintln!("Error: failed to create temporary file");
            std::process::exit(1);
        });
    tmpfile.write_all(original.as_bytes()).unwrap_or_else(|_| {
        eprintln!("Error: failed to write temporary file");
        std::process::exit(1);
    });
    tmpfile.flush().unwrap();

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(&editor)
        .arg(tmpfile.path())
        .status();

    match status {
        Ok(s) if s.success() => {}
        _ => {
            eprintln!("Error: failed to open editor");
            std::process::exit(1);
        }
    }

    let mut edited = String::new();
    let mut file = std::fs::File::open(tmpfile.path()).unwrap_or_else(|_| {
        eprintln!("Error: failed to read temporary file");
        std::process::exit(1);
    });
    file.read_to_string(&mut edited).unwrap_or_else(|_| {
        eprintln!("Error: failed to read temporary file");
        std::process::exit(1);
    });

    let original_map: HashMap<u32, &Task> = tasks.iter().map(|t| (t.id, t)).collect();
    let parsed = parse_yaml(&edited);
    let parsed_ids: std::collections::HashSet<u32> = parsed.iter().map(|e| e.id).collect();

    let today = Local::now().date_naive();
    let mut updated_count = 0u32;
    let mut closed_count = 0u32;

    // Close tasks whose blocks were deleted
    for task in &tasks {
        if !parsed_ids.contains(&task.id) {
            if db::close_task(&conn, task.id, today).is_err() {
                eprintln!("Error: failed to close task #{}", task.id);
                std::process::exit(1);
            }
            closed_count += 1;
        }
    }

    // Update tasks with changes
    for entry in &parsed {
        let orig = match original_map.get(&entry.id) {
            Some(t) => t,
            None => continue,
        };

        let title_changed = entry.title != orig.title;
        let project_changed = entry.project != orig.project;
        let due_changed = entry.due != orig.due;

        if !title_changed && !project_changed && !due_changed {
            continue;
        }

        if entry.title.is_empty() {
            eprintln!("Error: title cannot be empty for task #{}", entry.id);
            std::process::exit(1);
        }

        let new_title = if title_changed {
            Some(entry.title.as_str())
        } else {
            None
        };
        let new_project = if project_changed {
            Some(entry.project.as_deref().unwrap_or(""))
        } else {
            None
        };
        let new_due = if due_changed { entry.due } else { None };

        if db::update_task(&conn, entry.id, new_title, new_project, new_due, today).is_err() {
            eprintln!("Error: failed to update task #{}", entry.id);
            std::process::exit(1);
        }
        updated_count += 1;
    }

    if updated_count == 0 && closed_count == 0 {
        println!("No changes");
    } else {
        let mut parts = Vec::new();
        if updated_count > 0 {
            parts.push(format!("Updated {} tasks", updated_count));
        }
        if closed_count > 0 {
            parts.push(format!("Closed {} tasks", closed_count));
        }
        println!("{}", parts.join(", "));
    }
}

fn tasks_to_yaml(tasks: &[Task]) -> String {
    let mut out = String::new();
    out.push_str("# my-task edit: 編集して保存してください\n");
    out.push_str("# 行やブロックを削除するとタスクをクローズします\n");
    out.push_str("# id は変更できません\n");

    for task in tasks {
        out.push('\n');
        out.push_str(&format!("- id: {}\n", task.id));
        out.push_str(&format!("  title: {}\n", task.title));
        out.push_str(&format!(
            "  project: {}\n",
            task.project.as_deref().unwrap_or("")
        ));
        out.push_str(&format!(
            "  due: {}\n",
            task.due.map(|d| d.to_string()).unwrap_or_default()
        ));
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
struct EditEntry {
    id: u32,
    title: String,
    project: Option<String>,
    due: Option<NaiveDate>,
}

fn parse_yaml(input: &str) -> Vec<EditEntry> {
    let mut entries = Vec::new();
    let mut current_id: Option<u32> = None;
    let mut current_title: Option<String> = None;
    let mut current_project: Option<String> = None;
    let mut current_due: Option<NaiveDate> = None;

    for (line_num, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with("- id:") {
            // Save previous entry
            if let Some(id) = current_id {
                entries.push(EditEntry {
                    id,
                    title: current_title.take().unwrap_or_default(),
                    project: current_project.take(),
                    due: current_due.take(),
                });
            }
            let val = trimmed.trim_start_matches("- id:").trim();
            current_id = Some(val.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("Error: failed to parse edit file at line {}", line_num + 1);
                std::process::exit(1);
            }));
            current_title = None;
            current_project = None;
            current_due = None;
        } else if trimmed.starts_with("title:") {
            let val = trimmed.trim_start_matches("title:").trim();
            current_title = Some(val.to_string());
        } else if trimmed.starts_with("project:") {
            let val = trimmed.trim_start_matches("project:").trim();
            current_project = if val.is_empty() {
                None
            } else {
                Some(val.to_string())
            };
        } else if trimmed.starts_with("due:") {
            let val = trimmed.trim_start_matches("due:").trim();
            current_due = if val.is_empty() {
                None
            } else {
                Some(
                    NaiveDate::parse_from_str(val, "%Y-%m-%d").unwrap_or_else(|_| {
                        eprintln!("Error: failed to parse edit file at line {}", line_num + 1);
                        std::process::exit(1);
                    }),
                )
            };
        }
    }

    // Save last entry
    if let Some(id) = current_id {
        entries.push(EditEntry {
            id,
            title: current_title.unwrap_or_default(),
            project: current_project,
            due: current_due,
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_parse_yaml_single() {
        let input = r#"
# comment
- id: 5
  title: Hello world
  project: my-app
  due: 2026-04-10
"#;
        let entries = parse_yaml(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 5);
        assert_eq!(entries[0].title, "Hello world");
        assert_eq!(entries[0].project, Some("my-app".to_string()));
        assert_eq!(
            entries[0].due,
            Some(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap())
        );
    }

    #[test]
    fn test_parse_yaml_multiple() {
        let input = r#"
- id: 1
  title: Task one
  project: proj-a
  due:

- id: 2
  title: Task two
  project:
  due: 2026-05-01
"#;
        let entries = parse_yaml(input);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Task one");
        assert_eq!(entries[0].project, Some("proj-a".to_string()));
        assert_eq!(entries[0].due, None);
        assert_eq!(entries[1].title, "Task two");
        assert_eq!(entries[1].project, None);
        assert_eq!(
            entries[1].due,
            Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap())
        );
    }

    #[test]
    fn test_parse_yaml_deleted_block() {
        let input = r#"
- id: 1
  title: Task one
  project:
  due:
"#;
        // Original had id 1 and 2, but 2 is deleted
        let entries = parse_yaml(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 1);
    }

    #[test]
    fn test_parse_yaml_empty_input() {
        let input = "# only comments\n\n";
        let entries = parse_yaml(input);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_tasks_to_yaml_roundtrip() {
        let tasks = vec![Task {
            id: 1,
            title: "Test task".to_string(),
            status: crate::model::Status::Open,
            source: "private".to_string(),
            created: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            project: Some("my-proj".to_string()),
            due: Some(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()),
            done_at: None,
            updated: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        }];
        let yaml = tasks_to_yaml(&tasks);
        let entries = parse_yaml(&yaml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[0].title, "Test task");
        assert_eq!(entries[0].project, Some("my-proj".to_string()));
        assert_eq!(
            entries[0].due,
            Some(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap())
        );
    }
}
