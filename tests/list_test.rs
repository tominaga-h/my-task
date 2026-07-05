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
fn test_list_sort_multiple_keys() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args([
            "add",
            "A in beta",
            "--project",
            "beta",
            "--due",
            "2026-04-10",
        ])
        .assert()
        .success();
    cmd(&db_path)
        .args([
            "add",
            "B in alpha",
            "--project",
            "alpha",
            "--due",
            "2026-04-20",
        ])
        .assert()
        .success();
    cmd(&db_path)
        .args([
            "add",
            "C in alpha",
            "--project",
            "alpha",
            "--due",
            "2026-04-05",
        ])
        .assert()
        .success();

    // Sort by project first, then by due
    let output = cmd(&db_path)
        .args(["list", "--sort", "project", "--sort", "due"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let pos_c = stdout.find("C in alpha").unwrap();
    let pos_b = stdout.find("B in alpha").unwrap();
    let pos_a = stdout.find("A in beta").unwrap();
    // alpha group first, within alpha: C (04-05) before B (04-20), then beta: A
    assert!(
        pos_c < pos_b && pos_b < pos_a,
        "Should sort by project then due: C < B < A, got c={} b={} a={}",
        pos_c,
        pos_b,
        pos_a
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

#[test]
fn test_list_important_only() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Normal task"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Important task", "--important"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Another normal"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--important-only"])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Important task"));
    assert!(!stdout.contains("Normal task"));
    assert!(!stdout.contains("Another normal"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_list_important_only_no_results() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Normal task"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["list", "--important-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No tasks. Add one with: my-task add \"task title\"",
        ));
}

#[test]
fn test_list_help_shows_follow_and_interval() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--follow"))
        .stdout(predicate::str::contains("--interval"));
}

#[test]
fn test_ls_help_shows_follow_and_interval() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["ls", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--follow"))
        .stdout(predicate::str::contains("--interval"));
}

