use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_edit_title() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Original title"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "--title", "New title"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1 New title"));

    cmd(&db_path)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New title"));
}

#[test]
fn test_edit_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Task", "--project", "old-proj"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "--project", "new-proj"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));

    cmd(&db_path)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("new-proj"));
}

#[test]
fn test_edit_due() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "--due", "2026-12-25"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));

    cmd(&db_path)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("12/25"));
}

#[test]
fn test_edit_multiple_fields() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args([
            "edit",
            "1",
            "--title",
            "Updated",
            "--project",
            "proj",
            "--due",
            "2026-06-01",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1 Updated"));
}

#[test]
fn test_edit_not_found() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "999", "--title", "New"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: task #999 not found"));
}

#[test]
fn test_edit_empty_title() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "--title", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("title cannot be empty"));
}

#[test]
fn test_edit_add_remind() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "--remind", "2026-04-10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));

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
fn test_edit_add_multiple_reminds() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "--remind", "2026-04-10"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "--remind", "2026-04-15"])
        .assert()
        .success();

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM task_reminds WHERE task_id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_edit_remind_only() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    // Should succeed with only --remind (no --title, --project, --due)
    cmd(&db_path)
        .args(["edit", "1", "--remind", "2026-04-10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));
}

#[test]
fn test_edit_remind_short_flag() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "-r", "2026-04-10"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));
}

#[test]
fn test_edit_no_flags() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "specify at least one field to edit",
        ));
}

#[test]
fn test_edit_set_important() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "--important"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let important: i32 = conn
        .query_row("SELECT important FROM tasks WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(important, 1);
}

#[test]
fn test_edit_unset_important() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Task", "--important"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "--no-important"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: #1"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let important: i32 = conn
        .query_row("SELECT important FROM tasks WHERE id = 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(important, 0);
}

#[test]
fn test_edit_important_conflict() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["edit", "1", "--important", "--no-important"])
        .assert()
        .failure();
}
