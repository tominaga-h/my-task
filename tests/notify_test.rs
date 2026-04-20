use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

fn setup_db(db_path: &std::path::Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(db_path).unwrap();
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
    )
    .unwrap();
    conn
}

fn insert_task(
    conn: &rusqlite::Connection,
    title: &str,
    due: Option<&str>,
    status: &str,
    project: Option<&str>,
) {
    let project_id = if let Some(project) = project {
        conn.execute(
            "INSERT OR IGNORE INTO projects (name) VALUES (?1)",
            rusqlite::params![project],
        )
        .unwrap();
        Some(
            conn.query_row(
                "SELECT id FROM projects WHERE name = ?1",
                rusqlite::params![project],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        )
    } else {
        None
    };
    conn.execute(
        "INSERT INTO tasks (title, status, source, project_id, due, created, updated)
         VALUES (?1, ?2, 'private', ?3, ?4, '2026-03-01', '2026-03-01')",
        rusqlite::params![title, status, project_id, due],
    )
    .unwrap();
}

fn insert_remind(conn: &rusqlite::Connection, task_id: u32, remind_at: &str) {
    conn.execute(
        "INSERT INTO task_reminds (task_id, remind_at) VALUES (?1, ?2)",
        rusqlite::params![task_id, remind_at],
    )
    .unwrap();
}

fn today_str() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

fn days_ago(n: i64) -> String {
    (chrono::Local::now().date_naive() - chrono::Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

fn days_later(n: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn test_notify_no_tasks_silent() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["notify"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_notify_no_due_tasks_silent() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "No due date", None, "open", None);
    insert_task(&conn, "Future task", Some(&days_later(30)), "open", None);

    cmd(&db_path)
        .args(["notify"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_notify_overdue_task() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Overdue report", Some(&days_ago(3)), "open", None);

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("期限切れタスクがあります"));
    assert!(stdout.contains("Overdue report"));
    assert!(stdout.contains("3日超過"));
}

#[test]
fn test_notify_due_today() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Due today task", Some(&today_str()), "open", None);

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("期限切れタスクがあります"));
    assert!(stdout.contains("Due today task"));
    assert!(stdout.contains("今日"));
}

#[test]
fn test_notify_days_option() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Due in 2 days", Some(&days_later(2)), "open", None);
    insert_task(&conn, "Due in 5 days", Some(&days_later(5)), "open", None);

    cmd(&db_path)
        .args(["notify"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let output = cmd(&db_path)
        .args(["notify", "--days", "3"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Due in 2 days"));
    assert!(stdout.contains("あと2日"));
    assert!(!stdout.contains("Due in 5 days"));
}

#[test]
fn test_notify_excludes_done_and_closed() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Done task", Some(&today_str()), "done", None);
    insert_task(&conn, "Closed task", Some(&today_str()), "closed", None);
    insert_task(&conn, "Open task", Some(&today_str()), "open", None);

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Open task"));
    assert!(!stdout.contains("Done task"));
    assert!(!stdout.contains("Closed task"));
}

#[test]
fn test_notify_shows_project_column() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(
        &conn,
        "Project task",
        Some(&today_str()),
        "open",
        Some("my-task"),
    );

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Project"));
    assert!(stdout.contains("my-task"));
    assert!(stdout.contains("Project task"));
}

#[test]
fn test_notify_days_short_flag() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Due in 2 days", Some(&days_later(2)), "open", None);

    // -d 3 should include a task due in 2 days
    let output = cmd(&db_path).args(["notify", "-d", "3"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Due in 2 days"));
    assert!(stdout.contains("あと2日"));
}

#[test]
fn test_notify_mixed_projects() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "API task", Some(&today_str()), "open", Some("api"));
    insert_task(&conn, "No project task", Some(&days_ago(1)), "open", None);

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("api"));
    assert!(stdout.contains("API task"));
    assert!(stdout.contains("No project task"));
}

#[test]
fn test_notify_remind_today() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Remind me task", None, "open", Some("proj"));
    insert_remind(&conn, 1, &today_str());

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("リマインドタスクがあります"));
    assert!(stdout.contains("Remind me task"));
    assert!(stdout.contains("proj"));
}

#[test]
fn test_notify_remind_tomorrow_no_output() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Remind tomorrow", None, "open", None);
    insert_remind(&conn, 1, &days_later(1));

    cmd(&db_path)
        .args(["notify"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_notify_remind_excludes_done() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Done remind", None, "done", None);
    insert_remind(&conn, 1, &today_str());

    cmd(&db_path)
        .args(["notify"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_notify_remind_excludes_closed() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    insert_task(&conn, "Closed remind", None, "closed", None);
    insert_remind(&conn, 1, &today_str());

    cmd(&db_path)
        .args(["notify"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_notify_both_due_and_remind() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");
    let conn = setup_db(&db_path);

    // Due task
    insert_task(&conn, "Due task", Some(&today_str()), "open", None);
    // Remind task (different task)
    insert_task(&conn, "Remind task", None, "open", None);
    insert_remind(&conn, 2, &today_str());

    let output = cmd(&db_path).args(["notify"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("期限切れタスクがあります"));
    assert!(stdout.contains("Due task"));
    assert!(stdout.contains("リマインドタスクがあります"));
    assert!(stdout.contains("Remind task"));
}
