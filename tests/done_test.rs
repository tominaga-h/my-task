use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_done_basic() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Finish me"]).assert().success();

    cmd(&db_path)
        .args(["done", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Done: #1 Finish me"));

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("Finish me"));
}

#[test]
fn test_done_not_found() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Exists"]).assert().success();

    cmd(&db_path)
        .args(["done", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: task #999 not found"));
}

#[test]
fn test_done_already_done() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Double done"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "1"]).assert().success();

    cmd(&db_path)
        .args(["done", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error: task #1 is already done"));
}