#[test]
fn test_follow_non_tty_falls_back_to_single_render() {
    // In a pipe (no TTY), `-f` must not hang and must print list-like output.
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Follow fallback task"])
        .assert()
        .success();

    // assert_cmd captures stdout via a pipe => stdout is not a TTY.
    cmd(&db_path)
        .args(["ls", "-f"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Follow fallback task"))
        .stdout(predicate::str::contains("1 tasks"));
}

#[test]
fn test_list_filter_due_exact_date() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Due on target", "--due", "2026-04-15"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Due elsewhere", "--due", "2026-04-16"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--due", "2026-04-15"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Due on target"));
    assert!(!stdout.contains("Due elsewhere"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_list_filter_due_relative_today() {
    // `add --due today` and `list --due today` resolve the same relative value
    // through the same date_parser, so they stay consistent regardless of the
    // actual run date. This makes the end-to-end test deterministic.
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Due today task", "--due", "today"])
        .assert()
        .success();
    // Contrast task with no due date: must be excluded by the today filter.
    cmd(&db_path)
        .args(["add", "No due task"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--due", "today"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Due today task"));
    assert!(!stdout.contains("No due task"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_list_filter_due_short_flag() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Due on target", "--due", "2026-04-15"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Due elsewhere", "--due", "2026-04-16"])
        .assert()
        .success();

    // -d should behave the same as --due.
    let output = cmd(&db_path)
        .args(["list", "-d", "2026-04-15"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Due on target"));
    assert!(!stdout.contains("Due elsewhere"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_list_filter_due_with_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args([
            "add",
            "Alpha due",
            "--project",
            "alpha",
            "--due",
            "2026-04-15",
        ])
        .assert()
        .success();
    cmd(&db_path)
        .args([
            "add",
            "Beta due",
            "--project",
            "beta",
            "--due",
            "2026-04-15",
        ])
        .assert()
        .success();

    // --due AND --project: only the alpha task with the matching due date.
    let output = cmd(&db_path)
        .args(["list", "--due", "2026-04-15", "--project", "alpha"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Alpha due"));
    assert!(!stdout.contains("Beta due"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_list_filter_due_with_all() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Done due task", "--due", "2026-04-15"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "1"]).assert().success();

    // Without --all, the done task is hidden even though due matches.
    let output = cmd(&db_path)
        .args(["list", "--due", "2026-04-15"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("Done due task"));

    // With --all, the done task with a matching due date is shown.
    let output = cmd(&db_path)
        .args(["list", "--due", "2026-04-15", "--all"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Done due task"));
    assert!(stdout.contains("DONE"));
}

#[test]
fn test_list_filter_due_no_match() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Some task", "--due", "2026-04-15"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["list", "--due", "2026-12-31"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No tasks. Add one with: my-task add \"task title\"",
        ));
}

#[test]
fn test_list_filter_due_invalid() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Task"]).assert().success();

    cmd(&db_path)
        .args(["list", "--due", "not-a-date"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid due date"));
}

#[test]
fn test_list_help_shows_due() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--due"));
}

#[test]
fn test_follow_non_tty_empty_db() {
    // Non-TTY fallback with no tasks should print the empty message and exit 0.
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["list", "-f"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No tasks. Add one with: my-task add \"task title\"",
        ));
}

#[test]
fn test_list_filter_category() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Job task", "-p", "job"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Hobby task", "-p", "hobby"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["project", "job", "--set-category", "work"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["project", "hobby", "--set-category", "fun"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["list", "--category", "work"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Job task"));
    assert!(!stdout.contains("Hobby task"));
}

#[test]
fn test_list_filter_category_no_match() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Job task", "-p", "job"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["project", "job", "--set-category", "work"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["list", "--category", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks."));
}

#[test]
fn test_list_category_conflicts_with_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    // --project and --category are mutually exclusive: clap rejects the
    // combination before any query runs.
    cmd(&db_path)
        .args(["list", "--project", "job", "--category", "work"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_list_json_basic() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "First task"]).assert().success();
    cmd(&db_path)
        .args(["add", "Second task"])
        .assert()
        .success();

    let output = cmd(&db_path).args(["list", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout)
        .unwrap_or_else(|e| panic!("output was not valid JSON: {e}\n{stdout}"));
    let arr = value.as_array().expect("top-level JSON array");
    assert_eq!(arr.len(), 2);

    assert_eq!(arr[0]["id"], 1);
    assert_eq!(arr[0]["title"], "First task");
    assert_eq!(arr[0]["status"], "open");
    assert_eq!(arr[1]["id"], 2);
    assert_eq!(arr[1]["title"], "Second task");

    // Table decorations must not appear in JSON mode.
    assert!(!stdout.contains("tasks"));
    assert!(!stdout.contains("OPEN"));
}

#[test]
fn test_list_json_all_includes_done() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Open one"]).assert().success();
    cmd(&db_path).args(["add", "Done one"]).assert().success();
    cmd(&db_path).args(["done", "2"]).assert().success();

    // Without --all, only the open task is returned.
    let output = cmd(&db_path).args(["list", "--json"]).assert().success();
    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "Open one");

    // With --all, the done task appears with status "done".
    let output = cmd(&db_path)
        .args(["list", "--json", "--all"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let done = arr.iter().find(|t| t["id"] == 2).unwrap();
    assert_eq!(done["status"], "done");
    assert!(done["done_at"].is_string());
}

#[test]
fn test_list_json_due_and_remind_values() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args([
            "add",
            "Dated task",
            "--due",
            "2026-07-10",
            "--remind",
            "2026-07-08",
        ])
        .assert()
        .success();

    let output = cmd(&db_path).args(["list", "--json"]).assert().success();
    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let task = &value.as_array().unwrap()[0];

    // due is "YYYY-MM-DD".
    assert_eq!(task["due"], "2026-07-10");
    // reminds is an array of "YYYY-MM-DD" strings.
    let reminds = task["reminds"].as_array().unwrap();
    assert_eq!(reminds.len(), 1);
    assert_eq!(reminds[0], "2026-07-08");
    // created is a datetime "YYYY-MM-DD HH:MM:SS".
    let created = task["created"].as_str().unwrap();
    assert_eq!(created.len(), 19);
    assert!(created.contains(':'));
}

#[test]
fn test_list_json_empty_is_bracket_pair() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    let output = cmd(&db_path).args(["list", "--json"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // No tasks -> JSON empty array, and NOT the "No tasks" message.
    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    assert!(value.as_array().unwrap().is_empty());
    assert_eq!(stdout.trim(), "[]");
    assert!(!stdout.contains("No tasks"));
}

#[test]
fn test_list_json_conflicts_with_follow() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["list", "--json", "--follow"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
