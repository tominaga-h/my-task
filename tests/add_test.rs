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
fn test_add_empty_title() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: title cannot be empty"));
}
