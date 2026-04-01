use crate::model::{SortKey, Status, Task};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;

pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tasks (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            title   TEXT    NOT NULL,
            status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
            source  TEXT    NOT NULL DEFAULT 'private',
            project TEXT,
            due     TEXT,
            done_at TEXT,
            created TEXT    NOT NULL,
            updated TEXT    NOT NULL
        );",
    )?;
    // Migrate: add 'closed' to CHECK constraint for existing databases
    migrate_status_check(&conn)?;
    Ok(conn)
}

fn migrate_status_check(conn: &Connection) -> Result<(), rusqlite::Error> {
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='tasks'",
        [],
        |row| row.get(0),
    )?;
    if sql.contains("'closed'") {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE tasks_new (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            title   TEXT    NOT NULL,
            status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
            source  TEXT    NOT NULL DEFAULT 'private',
            project TEXT,
            due     TEXT,
            done_at TEXT,
            created TEXT    NOT NULL,
            updated TEXT    NOT NULL
         );
         INSERT INTO tasks_new SELECT * FROM tasks;
         DROP TABLE tasks;
         ALTER TABLE tasks_new RENAME TO tasks;
         COMMIT;",
    )?;
    Ok(())
}

pub fn add_task(
    conn: &Connection,
    title: &str,
    project: Option<&str>,
    due: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<u32, rusqlite::Error> {
    let today_str = today.to_string();
    let due_str = due.map(|d| d.to_string());
    conn.execute(
        "INSERT INTO tasks (title, source, project, due, created, updated)
         VALUES (?1, 'private', ?2, ?3, ?4, ?4)",
        params![title, project, due_str, today_str],
    )?;
    Ok(conn.last_insert_rowid() as u32)
}

pub fn find_task(conn: &Connection, id: u32) -> Result<Option<Task>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, title, status, source, project, due, done_at, created, updated
         FROM tasks WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_task)?;
    rows.next().transpose()
}

pub fn close_task(conn: &Connection, id: u32, today: NaiveDate) -> Result<(), rusqlite::Error> {
    let today_str = today.to_string();
    conn.execute(
        "UPDATE tasks SET status = 'closed', updated = ?1 WHERE id = ?2",
        params![today_str, id],
    )?;
    Ok(())
}

pub fn complete_task(conn: &Connection, id: u32, today: NaiveDate) -> Result<(), rusqlite::Error> {
    let today_str = today.to_string();
    conn.execute(
        "UPDATE tasks SET status = 'done', done_at = ?1, updated = ?1 WHERE id = ?2",
        params![today_str, id],
    )?;
    Ok(())
}

pub fn list_tasks(
    conn: &Connection,
    all: bool,
    project: Option<&str>,
    sort: SortKey,
) -> Result<Vec<Task>, rusqlite::Error> {
    let base =
        "SELECT id, title, status, source, project, due, done_at, created, updated FROM tasks";
    let mut conditions = Vec::new();
    if !all {
        conditions.push("status = 'open'");
    }
    if project.is_some() {
        conditions.push("project = ?1");
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    let sql = format!("{}{} ORDER BY {}", base, where_clause, sort.as_sql());

    let mut stmt = conn.prepare(&sql)?;
    let tasks: Vec<Task> = if let Some(p) = project {
        stmt.query_map(params![p], row_to_task)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map([], row_to_task)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(tasks)
}

pub fn update_task(
    conn: &Connection,
    id: u32,
    title: Option<&str>,
    project: Option<&str>,
    due: Option<NaiveDate>,
    today: NaiveDate,
) -> Result<(), rusqlite::Error> {
    let today_str = today.to_string();
    let mut sets = vec!["updated = ?1"];
    let mut param_idx = 2u32;
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(today_str)];

    if let Some(t) = title {
        sets.push("title = ?2");
        values.push(Box::new(t.to_string()));
        param_idx = 3;
    }
    if let Some(p) = project {
        let placeholder = if param_idx == 2 { "?2" } else { "?3" };
        sets.push(if placeholder == "?2" {
            "project = ?2"
        } else {
            "project = ?3"
        });
        values.push(Box::new(p.to_string()));
        param_idx += 1;
    }
    if let Some(d) = due {
        let placeholder = match param_idx {
            2 => "due = ?2",
            3 => "due = ?3",
            _ => "due = ?4",
        };
        sets.push(placeholder);
        values.push(Box::new(d.to_string()));
        param_idx += 1;
    }

    let id_placeholder = format!("?{}", param_idx);
    let sql = format!(
        "UPDATE tasks SET {} WHERE id = {}",
        sets.join(", "),
        id_placeholder
    );
    values.push(Box::new(id));

    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    conn.execute(&sql, params.as_slice())?;
    Ok(())
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("invalid date in database")
}

fn row_to_task(row: &rusqlite::Row) -> Result<Task, rusqlite::Error> {
    let id: u32 = row.get(0)?;
    let title: String = row.get(1)?;
    let status_str: String = row.get(2)?;
    let source: String = row.get(3)?;
    let project: Option<String> = row.get(4)?;
    let due_str: Option<String> = row.get(5)?;
    let done_at_str: Option<String> = row.get(6)?;
    let created_str: String = row.get(7)?;
    let updated_str: String = row.get(8)?;

    Ok(Task {
        id,
        title,
        status: Status::from_str(&status_str),
        source,
        project,
        due: due_str.map(|s| parse_date(&s)),
        done_at: done_at_str.map(|s| parse_date(&s)),
        created: parse_date(&created_str),
        updated: parse_date(&updated_str),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn open_in_memory() -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                title   TEXT    NOT NULL,
                status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
                source  TEXT    NOT NULL DEFAULT 'private',
                project TEXT,
                due     TEXT,
                done_at TEXT,
                created TEXT    NOT NULL,
                updated TEXT    NOT NULL
            );",
        )?;
        Ok(conn)
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
    }

    #[test]
    fn test_open_creates_schema() {
        let conn = open_in_memory().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='tasks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_add_and_find() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Test task", Some("myproject"), None, today()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "Test task");
        assert_eq!(task.project, Some("myproject".to_string()));
        assert_eq!(task.status, Status::Open);
        assert_eq!(task.created, today());
    }

    #[test]
    fn test_complete_task() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Complete me", None, None, today()).unwrap();
        complete_task(&conn, id, today()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.status, Status::Done);
        assert_eq!(task.done_at, Some(today()));
    }

    #[test]
    fn test_update_task_title() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Old title", None, None, today()).unwrap();
        update_task(&conn, id, Some("New title"), None, None, today()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "New title");
    }

    #[test]
    fn test_update_task_multiple_fields() {
        let conn = open_in_memory().unwrap();
        let due = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let id = add_task(&conn, "Task", None, None, today()).unwrap();
        update_task(&conn, id, Some("Updated"), Some("proj"), Some(due), today()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "Updated");
        assert_eq!(task.project, Some("proj".to_string()));
        assert_eq!(task.due, Some(due));
    }

    #[test]
    fn test_close_task() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Close me", None, None, today()).unwrap();
        close_task(&conn, id, today()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.status, Status::Closed);
    }

    #[test]
    fn test_find_task_not_found() {
        let conn = open_in_memory().unwrap();
        let result = find_task(&conn, 999).unwrap();
        assert!(result.is_none());
    }
}
