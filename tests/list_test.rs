use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_list_empty() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No tasks. Add one with: my-task add \"task title\"",
        ));
}

#[test]
fn test_list_shows_open() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Open one"]).assert().success();
    cmd(&db_path).args(["add", "Open two"]).assert().success();
    cmd(&db_path)
        .args(["add", "To complete"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "3"]).assert().success();

    let output = cmd(&db_path).args(["list"]).assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Open one"));
    assert!(stdout.contains("Open two"));
    assert!(!stdout.contains("To complete"));
    assert!(stdout.contains("2 tasks"));
}

#[test]
fn test_list_filter_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "A task 1", "--project", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "A task 2", "--project", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "B task", "--project", "beta"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--project", "alpha"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("A task 1"));
    assert!(stdout.contains("A task 2"));
    assert!(!stdout.contains("B task"));
    assert!(stdout.contains("2 tasks"));
}

#[test]
fn test_list_all_flag() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Open task"]).assert().success();
    cmd(&db_path).args(["add", "Done task"]).assert().success();
    cmd(&db_path).args(["done", "2"]).assert().success();

    let output = cmd(&db_path).args(["list", "--all"]).assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Open task"));
    assert!(stdout.contains("\u{2713}"));
    assert!(stdout.contains("Done task"));
    assert!(stdout.contains("2 tasks (1 done)"));
}

#[test]
fn test_ls_alias() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Alias test"]).assert().success();

    let output = cmd(&db_path).args(["ls"]).assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Alias test"));
    assert!(stdout.contains("1 tasks"));
}
