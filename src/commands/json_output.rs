//! JSON output DTOs and helpers for `list`/`search`/`projects` commands.
//!
//! These are output-only Data Transfer Objects. We deliberately do *not* derive
//! `Serialize` on the internal `model::Task` / `db::ProjectSummary` types so that
//! the JSON schema is decoupled from internal representation and chrono types are
//! never serialized directly (all dates/datetimes are rendered as fixed-format
//! strings here).

use serde::Serialize;

use crate::db::ProjectSummary;
use crate::model::Task;

const DATE_FMT: &str = "%Y-%m-%d";
const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";

/// JSON representation of a single task.
#[derive(Debug, Serialize)]
pub struct TaskJson {
    pub id: u32,
    pub title: String,
    /// One of "open" / "done" / "closed".
    pub status: String,
    pub project: Option<String>,
    /// Due date as "YYYY-MM-DD", or null.
    pub due: Option<String>,
    /// Creation datetime as "YYYY-MM-DD HH:MM:SS".
    pub created: String,
    /// Completion datetime as "YYYY-MM-DD HH:MM:SS", or null.
    pub done_at: Option<String>,
    /// Last-updated datetime as "YYYY-MM-DD HH:MM:SS".
    pub updated: String,
    /// Reminder dates as "YYYY-MM-DD" strings.
    pub reminds: Vec<String>,
    pub important: bool,
    pub source: String,
}

impl From<&Task> for TaskJson {
    fn from(task: &Task) -> Self {
        TaskJson {
            id: task.id,
            title: task.title.clone(),
            status: task.status.as_str().to_string(),
            project: task.project.clone(),
            due: task.due.map(|d| d.format(DATE_FMT).to_string()),
            created: task.created.format(DATETIME_FMT).to_string(),
            done_at: task.done_at.map(|d| d.format(DATETIME_FMT).to_string()),
            updated: task.updated.format(DATETIME_FMT).to_string(),
            reminds: task
                .reminds
                .iter()
                .map(|d| d.format(DATE_FMT).to_string())
                .collect(),
            important: task.important,
            source: task.source.clone(),
        }
    }
}

/// JSON representation of a project summary.
#[derive(Debug, Serialize)]
pub struct ProjectJson {
    pub name: String,
    pub category: Option<String>,
    pub open: u32,
    pub done: u32,
    pub closed: u32,
    pub total: u32,
}

impl From<&ProjectSummary> for ProjectJson {
    fn from(p: &ProjectSummary) -> Self {
        ProjectJson {
            name: p.name.clone(),
            category: p.category.clone(),
            open: p.open_count,
            done: p.done_count,
            closed: p.closed_count,
            total: p.open_count + p.done_count + p.closed_count,
        }
    }
}

/// Serialize `value` as pretty-printed JSON to stdout.
///
/// An empty `Vec` renders as `[]`. Exits the process on a serialization error
/// (which should not happen for these plain DTOs).
pub fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("Error: failed to serialize JSON: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Status;
    use chrono::{NaiveDate, NaiveDateTime};

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
        day(y, m, d).and_hms_opt(h, mi, s).unwrap()
    }

    fn make_task(id: u32, title: &str, status: Status) -> Task {
        Task {
            id,
            title: title.to_string(),
            status,
            source: "manual".to_string(),
            created: dt(2026, 6, 1, 9, 30, 0),
            project: None,
            due: None,
            done_at: None,
            updated: dt(2026, 6, 2, 10, 0, 0),
            reminds: Vec::new(),
            important: false,
        }
    }

    #[test]
    fn test_task_json_open_with_due_and_reminds() {
        let mut t = make_task(1, "Write report", Status::Open);
        t.project = Some("alpha".to_string());
        t.due = Some(day(2026, 7, 10));
        t.reminds = vec![day(2026, 7, 8), day(2026, 7, 9)];
        t.important = true;

        let j = TaskJson::from(&t);
        assert_eq!(j.id, 1);
        assert_eq!(j.title, "Write report");
        assert_eq!(j.status, "open");
        assert_eq!(j.project.as_deref(), Some("alpha"));
        assert_eq!(j.due.as_deref(), Some("2026-07-10"));
        assert_eq!(j.created, "2026-06-01 09:30:00");
        assert_eq!(j.updated, "2026-06-02 10:00:00");
        assert_eq!(j.done_at, None);
        assert_eq!(j.reminds, vec!["2026-07-08", "2026-07-09"]);
        assert!(j.important);
        assert_eq!(j.source, "manual");
    }

    #[test]
    fn test_task_json_no_due_no_reminds_nulls() {
        let t = make_task(2, "No dates", Status::Open);
        let j = TaskJson::from(&t);
        assert_eq!(j.due, None);
        assert_eq!(j.project, None);
        assert_eq!(j.done_at, None);
        assert!(j.reminds.is_empty());

        // Empty reminds Vec serializes to `[]`, null fields to `null`.
        let s = serde_json::to_string(&j).unwrap();
        assert!(s.contains("\"reminds\":[]"), "got: {s}");
        assert!(s.contains("\"due\":null"), "got: {s}");
        assert!(s.contains("\"project\":null"), "got: {s}");
        assert!(s.contains("\"done_at\":null"), "got: {s}");
    }

    #[test]
    fn test_task_json_done_status_and_done_at() {
        let mut t = make_task(3, "Finished", Status::Done);
        t.done_at = Some(dt(2026, 6, 5, 12, 0, 0));
        let j = TaskJson::from(&t);
        assert_eq!(j.status, "done");
        assert_eq!(j.done_at.as_deref(), Some("2026-06-05 12:00:00"));
    }

    #[test]
    fn test_task_json_closed_status() {
        let t = make_task(4, "Abandoned", Status::Closed);
        let j = TaskJson::from(&t);
        assert_eq!(j.status, "closed");
    }

    #[test]
    fn test_project_json_conversion_and_total() {
        let p = ProjectSummary {
            name: "alpha".to_string(),
            category: Some("work".to_string()),
            open_count: 2,
            done_count: 3,
            closed_count: 1,
        };
        let j = ProjectJson::from(&p);
        assert_eq!(j.name, "alpha");
        assert_eq!(j.category.as_deref(), Some("work"));
        assert_eq!(j.open, 2);
        assert_eq!(j.done, 3);
        assert_eq!(j.closed, 1);
        assert_eq!(j.total, 6);
        assert_eq!(j.total, j.open + j.done + j.closed);
    }

    #[test]
    fn test_project_json_category_none_serializes_null() {
        let p = ProjectSummary {
            name: "solo".to_string(),
            category: None,
            open_count: 0,
            done_count: 0,
            closed_count: 0,
        };
        let j = ProjectJson::from(&p);
        assert_eq!(j.category, None);
        assert_eq!(j.total, 0);

        let s = serde_json::to_string(&j).unwrap();
        assert!(s.contains("\"category\":null"), "got: {s}");
    }

    #[test]
    fn test_print_json_empty_vec_is_bracket_pair() {
        let empty: Vec<TaskJson> = Vec::new();
        let s = serde_json::to_string_pretty(&empty).unwrap();
        assert_eq!(s, "[]");

        let empty_projects: Vec<ProjectJson> = Vec::new();
        let s = serde_json::to_string_pretty(&empty_projects).unwrap();
        assert_eq!(s, "[]");
    }
}
