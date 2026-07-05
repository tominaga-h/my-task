use crate::model::{SortKey, SortOrder, Status, Task};
use chrono::{NaiveDate, NaiveDateTime};
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
        "CREATE TABLE IF NOT EXISTS projects (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            name     TEXT    NOT NULL UNIQUE,
            category TEXT
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tasks (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            title     TEXT    NOT NULL,
            status    TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
            source    TEXT    NOT NULL DEFAULT 'private',
            project_id INTEGER REFERENCES projects(id),
            due       TEXT,
            done_at   TEXT,
            created   TEXT    NOT NULL,
            updated   TEXT    NOT NULL,
            important INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS task_reminds (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id  INTEGER NOT NULL REFERENCES tasks(id),
            remind_at TEXT NOT NULL
        );",
    )?;
    migrate_tasks_schema(&conn)?;
    migrate_projects_schema(&conn)?;
    normalize_datetime_columns(&conn)?;
    Ok(conn)
}

/// Idempotently add the `category` column to the `projects` table if it does not
/// already exist. Running `open()` repeatedly is a no-op once the column exists.
fn migrate_projects_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    if !table_has_column(conn, "projects", "category")? {
        conn.execute_batch("ALTER TABLE projects ADD COLUMN category TEXT")?;
    }
    Ok(())
}

/// Normalize legacy date-only values in the datetime columns (`created`,
/// `updated`, `done_at`) to the `%Y-%m-%d %H:%M:%S` format by appending
/// `00:00:00` to any 10-character (date-only) value. Idempotent: 19-character
/// values are left untouched, so running `open()` repeatedly causes no change.
///
/// `due` (tasks.due) and `task_reminds.remind_at` stay date-only and are never
/// touched here.
fn normalize_datetime_columns(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "UPDATE tasks SET created = created || ' 00:00:00' WHERE length(created) = 10;
         UPDATE tasks SET updated = updated || ' 00:00:00' WHERE length(updated) = 10;
         UPDATE tasks SET done_at = done_at || ' 00:00:00' WHERE done_at IS NOT NULL AND length(done_at) = 10;",
    )?;
    Ok(())
}

fn migrate_tasks_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='tasks'",
        [],
        |row| row.get(0),
    )?;
    if sql.contains("project_id") && sql.contains("important") && sql.contains("'closed'") {
        return Ok(());
    }

    let has_important = table_has_column(conn, "tasks", "important")?;

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    conn.execute_batch("BEGIN;")?;
    let migration_result: Result<(), rusqlite::Error> = (|| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                name     TEXT    NOT NULL UNIQUE,
                category TEXT
            );",
        )?;
        conn.execute_batch(
            "CREATE TABLE tasks_new (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                title      TEXT    NOT NULL,
                status     TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
                source     TEXT    NOT NULL DEFAULT 'private',
                project_id INTEGER REFERENCES projects(id),
                due        TEXT,
                done_at    TEXT,
                created    TEXT    NOT NULL,
                updated    TEXT    NOT NULL,
                important  INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        conn.execute_batch(
            "INSERT OR IGNORE INTO projects (name)
             SELECT DISTINCT project
             FROM tasks
             WHERE project IS NOT NULL AND project != '';",
        )?;
        let important_expr = if has_important {
            "COALESCE(important, 0)"
        } else {
            "0"
        };
        let insert_sql = format!(
            "INSERT INTO tasks_new (id, title, status, source, project_id, due, done_at, created, updated, important)
             SELECT id,
                    title,
                    status,
                    source,
                    CASE
                        WHEN project IS NULL OR project = '' THEN NULL
                        ELSE (SELECT id FROM projects WHERE name = tasks.project)
                    END,
                    due,
                    done_at,
                    created,
                    updated,
                    {}
             FROM tasks;",
            important_expr
        );
        conn.execute_batch(&insert_sql)?;
        conn.execute_batch(
            "DROP TABLE tasks;
             ALTER TABLE tasks_new RENAME TO tasks;",
        )?;
        Ok(())
    })();

    match migration_result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK;");
            conn.execute_batch("PRAGMA foreign_keys = ON;").ok();
            Err(err)
        }
    }
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn resolve_project_id(
    conn: &Connection,
    project: Option<&str>,
) -> Result<Option<i64>, rusqlite::Error> {
    let Some(project) = project.map(str::trim).filter(|p| !p.is_empty()) else {
        return Ok(None);
    };

    conn.execute(
        "INSERT OR IGNORE INTO projects (name) VALUES (?1)",
        params![project],
    )?;
    let id = conn.query_row(
        "SELECT id FROM projects WHERE name = ?1",
        params![project],
        |row| row.get(0),
    )?;
    Ok(Some(id))
}

fn tasks_select_sql() -> &'static str {
    "SELECT t.id, t.title, t.status, t.source, p.name AS project, t.due, t.done_at, t.created, t.updated, t.important
     FROM tasks t
     LEFT JOIN projects p ON t.project_id = p.id"
}

