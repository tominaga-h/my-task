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
    assert!(stdout.contains("OPEN"));
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
    assert!(stdout.contains("OPEN"));
    assert!(stdout.contains("DONE"));
    assert!(stdout.contains("Done task"));
    assert!(
        !stdout.contains("\u{2713}"),
        "checkmark should no longer appear"
    );
    assert!(stdout.contains("2 tasks (1 done)"));
}

#[test]
fn test_list_closed_hidden_by_default() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Will close"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Stay open"])
        .assert()
        .success();

    // Close task via direct DB update (simulating interactive edit block deletion)
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "UPDATE tasks SET status = 'closed' WHERE id = 1",
        [],
    )
    .unwrap();

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("Will close"));
    assert!(stdout.contains("Stay open"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_list_closed_shown_with_all() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Will close"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Stay open"])
        .assert()
        .success();

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "UPDATE tasks SET status = 'closed' WHERE id = 1",
        [],
    )
    .unwrap();

    let output = cmd(&db_path).args(["list", "--all"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Will close"));
    assert!(stdout.contains("CLOSED"));
    assert!(stdout.contains("Stay open"));
    assert!(stdout.contains("2 tasks (1 done)"));
}

#[test]
fn test_list_sort_by_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Zebra task", "--project", "z-proj"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Alpha task", "--project", "a-proj"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--sort", "project"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let pos_a = stdout.find("Alpha task").unwrap();
    let pos_z = stdout.find("Zebra task").unwrap();
    assert!(pos_a < pos_z, "a-proj should appear before z-proj");
}

#[test]
fn test_list_sort_invalid() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["list", "--sort", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown sort key"));
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
