use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_add_basic() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Test task"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1 Test task"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let title: String = conn
        .query_row("SELECT title FROM tasks WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    assert_eq!(title, "Test task");
}

#[test]
fn test_add_with_options() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args([
            "add",
            "With opts",
            "--project",
            "myproj",
            "--due",
            "2026-04-15",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1 With opts"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let (project, due): (Option<String>, Option<String>) = conn
        .query_row("SELECT project, due FROM tasks WHERE id = 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(project, Some("myproj".to_string()));
    assert_eq!(due, Some("2026-04-15".to_string()));
}

#[test]
fn test_add_auto_id() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "First"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1"));

    cmd(&db_path)
        .args(["add", "Second"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #2"));
}

#[test]
fn test_add_fuzzy_due_today() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Fuzzy task", "--due", "今日"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let due: Option<String> = conn
        .query_row("SELECT due FROM tasks WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(due, Some(today));
}

#[test]
fn test_add_fuzzy_due_tomorrow() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Tomorrow task", "--due", "明日"])
        .assert()
        .success();

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let due: Option<String> = conn
        .query_row("SELECT due FROM tasks WHERE id = 1", [], |row| row.get(0))
        .unwrap();
    let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1)).to_string();
    assert_eq!(due, Some(tomorrow));
}

#[test]
fn test_add_invalid_due() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Bad due", "--due", "aaaa"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid due date"));
}

#[test]
fn test_add_with_remind() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Remind task", "--remind", "2026-04-10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1 Remind task"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let remind: String = conn
        .query_row(
            "SELECT remind_at FROM task_reminds WHERE task_id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remind, "2026-04-10");
}

#[test]
fn test_add_with_remind_fuzzy() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Remind tomorrow", "--remind", "明日"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let remind: String = conn
        .query_row(
            "SELECT remind_at FROM task_reminds WHERE task_id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1)).to_string();
    assert_eq!(remind, tomorrow);
}

#[test]
fn test_add_with_remind_invalid() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Bad remind", "--remind", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid remind date"));
}

#[test]
fn test_add_with_remind_short_flag() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Short flag", "-r", "2026-05-01"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let remind: String = conn
        .query_row(
            "SELECT remind_at FROM task_reminds WHERE task_id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remind, "2026-05-01");
}

#[test]
fn test_add_empty_title() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: title cannot be empty"));
}

#[test]
fn test_add_with_important() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Important task", "--important"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1 Important task"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let important: i32 = conn
        .query_row("SELECT important FROM tasks WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(important, 1);
}

#[test]
fn test_add_without_important() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Normal task"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Added: #1 Normal task"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let important: i32 = conn
        .query_row("SELECT important FROM tasks WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(important, 0);
}