pub fn add_task(
    conn: &Connection,
    title: &str,
    project: Option<&str>,
    due: Option<NaiveDate>,
    now: NaiveDateTime,
    important: bool,
) -> Result<u32, rusqlite::Error> {
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let due_str = due.map(|d| d.to_string());
    let project_id = resolve_project_id(conn, project)?;
    conn.execute(
        "INSERT INTO tasks (title, source, project_id, due, created, updated, important)
         VALUES (?1, 'private', ?2, ?3, ?4, ?4, ?5)",
        params![title, project_id, due_str, now_str, important as i32],
    )?;
    Ok(conn.last_insert_rowid() as u32)
}

pub fn find_task(conn: &Connection, id: u32) -> Result<Option<Task>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("{} WHERE t.id = ?1", tasks_select_sql()))?;
    let mut rows = stmt.query_map(params![id], row_to_task)?;
    rows.next().transpose()
}

pub fn close_task(conn: &Connection, id: u32, now: NaiveDateTime) -> Result<(), rusqlite::Error> {
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE tasks SET status = 'closed', updated = ?1 WHERE id = ?2",
        params![now_str, id],
    )?;
    Ok(())
}

pub fn complete_task(
    conn: &Connection,
    id: u32,
    now: NaiveDateTime,
) -> Result<(), rusqlite::Error> {
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE tasks SET status = 'done', done_at = ?1, updated = ?1 WHERE id = ?2",
        params![now_str, id],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn list_tasks(
    conn: &Connection,
    all: bool,
    project: Option<&str>,
    category: Option<&str>,
    due: Option<NaiveDate>,
    sorts: &[SortKey],
    order: SortOrder,
    important_only: bool,
) -> Result<Vec<Task>, rusqlite::Error> {
    let base = tasks_select_sql();
    let mut conditions: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut param_idx = 1u32;

    if !all {
        conditions.push("t.status = 'open'".to_string());
    }
    if let Some(p) = project {
        conditions.push(format!("p.name = ?{}", param_idx));
        values.push(Box::new(p.to_string()));
        param_idx += 1;
    }
    if let Some(c) = category {
        conditions.push(format!("p.category = ?{}", param_idx));
        values.push(Box::new(c.to_string()));
        param_idx += 1;
    }
    if let Some(target) = due {
        conditions.push(format!("t.due = ?{}", param_idx));
        values.push(Box::new(target.to_string()));
    }
    if important_only {
        conditions.push("t.important = 1".to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };
    let order_clause = sorts
        .iter()
        .map(|k| format!("{} {}", k.as_sql(), order.as_sql()))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("{}{} ORDER BY {}", base, where_clause, order_clause);

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let tasks: Vec<Task> = stmt
        .query_map(params.as_slice(), row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

pub fn update_task(
    conn: &Connection,
    id: u32,
    title: Option<&str>,
    project: Option<&str>,
    due: Option<NaiveDate>,
    now: NaiveDateTime,
    important: Option<bool>,
) -> Result<(), rusqlite::Error> {
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let mut sets: Vec<String> = vec!["updated = ?1".to_string()];
    let mut param_idx = 2u32;
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str)];

    if let Some(t) = title {
        sets.push(format!("title = ?{}", param_idx));
        values.push(Box::new(t.to_string()));
        param_idx += 1;
    }
    if let Some(p) = project {
        let project_id = resolve_project_id(conn, Some(p))?;
        sets.push(format!("project_id = ?{}", param_idx));
        values.push(Box::new(project_id));
        param_idx += 1;
    }
    if let Some(d) = due {
        sets.push(format!("due = ?{}", param_idx));
        values.push(Box::new(d.to_string()));
        param_idx += 1;
    }
    if let Some(imp) = important {
        sets.push(format!("important = ?{}", param_idx));
        values.push(Box::new(imp as i32));
        param_idx += 1;
    }

    let sql = format!(
        "UPDATE tasks SET {} WHERE id = ?{}",
        sets.join(", "),
        param_idx
    );
    values.push(Box::new(id));

    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    conn.execute(&sql, params.as_slice())?;
    Ok(())
}

pub fn get_due_tasks(
    conn: &Connection,
    target_date: NaiveDate,
) -> Result<Vec<Task>, rusqlite::Error> {
    let target_str = target_date.to_string();
    let mut stmt = conn.prepare(&format!(
        "{} WHERE t.status = 'open' AND t.due IS NOT NULL AND t.due <= ?1 ORDER BY t.due ASC",
        tasks_select_sql()
    ))?;
    let tasks = stmt
        .query_map(params![target_str], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

pub fn add_remind(
    conn: &Connection,
    task_id: u32,
    remind_at: NaiveDate,
) -> Result<(), rusqlite::Error> {
    let remind_str = remind_at.to_string();
    conn.execute(
        "INSERT INTO task_reminds (task_id, remind_at) VALUES (?1, ?2)",
        params![task_id, remind_str],
    )?;
    Ok(())
}

pub fn get_reminds_for_task(
    conn: &Connection,
    task_id: u32,
) -> Result<Vec<NaiveDate>, rusqlite::Error> {
    let mut stmt = conn
        .prepare("SELECT remind_at FROM task_reminds WHERE task_id = ?1 ORDER BY remind_at ASC")?;
    let reminds = stmt
        .query_map(params![task_id], |row| {
            let s: String = row.get(0)?;
            Ok(parse_date(&s))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(reminds)
}

pub fn get_tasks_with_remind_today(
    conn: &Connection,
    today: NaiveDate,
) -> Result<Vec<Task>, rusqlite::Error> {
    let today_str = today.to_string();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT t.id, t.title, t.status, t.source, p.name AS project, t.due, t.done_at, t.created, t.updated, t.important
         FROM tasks t
         LEFT JOIN projects p ON t.project_id = p.id
         JOIN task_reminds r ON t.id = r.task_id
         WHERE t.status = 'open' AND r.remind_at = ?1
         ORDER BY t.id ASC",
    )?;
    let tasks = stmt
        .query_map(params![today_str], row_to_task)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

pub fn delete_reminds_for_task(conn: &Connection, task_id: u32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM task_reminds WHERE task_id = ?1",
        params![task_id],
    )?;
    Ok(())
}

pub fn delete_remind(
    conn: &Connection,
    task_id: u32,
    remind_at: NaiveDate,
) -> Result<usize, rusqlite::Error> {
    let remind_str = remind_at.to_string();
    let deleted = conn.execute(
        "DELETE FROM task_reminds WHERE task_id = ?1 AND remind_at = ?2",
        params![task_id, remind_str],
    )?;
    Ok(deleted)
}

pub fn search_tasks(
    conn: &Connection,
    keyword: &str,
    all: bool,
    project: Option<&str>,
) -> Result<Vec<Task>, rusqlite::Error> {
    let base = tasks_select_sql();
    let mut conditions = vec!["title LIKE ?1".to_string()];
    let like_pattern = format!("%{}%", keyword);

    if !all {
        conditions.push("t.status = 'open'".to_string());
    }
    if project.is_some() {
        conditions.push("p.name = ?2".to_string());
    }

    let where_clause = format!(" WHERE {}", conditions.join(" AND "));
    let sql = format!("{}{} ORDER BY t.id ASC", base, where_clause);

    let mut stmt = conn.prepare(&sql)?;
    let tasks: Vec<Task> = if let Some(p) = project {
        stmt.query_map(params![like_pattern, p], row_to_task)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![like_pattern], row_to_task)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(tasks)
}

pub struct ProjectSummary {
    pub name: String,
    pub category: Option<String>,
    pub open_count: u32,
    pub done_count: u32,
    pub closed_count: u32,
}

/// Set (or clear) the category of an existing project by name.
///
/// A `category` of `None` (or a value that is empty after trimming) clears the
/// category (sets it to NULL). The project is never created implicitly: if no
/// project with `project` exists, this returns `Ok(false)` and makes no change.
/// On success it returns `Ok(true)`.
pub fn set_project_category(
    conn: &Connection,
    project: &str,
    category: Option<&str>,
) -> Result<bool, rusqlite::Error> {
    let normalized: Option<String> = category
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string());
    let affected = conn.execute(
        "UPDATE projects SET category = ?1 WHERE name = ?2",
        params![normalized, project],
    )?;
    Ok(affected > 0)
}

pub fn list_projects(conn: &Connection) -> Result<Vec<ProjectSummary>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT p.name,
                p.category,
                SUM(CASE WHEN t.status = 'open' THEN 1 ELSE 0 END) AS open_count,
                SUM(CASE WHEN t.status = 'done' THEN 1 ELSE 0 END) AS done_count,
                SUM(CASE WHEN t.status = 'closed' THEN 1 ELSE 0 END) AS closed_count
         FROM projects p
         LEFT JOIN tasks t ON p.id = t.project_id
         GROUP BY p.id, p.name
         ORDER BY p.name ASC",
    )?;
    let projects = stmt
        .query_map([], |row| {
            Ok(ProjectSummary {
                name: row.get(0)?,
                category: row.get(1)?,
                open_count: row.get(2)?,
                done_count: row.get(3)?,
                closed_count: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(projects)
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("invalid date in database")
}

/// Parse a datetime stored in the `%Y-%m-%d %H:%M:%S` format. For backward
/// compatibility, a date-only value (`%Y-%m-%d`, 10 chars) is accepted and
/// treated as midnight (`00:00:00`).
fn parse_datetime(s: &str) -> NaiveDateTime {
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return dt;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .expect("invalid datetime in database")
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
    let important_int: i32 = row.get(9)?;

    Ok(Task {
        id,
        title,
        status: Status::from_str(&status_str),
        source,
        project,
        due: due_str.map(|s| parse_date(&s)),
        done_at: done_at_str.map(|s| parse_datetime(&s)),
        created: parse_datetime(&created_str),
        updated: parse_datetime(&updated_str),
        reminds: Vec::new(),
        important: important_int != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use tempfile::TempDir;

    fn open_in_memory() -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                name     TEXT    NOT NULL UNIQUE,
                category TEXT
            );
            CREATE TABLE IF NOT EXISTS tasks (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                title      TEXT    NOT NULL,
                status     TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
                source     TEXT    NOT NULL DEFAULT 'private',
                project_id INTEGER REFERENCES projects(id),
                due        TEXT,
                done_at    TEXT,
                created    TEXT    NOT NULL,
                updated    TEXT    NOT NULL,
                important  INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS task_reminds (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id  INTEGER NOT NULL REFERENCES tasks(id),
                remind_at TEXT NOT NULL
            );",
        )?;
        Ok(conn)
    }

    fn legacy_db_path() -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("legacy.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE tasks (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                title     TEXT    NOT NULL,
                status    TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
                source    TEXT    NOT NULL DEFAULT 'private',
                project   TEXT,
                due       TEXT,
                done_at   TEXT,
                created   TEXT    NOT NULL,
                updated   TEXT    NOT NULL
            );
            INSERT INTO tasks (title, status, source, project, due, done_at, created, updated)
            VALUES ('Legacy task', 'open', 'private', 'legacy-project', NULL, NULL, '2026-03-31', '2026-03-31');",
        )
        .unwrap();
        drop(conn);
        (tmp, db_path)
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 31).unwrap()
    }

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 3, 31)
            .unwrap()
            .and_hms_opt(9, 30, 0)
            .unwrap()
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
    fn test_open_migrates_legacy_project_column() {
        let (_tmp, db_path) = legacy_db_path();
        let conn = open(&db_path).unwrap();

        let project_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM projects WHERE name = 'legacy-project'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(project_count, 1);

        let project_name: Option<String> = conn
            .query_row(
                "SELECT p.name
                 FROM tasks t
                 LEFT JOIN projects p ON t.project_id = p.id
                 WHERE t.id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(project_name, Some("legacy-project".to_string()));

        // The projects table must gain the `category` column after migration.
        assert!(table_has_column(&conn, "projects", "category").unwrap());
    }

    #[test]
    fn test_add_and_find() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Test task", Some("myproject"), None, now(), false).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "Test task");
        assert_eq!(task.project, Some("myproject".to_string()));
        assert_eq!(task.status, Status::Open);
        assert_eq!(task.created, now());
    }

    #[test]
    fn test_add_task_preserves_second_precision() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Timed task", None, None, now(), false).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        // The seconds component must survive the round-trip through the DB.
        assert_eq!(task.created, now());
        assert_eq!(task.updated, now());
        assert_eq!(task.created.format("%H:%M:%S").to_string(), "09:30:00");
    }

    #[test]
    fn test_complete_task() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Complete me", None, None, now(), false).unwrap();
        complete_task(&conn, id, now()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.status, Status::Done);
        assert_eq!(task.done_at, Some(now()));
    }

    #[test]
    fn test_complete_task_done_at_equals_updated() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Complete me", None, None, now(), false).unwrap();
        let done_time = NaiveDate::from_ymd_opt(2026, 4, 1)
            .unwrap()
            .and_hms_opt(14, 5, 6)
            .unwrap();
        complete_task(&conn, id, done_time).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        // done_at and updated are written from the same value.
        assert_eq!(task.done_at, Some(done_time));
        assert_eq!(task.updated, done_time);
        assert_eq!(task.done_at, Some(task.updated));
    }

    #[test]
    fn test_update_task_title() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Old title", None, None, now(), false).unwrap();
        update_task(&conn, id, Some("New title"), None, None, now(), None).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "New title");
    }

    #[test]
    fn test_update_task_multiple_fields() {
        let conn = open_in_memory().unwrap();
        let due = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let id = add_task(&conn, "Task", None, None, now(), false).unwrap();
        update_task(
            &conn,
            id,
            Some("Updated"),
            Some("proj"),
            Some(due),
            now(),
            None,
        )
        .unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "Updated");
        assert_eq!(task.project, Some("proj".to_string()));
        assert_eq!(task.due, Some(due));
    }

    #[test]
    fn test_close_task() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Close me", None, None, now(), false).unwrap();
        close_task(&conn, id, now()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.status, Status::Closed);
    }

    #[test]
    fn test_find_task_not_found() {
        let conn = open_in_memory().unwrap();
        let result = find_task(&conn, 999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_due_tasks_returns_due_before_target() {
        let conn = open_in_memory().unwrap();
        let t = today();
        let past = NaiveDate::from_ymd_opt(2026, 3, 28).unwrap();
        let future = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

        add_task(&conn, "Overdue", None, Some(past), now(), false).unwrap();
        add_task(&conn, "Due today", None, Some(t), now(), false).unwrap();
        add_task(&conn, "Future", None, Some(future), now(), false).unwrap();
        add_task(&conn, "No due", None, None, now(), false).unwrap();

        let tasks = get_due_tasks(&conn, t).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Overdue");
        assert_eq!(tasks[1].title, "Due today");
    }

    #[test]
    fn test_get_due_tasks_excludes_non_open() {
        let conn = open_in_memory().unwrap();
        let t = today();

        let id1 = add_task(&conn, "Done task", None, Some(t), now(), false).unwrap();
        complete_task(&conn, id1, now()).unwrap();
        let id2 = add_task(&conn, "Closed task", None, Some(t), now(), false).unwrap();
        close_task(&conn, id2, now()).unwrap();
        add_task(&conn, "Open task", None, Some(t), now(), false).unwrap();

        let tasks = get_due_tasks(&conn, t).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Open task");
    }

    #[test]
    fn test_get_due_tasks_excludes_null_due() {
        let conn = open_in_memory().unwrap();
        let t = today();

        add_task(&conn, "No due", None, None, now(), false).unwrap();

        let tasks = get_due_tasks(&conn, t).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_add_and_get_reminds() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Remind me", None, None, now(), false).unwrap();

        let r1 = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let r2 = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        add_remind(&conn, id, r1).unwrap();
        add_remind(&conn, id, r2).unwrap();

        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert_eq!(reminds.len(), 2);
        assert_eq!(reminds[0], r1);
        assert_eq!(reminds[1], r2);
    }

    #[test]
    fn test_get_reminds_empty() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "No remind", None, None, now(), false).unwrap();

        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert!(reminds.is_empty());
    }

    #[test]
    fn test_get_tasks_with_remind_today() {
        let conn = open_in_memory().unwrap();
        let t = today();

        let id1 = add_task(&conn, "Remind today", None, None, now(), false).unwrap();
        add_remind(&conn, id1, t).unwrap();

        let tomorrow = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let id2 = add_task(&conn, "Remind tomorrow", None, None, now(), false).unwrap();
        add_remind(&conn, id2, tomorrow).unwrap();

        let id3 = add_task(&conn, "No remind", None, None, now(), false).unwrap();
        let _ = id3;

        let tasks = get_tasks_with_remind_today(&conn, t).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Remind today");
    }

    #[test]
    fn test_get_tasks_with_remind_today_excludes_done() {
        let conn = open_in_memory().unwrap();
        let t = today();

        let id1 = add_task(&conn, "Done task", None, None, now(), false).unwrap();
        add_remind(&conn, id1, t).unwrap();
        complete_task(&conn, id1, now()).unwrap();

        let id2 = add_task(&conn, "Closed task", None, None, now(), false).unwrap();
        add_remind(&conn, id2, t).unwrap();
        close_task(&conn, id2, now()).unwrap();

        let id3 = add_task(&conn, "Open task", None, None, now(), false).unwrap();
        add_remind(&conn, id3, t).unwrap();

        let tasks = get_tasks_with_remind_today(&conn, t).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Open task");
    }

    #[test]
    fn test_delete_reminds_for_task() {
        let conn = open_in_memory().unwrap();
        let t = today();
        let id = add_task(&conn, "Task", None, None, now(), false).unwrap();
        add_remind(&conn, id, t).unwrap();

        delete_reminds_for_task(&conn, id).unwrap();
        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert!(reminds.is_empty());
    }

    #[test]
    fn test_delete_remind_single() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, now(), false).unwrap();

        let r1 = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        let r2 = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        add_remind(&conn, id, r1).unwrap();
        add_remind(&conn, id, r2).unwrap();

        let deleted = delete_remind(&conn, id, r1).unwrap();
        assert_eq!(deleted, 1);

        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert_eq!(reminds.len(), 1);
        assert_eq!(reminds[0], r2);
        assert!(!reminds.contains(&r1));
    }

    #[test]
    fn test_delete_remind_not_found() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, now(), false).unwrap();

        let r1 = NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
        add_remind(&conn, id, r1).unwrap();

        let missing = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
        let deleted = delete_remind(&conn, id, missing).unwrap();
        assert_eq!(deleted, 0);

        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert_eq!(reminds.len(), 1);
        assert_eq!(reminds[0], r1);
    }

    #[test]
    fn test_add_task_with_important_true() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Important task", None, None, now(), true).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(task.important);
    }

    #[test]
    fn test_add_task_with_important_false() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Normal task", None, None, now(), false).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(!task.important);
    }

    #[test]
    fn test_update_task_important() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, now(), false).unwrap();
        update_task(&conn, id, None, None, None, now(), Some(true)).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(task.important);
    }

    #[test]
    fn test_update_task_remove_important() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, now(), true).unwrap();
        update_task(&conn, id, None, None, None, now(), Some(false)).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(!task.important);
    }

    #[test]
    fn test_update_task_important_none_unchanged() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, now(), true).unwrap();
        update_task(&conn, id, Some("New title"), None, None, now(), None).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "New title");
        assert!(task.important);
    }

    #[test]
    fn test_list_tasks_important_only() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Normal", None, None, t, false).unwrap();
        add_task(&conn, "Important", None, None, t, true).unwrap();
        add_task(&conn, "Also normal", None, None, t, false).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            None,
            None,
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            true,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Important");
        assert!(tasks[0].important);
    }

    #[test]
    fn test_search_tasks_by_keyword() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Buy groceries", None, None, t, false).unwrap();
        add_task(&conn, "Write report", None, None, t, false).unwrap();
        add_task(&conn, "Buy flowers", None, None, t, false).unwrap();

        let tasks = search_tasks(&conn, "Buy", false, None).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Buy groceries");
        assert_eq!(tasks[1].title, "Buy flowers");
    }

    #[test]
    fn test_search_tasks_open_only() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let id1 = add_task(&conn, "Open task match", None, None, t, false).unwrap();
        let _ = id1;
        let id2 = add_task(&conn, "Done task match", None, None, t, false).unwrap();
        complete_task(&conn, id2, t).unwrap();

        let tasks = search_tasks(&conn, "match", false, None).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Open task match");
    }

    #[test]
    fn test_search_tasks_all() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Open task match", None, None, t, false).unwrap();
        let id2 = add_task(&conn, "Done task match", None, None, t, false).unwrap();
        complete_task(&conn, id2, t).unwrap();

        let tasks = search_tasks(&conn, "match", true, None).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_search_tasks_with_project() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Task alpha", Some("alpha"), None, t, false).unwrap();
        add_task(&conn, "Task beta", Some("beta"), None, t, false).unwrap();
        add_task(&conn, "Task alpha2", Some("alpha"), None, t, false).unwrap();

        let tasks = search_tasks(&conn, "Task", false, Some("alpha")).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Task alpha");
        assert_eq!(tasks[1].title, "Task alpha2");
    }

    #[test]
    fn test_search_tasks_no_match() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Some task", None, None, t, false).unwrap();

        let tasks = search_tasks(&conn, "nonexistent", false, None).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_search_tasks_case_insensitive() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Buy Milk", None, None, t, false).unwrap();
        add_task(&conn, "buy bread", None, None, t, false).unwrap();

        // SQLite LIKE is case-insensitive for ASCII by default
        let tasks = search_tasks(&conn, "buy", false, None).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_list_tasks_important_only_false() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Normal", None, None, t, false).unwrap();
        add_task(&conn, "Important", None, None, t, true).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            None,
            None,
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_list_tasks_filter_by_due() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let target = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        let other = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();

        add_task(&conn, "Due target", None, Some(target), t, false).unwrap();
        add_task(&conn, "Due other", None, Some(other), t, false).unwrap();
        add_task(&conn, "No due", None, None, t, false).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            None,
            None,
            Some(target),
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Due target");
        assert_eq!(tasks[0].due, Some(target));
    }

    #[test]
    fn test_list_tasks_due_and_project() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let target = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();

        // Same due, different projects.
        add_task(&conn, "Alpha due", Some("alpha"), Some(target), t, false).unwrap();
        add_task(&conn, "Beta due", Some("beta"), Some(target), t, false).unwrap();
        // Same project, different due.
        let other = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
        add_task(&conn, "Alpha other", Some("alpha"), Some(other), t, false).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            Some("alpha"),
            None,
            Some(target),
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Alpha due");
    }

    #[test]
    fn test_list_tasks_due_with_all() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let target = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();

        let done_id = add_task(&conn, "Done due", None, Some(target), t, false).unwrap();
        complete_task(&conn, done_id, t).unwrap();
        let closed_id = add_task(&conn, "Closed due", None, Some(target), t, false).unwrap();
        close_task(&conn, closed_id, t).unwrap();
        add_task(&conn, "Open due", None, Some(target), t, false).unwrap();

        // all = false: only the open task is returned.
        let open_only = list_tasks(
            &conn,
            false,
            None,
            None,
            Some(target),
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(open_only.len(), 1);
        assert_eq!(open_only[0].title, "Open due");

        // all = true: done/closed tasks with matching due are also returned.
        let with_all = list_tasks(
            &conn,
            true,
            None,
            None,
            Some(target),
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(with_all.len(), 3);
    }

    #[test]
    fn test_list_tasks_due_none_unfiltered() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let target = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();

        add_task(&conn, "Has due", None, Some(target), t, false).unwrap();
        add_task(&conn, "No due", None, None, t, false).unwrap();

        // due = None means no due filter: all open tasks returned (backward compat).
        let tasks = list_tasks(
            &conn,
            false,
            None,
            None,
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_list_tasks_due_no_match() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let target = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        let missing = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();

        add_task(&conn, "Has due", None, Some(target), t, false).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            None,
            None,
            Some(missing),
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_list_projects_empty() {
        let conn = open_in_memory().unwrap();
        let projects = list_projects(&conn).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_list_projects_with_counts() {
        let conn = open_in_memory().unwrap();
        let t = now();

        add_task(&conn, "Open 1", Some("alpha"), None, t, false).unwrap();
        add_task(&conn, "Open 2", Some("alpha"), None, t, false).unwrap();
        let id3 = add_task(&conn, "Done", Some("alpha"), None, t, false).unwrap();
        complete_task(&conn, id3, t).unwrap();
        add_task(&conn, "Beta task", Some("beta"), None, t, false).unwrap();
        set_project_category(&conn, "alpha", Some("work")).unwrap();

        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects.len(), 2);

        assert_eq!(projects[0].name, "alpha");
        // list_projects surfaces the category (None until set).
        assert_eq!(projects[0].category, Some("work".to_string()));
        assert_eq!(projects[1].category, None);
        assert_eq!(projects[0].open_count, 2);
        assert_eq!(projects[0].done_count, 1);
        assert_eq!(projects[0].closed_count, 0);

        assert_eq!(projects[1].name, "beta");
        assert_eq!(projects[1].open_count, 1);
        assert_eq!(projects[1].done_count, 0);
    }

    #[test]
    fn test_list_projects_excludes_no_project_tasks() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "No project", None, None, t, false).unwrap();
        add_task(&conn, "Has project", Some("proj"), None, t, false).unwrap();

        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "proj");
    }

    // ----- project category -----

    #[test]
    fn test_migrate_projects_schema_adds_category_column() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("no_category.db");
        // Create a projects table WITHOUT the category column, mimicking an
        // older schema.
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE projects (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT    NOT NULL UNIQUE
            );
            INSERT INTO projects (name) VALUES ('legacy');",
        )
        .unwrap();
        assert!(!table_has_column(&conn, "projects", "category").unwrap());
        drop(conn);

        // open() runs the migration and adds the category column.
        let conn = open(&db_path).unwrap();
        assert!(table_has_column(&conn, "projects", "category").unwrap());
    }

    #[test]
    fn test_migrate_projects_schema_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("category.db");

        // First open creates the schema (with category) and sets a value.
        let conn = open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO projects (name, category) VALUES ('job', 'work')",
            [],
        )
        .unwrap();
        drop(conn);

        // Opening again must not error and must preserve the value.
        let conn = open(&db_path).unwrap();
        assert!(table_has_column(&conn, "projects", "category").unwrap());
        let category: Option<String> = conn
            .query_row(
                "SELECT category FROM projects WHERE name = 'job'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(category, Some("work".to_string()));
    }

    #[test]
    fn test_set_project_category_sets_and_lists() {
        let conn = open_in_memory().unwrap();
        add_task(&conn, "Task", Some("job"), None, now(), false).unwrap();

        let ok = set_project_category(&conn, "job", Some("work")).unwrap();
        assert!(ok);

        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "job");
        assert_eq!(projects[0].category, Some("work".to_string()));
    }

    #[test]
    fn test_set_project_category_clears_with_none() {
        let conn = open_in_memory().unwrap();
        add_task(&conn, "Task", Some("job"), None, now(), false).unwrap();
        set_project_category(&conn, "job", Some("work")).unwrap();

        // None clears the category (NULL).
        let ok = set_project_category(&conn, "job", None).unwrap();
        assert!(ok);

        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects[0].category, None);
    }

    #[test]
    fn test_set_project_category_clears_with_empty_string() {
        let conn = open_in_memory().unwrap();
        add_task(&conn, "Task", Some("job"), None, now(), false).unwrap();
        set_project_category(&conn, "job", Some("work")).unwrap();

        // A blank (whitespace-only) category also clears it.
        let ok = set_project_category(&conn, "job", Some("   ")).unwrap();
        assert!(ok);

        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects[0].category, None);
    }

    #[test]
    fn test_set_project_category_missing_project_returns_false() {
        let conn = open_in_memory().unwrap();
        // No project named "ghost" exists and none is created implicitly.
        let ok = set_project_category(&conn, "ghost", Some("work")).unwrap();
        assert!(!ok);

        let projects = list_projects(&conn).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_list_tasks_filter_by_category() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Job task", Some("job"), None, t, false).unwrap();
        add_task(&conn, "Hobby task", Some("hobby"), None, t, false).unwrap();
        set_project_category(&conn, "job", Some("work")).unwrap();
        set_project_category(&conn, "hobby", Some("fun")).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            None,
            Some("work"),
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Job task");
    }

    #[test]
    fn test_list_tasks_category_and_project_and() {
        let conn = open_in_memory().unwrap();
        let t = now();
        // Two projects share the same category.
        add_task(&conn, "Alpha task", Some("alpha"), None, t, false).unwrap();
        add_task(&conn, "Beta task", Some("beta"), None, t, false).unwrap();
        set_project_category(&conn, "alpha", Some("work")).unwrap();
        set_project_category(&conn, "beta", Some("work")).unwrap();

        // category = work matches both, project = alpha narrows to one (AND).
        let tasks = list_tasks(
            &conn,
            false,
            Some("alpha"),
            Some("work"),
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Alpha task");
    }

    #[test]
    fn test_list_tasks_category_none_returns_all() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Job task", Some("job"), None, t, false).unwrap();
        add_task(&conn, "Plain task", None, None, t, false).unwrap();
        set_project_category(&conn, "job", Some("work")).unwrap();

        // category = None means no category filter (backward compat).
        let tasks = list_tasks(
            &conn,
            false,
            None,
            None,
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_list_tasks_category_no_match() {
        let conn = open_in_memory().unwrap();
        let t = now();
        add_task(&conn, "Job task", Some("job"), None, t, false).unwrap();
        set_project_category(&conn, "job", Some("work")).unwrap();

        let tasks = list_tasks(
            &conn,
            false,
            None,
            Some("nonexistent"),
            None,
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_list_tasks_category_and_due_and() {
        let conn = open_in_memory().unwrap();
        let t = now();
        let target = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
        let other = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();

        // Same category, different dues: only the matching-due task should pass.
        add_task(
            &conn,
            "Work due target",
            Some("job"),
            Some(target),
            t,
            false,
        )
        .unwrap();
        add_task(&conn, "Work due other", Some("job"), Some(other), t, false).unwrap();
        // Matching due but a different category: must be excluded by the AND.
        add_task(
            &conn,
            "Fun due target",
            Some("hobby"),
            Some(target),
            t,
            false,
        )
        .unwrap();
        set_project_category(&conn, "job", Some("work")).unwrap();
        set_project_category(&conn, "hobby", Some("fun")).unwrap();

        // category = work AND due = target narrows to exactly one task.
        let tasks = list_tasks(
            &conn,
            false,
            None,
            Some("work"),
            Some(target),
            &[SortKey::Id],
            SortOrder::Asc,
            false,
        )
        .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Work due target");
    }

    // ----- parse_datetime -----

    #[test]
    fn test_parse_datetime_full() {
        let dt = parse_datetime("2026-03-01 12:34:56");
        let expected = NaiveDate::from_ymd_opt(2026, 3, 1)
            .unwrap()
            .and_hms_opt(12, 34, 56)
            .unwrap();
        assert_eq!(dt, expected);
    }

    #[test]
    fn test_parse_datetime_date_only_fallback() {
        // A legacy date-only value is treated as midnight.
        let dt = parse_datetime("2026-03-01");
        let expected = NaiveDate::from_ymd_opt(2026, 3, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(dt, expected);
    }

    // ----- normalize_datetime_columns / migration backward compatibility -----

    /// Build a current-schema DB that contains date-only (10-char) values in the
    /// datetime columns, mimicking data written by an older version. Returns the
    /// path so the test can `open()` it and exercise normalization.
    fn date_only_db_path() -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("date_only.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE projects (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                name     TEXT    NOT NULL UNIQUE,
                category TEXT
            );
            CREATE TABLE tasks (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                title      TEXT    NOT NULL,
                status     TEXT    NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'done', 'closed')),
                source     TEXT    NOT NULL DEFAULT 'private',
                project_id INTEGER REFERENCES projects(id),
                due        TEXT,
                done_at    TEXT,
                created    TEXT    NOT NULL,
                updated    TEXT    NOT NULL,
                important  INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE task_reminds (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id  INTEGER NOT NULL REFERENCES tasks(id),
                remind_at TEXT NOT NULL
            );
            -- Task 1: open, with date-only created/updated and a due date.
            INSERT INTO tasks (id, title, status, source, due, done_at, created, updated)
            VALUES (1, 'Old open', 'open', 'private', '2026-04-10', NULL, '2026-03-01', '2026-03-02');
            -- Task 2: done, with a date-only done_at.
            INSERT INTO tasks (id, title, status, source, due, done_at, created, updated)
            VALUES (2, 'Old done', 'done', 'private', NULL, '2026-03-05', '2026-03-01', '2026-03-05');
            INSERT INTO task_reminds (task_id, remind_at) VALUES (1, '2026-04-08');",
        )
        .unwrap();
        drop(conn);
        (tmp, db_path)
    }

    fn column_value(conn: &Connection, sql: &str) -> Option<String> {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn test_open_normalizes_date_only_datetime_columns() {
        let (_tmp, db_path) = date_only_db_path();
        let conn = open(&db_path).unwrap();

        assert_eq!(
            column_value(&conn, "SELECT created FROM tasks WHERE id = 1"),
            Some("2026-03-01 00:00:00".to_string())
        );
        assert_eq!(
            column_value(&conn, "SELECT updated FROM tasks WHERE id = 1"),
            Some("2026-03-02 00:00:00".to_string())
        );
        // done_at is NULL on task 1: it must not be touched or error.
        assert_eq!(
            column_value(&conn, "SELECT done_at FROM tasks WHERE id = 1"),
            None
        );
        // done_at on task 2 is normalized.
        assert_eq!(
            column_value(&conn, "SELECT done_at FROM tasks WHERE id = 2"),
            Some("2026-03-05 00:00:00".to_string())
        );

        // due and remind_at stay date-only.
        assert_eq!(
            column_value(&conn, "SELECT due FROM tasks WHERE id = 1"),
            Some("2026-04-10".to_string())
        );
        assert_eq!(
            column_value(
                &conn,
                "SELECT remind_at FROM task_reminds WHERE task_id = 1"
            ),
            Some("2026-04-08".to_string())
        );

        // The values round-trip into NaiveDateTime via row_to_task.
        let task = find_task(&conn, 1).unwrap().expect("task should exist");
        assert_eq!(
            task.created,
            NaiveDate::from_ymd_opt(2026, 3, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        );
        assert_eq!(task.due, NaiveDate::from_ymd_opt(2026, 4, 10));
    }

    #[test]
    fn test_open_normalization_is_idempotent() {
        let (_tmp, db_path) = date_only_db_path();
        // First open normalizes to 19-char datetimes.
        let conn = open(&db_path).unwrap();
        let created_after_first =
            column_value(&conn, "SELECT created FROM tasks WHERE id = 1").unwrap();
        assert_eq!(created_after_first.len(), 19);
        drop(conn);

        // Opening again must not change the already-normalized 19-char values.
        let conn = open(&db_path).unwrap();
        let created_after_second =
            column_value(&conn, "SELECT created FROM tasks WHERE id = 1").unwrap();
        assert_eq!(created_after_second, created_after_first);

        let updated_after_second =
            column_value(&conn, "SELECT updated FROM tasks WHERE id = 1").unwrap();
        assert_eq!(updated_after_second, "2026-03-02 00:00:00");

        // due and remind_at remain date-only across repeated opens.
        assert_eq!(
            column_value(&conn, "SELECT due FROM tasks WHERE id = 1"),
            Some("2026-04-10".to_string())
        );
        assert_eq!(
            column_value(
                &conn,
                "SELECT remind_at FROM task_reminds WHERE task_id = 1"
            ),
            Some("2026-04-08".to_string())
        );
    }
}
