use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_show_basic() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "My task"]).assert().success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID: 1"))
        .stdout(predicate::str::contains("Title: My task"))
        .stdout(predicate::str::contains("Status: open"))
        .stdout(predicate::str::contains("Created:"))
        .stdout(predicate::str::contains("Updated:"));
}

#[test]
fn test_show_not_found() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Exists"]).assert().success();

    cmd(&db_path)
        .args(["show", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: task #999 not found"));
}

#[test]
fn test_show_with_project_and_due() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Project task", "-p", "myproject", "-d", "2026-04-15"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Project: myproject"))
        .stdout(predicate::str::contains("Due: 2026-04-15"));
}

#[test]
fn test_show_none_fields() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Simple task"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Project: (none)"))
        .stdout(predicate::str::contains("Due: (none)"))
        .stdout(predicate::str::contains("Remind: (none)"));
}

#[test]
fn test_show_important() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Important task", "--important"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Important: yes"));
}

#[test]
fn test_show_done_status() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Done task"]).assert().success();
    cmd(&db_path).args(["done", "1"]).assert().success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Status: done"));
}

#[test]
fn test_show_closed_status() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Closed task"])
        .assert()
        .success();
    cmd(&db_path).args(["close", "1"]).assert().success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Status: closed"));
}

#[test]
fn test_show_with_remind() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Remind task", "-r", "2026-04-10"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Remind: 2026-04-10"));
}

#[test]
fn test_show_no_args() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["show"]).assert().failure();
}
