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

    cmd(&db_path).args(["add", "Will close"]).assert().success();
    cmd(&db_path).args(["add", "Stay open"]).assert().success();

    // Close task via direct DB update (simulating interactive edit block deletion)
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("UPDATE tasks SET status = 'closed' WHERE id = 1", [])
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

    cmd(&db_path).args(["add", "Will close"]).assert().success();
    cmd(&db_path).args(["add", "Stay open"]).assert().success();

    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute("UPDATE tasks SET status = 'closed' WHERE id = 1", [])
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
fn test_list_sort_desc_by_project() {
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
    cmd(&db_path)
        .args(["add", "Middle task", "--project", "m-proj"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--sort", "project", "--desc"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let pos_a = stdout.find("Alpha task").unwrap();
    let pos_m = stdout.find("Middle task").unwrap();
    let pos_z = stdout.find("Zebra task").unwrap();
    assert!(
        pos_z < pos_m && pos_m < pos_a,
        "z-proj should appear before m-proj before a-proj in descending order"
    );
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
fn test_list_sort_asc() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "First"]).assert().success();
    cmd(&db_path).args(["add", "Second"]).assert().success();
    cmd(&db_path).args(["add", "Third"]).assert().success();

    let output = cmd(&db_path)
        .args(["list", "--sort", "id", "--asc"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let pos_first = stdout.find("First").unwrap();
    let pos_third = stdout.find("Third").unwrap();
    assert!(
        pos_first < pos_third,
        "First should appear before Third in ascending order"
    );
}

#[test]
fn test_list_sort_desc() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "First"]).assert().success();
    cmd(&db_path).args(["add", "Second"]).assert().success();
    cmd(&db_path).args(["add", "Third"]).assert().success();

    let output = cmd(&db_path)
        .args(["list", "--sort", "id", "--desc"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let pos_first = stdout.find("First").unwrap();
    let pos_third = stdout.find("Third").unwrap();
    assert!(
        pos_third < pos_first,
        "Third should appear before First in descending order"
    );
}

#[test]
fn test_list_sort_asc_desc_conflict() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["list", "--asc", "--desc"])
        .assert()
        .failure();
}

#[test]
fn test_list_sort_default_order() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "First"]).assert().success();
    cmd(&db_path).args(["add", "Second"]).assert().success();
    cmd(&db_path).args(["add", "Third"]).assert().success();

    // Default (no --asc/--desc) should be ascending
    let output = cmd(&db_path)
        .args(["list", "--sort", "id"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let pos_first = stdout.find("First").unwrap();
    let pos_third = stdout.find("Third").unwrap();
    assert!(pos_first < pos_third, "Default order should be ascending");
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

#[test]
fn test_list_no_panic_in_pipe() {
    // When running in a pipe (no TTY), terminal_size() returns None.
    // The command should still work without panicking by using a default width.
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args([
            "add",
            "Task with a fairly long title for testing width handling",
        ])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Short", "--project", "proj"])
        .assert()
        .success();

    // assert_cmd captures stdout via pipe, so terminal_size() returns None (default 80)
    cmd(&db_path)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Task with a fairly long title"))
        .stdout(predicate::str::contains("Short"));
}

#[test]
fn test_list_shows_remind_column() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Remind task", "--remind", "2026-04-10"])
        .assert()
        .success();

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("Remind"));
    assert!(stdout.contains("4/10"));
}

#[test]
fn test_list_shows_multiple_reminds() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Multi remind", "--remind", "2026-04-10"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "--remind", "2026-04-15"])
        .assert()
        .success();

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    assert!(stdout.contains("4/10"));
    assert!(stdout.contains("4/15"));
}

#[test]
fn test_list_no_remind_empty() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "No remind"]).assert().success();

    let output = cmd(&db_path).args(["list"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Remind header should be present, but no remind data for this task
    assert!(stdout.contains("Remind"));
    assert!(stdout.contains("No remind"));
}

#[test]
fn test_list_many_tasks_no_panic() {
    // Ensure table rendering doesn't panic even with many rows
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    for i in 1..=20 {
        cmd(&db_path)
            .args(["add", &format!("Task number {}", i)])
            .assert()
            .success();
    }

    cmd(&db_path)
        .args(["list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("20 tasks"));
}
