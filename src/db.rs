use crate::model::{SortKey, SortOrder, Status, Task};
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
        "CREATE TABLE IF NOT EXISTS projects (
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            name  TEXT    NOT NULL UNIQUE
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
    Ok(conn)
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
                id    INTEGER PRIMARY KEY AUTOINCREMENT,
                name  TEXT    NOT NULL UNIQUE
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
    today: NaiveDate,
    important: bool,
) -> Result<u32, rusqlite::Error> {
    let today_str = today.to_string();
    let due_str = due.map(|d| d.to_string());
    let project_id = resolve_project_id(conn, project)?;
    conn.execute(
        "INSERT INTO tasks (title, source, project_id, due, created, updated, important)
         VALUES (?1, 'private', ?2, ?3, ?4, ?4, ?5)",
        params![title, project_id, due_str, today_str, important as i32],
    )?;
    Ok(conn.last_insert_rowid() as u32)
}

pub fn find_task(conn: &Connection, id: u32) -> Result<Option<Task>, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("{} WHERE t.id = ?1", tasks_select_sql()))?;
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
    sorts: &[SortKey],
    order: SortOrder,
    important_only: bool,
) -> Result<Vec<Task>, rusqlite::Error> {
    let base = tasks_select_sql();
    let mut conditions = Vec::new();
    if !all {
        conditions.push("t.status = 'open'");
    }
    if project.is_some() {
        conditions.push("p.name = ?1");
    }
    if important_only {
        conditions.push("t.important = 1");
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
    important: Option<bool>,
) -> Result<(), rusqlite::Error> {
    let today_str = today.to_string();
    let mut sets: Vec<String> = vec!["updated = ?1".to_string()];
    let mut param_idx = 2u32;
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(today_str)];

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
    let important_int: i32 = row.get(9)?;

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
                id    INTEGER PRIMARY KEY AUTOINCREMENT,
                name  TEXT    NOT NULL UNIQUE
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
    }

    #[test]
    fn test_add_and_find() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Test task", Some("myproject"), None, today(), false).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "Test task");
        assert_eq!(task.project, Some("myproject".to_string()));
        assert_eq!(task.status, Status::Open);
        assert_eq!(task.created, today());
    }

    #[test]
    fn test_complete_task() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Complete me", None, None, today(), false).unwrap();
        complete_task(&conn, id, today()).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.status, Status::Done);
        assert_eq!(task.done_at, Some(today()));
    }

    #[test]
    fn test_update_task_title() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Old title", None, None, today(), false).unwrap();
        update_task(&conn, id, Some("New title"), None, None, today(), None).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "New title");
    }

    #[test]
    fn test_update_task_multiple_fields() {
        let conn = open_in_memory().unwrap();
        let due = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
        let id = add_task(&conn, "Task", None, None, today(), false).unwrap();
        update_task(
            &conn,
            id,
            Some("Updated"),
            Some("proj"),
            Some(due),
            today(),
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
        let id = add_task(&conn, "Close me", None, None, today(), false).unwrap();
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

    #[test]
    fn test_get_due_tasks_returns_due_before_target() {
        let conn = open_in_memory().unwrap();
        let t = today();
        let past = NaiveDate::from_ymd_opt(2026, 3, 28).unwrap();
        let future = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();

        add_task(&conn, "Overdue", None, Some(past), t, false).unwrap();
        add_task(&conn, "Due today", None, Some(t), t, false).unwrap();
        add_task(&conn, "Future", None, Some(future), t, false).unwrap();
        add_task(&conn, "No due", None, None, t, false).unwrap();

        let tasks = get_due_tasks(&conn, t).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Overdue");
        assert_eq!(tasks[1].title, "Due today");
    }

    #[test]
    fn test_get_due_tasks_excludes_non_open() {
        let conn = open_in_memory().unwrap();
        let t = today();

        let id1 = add_task(&conn, "Done task", None, Some(t), t, false).unwrap();
        complete_task(&conn, id1, t).unwrap();
        let id2 = add_task(&conn, "Closed task", None, Some(t), t, false).unwrap();
        close_task(&conn, id2, t).unwrap();
        add_task(&conn, "Open task", None, Some(t), t, false).unwrap();

        let tasks = get_due_tasks(&conn, t).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Open task");
    }

    #[test]
    fn test_get_due_tasks_excludes_null_due() {
        let conn = open_in_memory().unwrap();
        let t = today();

        add_task(&conn, "No due", None, None, t, false).unwrap();

        let tasks = get_due_tasks(&conn, t).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_add_and_get_reminds() {
        let conn = open_in_memory().unwrap();
        let t = today();
        let id = add_task(&conn, "Remind me", None, None, t, false).unwrap();

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
        let t = today();
        let id = add_task(&conn, "No remind", None, None, t, false).unwrap();

        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert!(reminds.is_empty());
    }

    #[test]
    fn test_get_tasks_with_remind_today() {
        let conn = open_in_memory().unwrap();
        let t = today();

        let id1 = add_task(&conn, "Remind today", None, None, t, false).unwrap();
        add_remind(&conn, id1, t).unwrap();

        let tomorrow = NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
        let id2 = add_task(&conn, "Remind tomorrow", None, None, t, false).unwrap();
        add_remind(&conn, id2, tomorrow).unwrap();

        let id3 = add_task(&conn, "No remind", None, None, t, false).unwrap();
        let _ = id3;

        let tasks = get_tasks_with_remind_today(&conn, t).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Remind today");
    }

    #[test]
    fn test_get_tasks_with_remind_today_excludes_done() {
        let conn = open_in_memory().unwrap();
        let t = today();

        let id1 = add_task(&conn, "Done task", None, None, t, false).unwrap();
        add_remind(&conn, id1, t).unwrap();
        complete_task(&conn, id1, t).unwrap();

        let id2 = add_task(&conn, "Closed task", None, None, t, false).unwrap();
        add_remind(&conn, id2, t).unwrap();
        close_task(&conn, id2, t).unwrap();

        let id3 = add_task(&conn, "Open task", None, None, t, false).unwrap();
        add_remind(&conn, id3, t).unwrap();

        let tasks = get_tasks_with_remind_today(&conn, t).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Open task");
    }

    #[test]
    fn test_delete_reminds_for_task() {
        let conn = open_in_memory().unwrap();
        let t = today();
        let id = add_task(&conn, "Task", None, None, t, false).unwrap();
        add_remind(&conn, id, t).unwrap();

        delete_reminds_for_task(&conn, id).unwrap();
        let reminds = get_reminds_for_task(&conn, id).unwrap();
        assert!(reminds.is_empty());
    }

    #[test]
    fn test_add_task_with_important_true() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Important task", None, None, today(), true).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(task.important);
    }

    #[test]
    fn test_add_task_with_important_false() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Normal task", None, None, today(), false).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(!task.important);
    }

    #[test]
    fn test_update_task_important() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, today(), false).unwrap();
        update_task(&conn, id, None, None, None, today(), Some(true)).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(task.important);
    }

    #[test]
    fn test_update_task_remove_important() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, today(), true).unwrap();
        update_task(&conn, id, None, None, None, today(), Some(false)).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert!(!task.important);
    }

    #[test]
    fn test_update_task_important_none_unchanged() {
        let conn = open_in_memory().unwrap();
        let id = add_task(&conn, "Task", None, None, today(), true).unwrap();
        update_task(&conn, id, Some("New title"), None, None, today(), None).unwrap();
        let task = find_task(&conn, id).unwrap().expect("task should exist");
        assert_eq!(task.title, "New title");
        assert!(task.important);
    }

    #[test]
    fn test_list_tasks_important_only() {
        let conn = open_in_memory().unwrap();
        let t = today();
        add_task(&conn, "Normal", None, None, t, false).unwrap();
        add_task(&conn, "Important", None, None, t, true).unwrap();
        add_task(&conn, "Also normal", None, None, t, false).unwrap();

        let tasks = list_tasks(&conn, false, None, &[SortKey::Id], SortOrder::Asc, true).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Important");
        assert!(tasks[0].important);
    }

    #[test]
    fn test_search_tasks_by_keyword() {
        let conn = open_in_memory().unwrap();
        let t = today();
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
        let t = today();
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
        let t = today();
        add_task(&conn, "Open task match", None, None, t, false).unwrap();
        let id2 = add_task(&conn, "Done task match", None, None, t, false).unwrap();
        complete_task(&conn, id2, t).unwrap();

        let tasks = search_tasks(&conn, "match", true, None).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_search_tasks_with_project() {
        let conn = open_in_memory().unwrap();
        let t = today();
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
        let t = today();
        add_task(&conn, "Some task", None, None, t, false).unwrap();

        let tasks = search_tasks(&conn, "nonexistent", false, None).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_search_tasks_case_insensitive() {
        let conn = open_in_memory().unwrap();
        let t = today();
        add_task(&conn, "Buy Milk", None, None, t, false).unwrap();
        add_task(&conn, "buy bread", None, None, t, false).unwrap();

        // SQLite LIKE is case-insensitive for ASCII by default
        let tasks = search_tasks(&conn, "buy", false, None).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_list_tasks_important_only_false() {
        let conn = open_in_memory().unwrap();
        let t = today();
        add_task(&conn, "Normal", None, None, t, false).unwrap();
        add_task(&conn, "Important", None, None, t, true).unwrap();

        let tasks = list_tasks(&conn, false, None, &[SortKey::Id], SortOrder::Asc, false).unwrap();
        assert_eq!(tasks.len(), 2);
    }
}
