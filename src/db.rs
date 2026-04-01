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
            status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done')),
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
                status  TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done')),
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
}
