use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_close_open_task() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "To close"]).assert().success();

    cmd(&db_path)
        .args(["close", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Closed: #1 To close"));

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("To close"));
}

#[test]
fn test_close_done_task() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Done then close"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "1"]).assert().success();

    cmd(&db_path)
        .args(["close", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Closed: #1 Done then close"));
}

#[test]
fn test_close_not_found() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Exists"]).assert().success();

    cmd(&db_path)
        .args(["close", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: task #999 not found"));
}

#[test]
fn test_close_already_closed() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Double close"])
        .assert()
        .success();
    cmd(&db_path).args(["close", "1"]).assert().success();

    cmd(&db_path)
        .args(["close", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: task #1 is already closed"));
}
