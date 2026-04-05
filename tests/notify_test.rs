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
    conn.execute(
        "INSERT INTO tasks (title, status, source, project, due, created, updated)
         VALUES (?1, ?2, 'private', ?3, ?4, '2026-03-01', '2026-03-01')",
        rusqlite::params![title, status, project, due],
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
