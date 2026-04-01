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
