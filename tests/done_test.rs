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

#[test]
fn test_done_multiple_ids() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task A"]).assert().success();
    cmd(&db_path).args(["add", "Task B"]).assert().success();
    cmd(&db_path).args(["add", "Task C"]).assert().success();

    cmd(&db_path)
        .args(["done", "1", "2", "3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Done: #1 Task A"))
        .stdout(predicate::str::contains("Done: #2 Task B"))
        .stdout(predicate::str::contains("Done: #3 Task C"));

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("Task A"));
    assert!(!stdout.contains("Task B"));
    assert!(!stdout.contains("Task C"));
}

#[test]
fn test_done_multiple_ids_partial_failure() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Real task"]).assert().success();
    cmd(&db_path)
        .args(["add", "Another real task"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["done", "1", "999", "2"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Done: #1 Real task"))
        .stdout(predicate::str::contains("Done: #2 Another real task"))
        .stderr(predicate::str::contains("Error: task #999 not found"));

    // Verify successful ones are actually done
    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("Real task"));
    assert!(!stdout.contains("Another real task"));
}

#[test]
fn test_done_multiple_ids_no_args() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["done"]).assert().failure();
}
