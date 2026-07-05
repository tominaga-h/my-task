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
        .stdout(predicate::str::contains("Remind: (none)"))
        .stdout(predicate::str::contains("Important: no"));
}

#[test]
fn test_show_multiple_reminds() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Multi remind task"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "-r", "2026-04-10"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["edit", "1", "-r", "2026-04-15"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["show", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Remind: 2026-04-10, 2026-04-15"));
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
fn test_show_json() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args([
            "add",
            "JSON task",
            "-p",
            "myproj",
            "-d",
            "2026-05-01",
            "--important",
        ])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["show", "1", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"id\":1"));
    assert!(stdout.contains("\"title\":\"JSON task\""));
    assert!(stdout.contains("\"status\":\"open\""));
    assert!(stdout.contains("\"project\":\"myproj\""));
    assert!(stdout.contains("\"due\":\"2026-05-01\""));
    assert!(stdout.contains("\"important\":true"));
    assert!(stdout.contains("\"remind\":[]"));
}

#[test]
fn test_show_json_null_fields() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Simple"]).assert().success();

    let output = cmd(&db_path)
        .args(["show", "1", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"project\":null"));
    assert!(stdout.contains("\"due\":null"));
    assert!(stdout.contains("\"important\":false"));
}

#[test]
fn test_show_no_args() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["show"]).assert().failure();
}

/// True when `s` looks like a `HH:MM:SS` time token (e.g. "09:30:05").
fn looks_like_hms(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 8
        && bytes[2] == b':'
        && bytes[5] == b':'
        && bytes.iter().enumerate().all(|(i, &b)| {
            if i == 2 || i == 5 {
                b == b':'
            } else {
                b.is_ascii_digit()
            }
        })
}

#[test]
fn test_show_text_created_updated_have_time_component() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Timed task"]).assert().success();

    let output = cmd(&db_path).args(["show", "1"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Created / Updated are now "YYYY-MM-DD HH:MM:SS": a date, a space, then a time.
    for prefix in ["Created:", "Updated:"] {
        let line = stdout
            .lines()
            .find(|l| l.starts_with(prefix))
            .unwrap_or_else(|| panic!("{prefix} line present"));
        let value = line.trim_start_matches(prefix).trim();
        let (date_part, time_part) = value
            .split_once(' ')
            .unwrap_or_else(|| panic!("{prefix} value lacks a time component: {value}"));
        assert_eq!(date_part.len(), 10, "{prefix} date part: {date_part}");
        assert!(
            looks_like_hms(time_part),
            "{prefix} time part is not HH:MM:SS: {time_part}"
        );
    }
}

#[test]
fn test_show_json_created_updated_have_time_due_does_not() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "JSON timed", "-d", "2026-05-01"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["show", "1", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // created / updated carry a HH:MM:SS time component inside the JSON string.
    for key in ["created", "updated"] {
        let needle = format!("\"{key}\":\"");
        let start = stdout
            .find(&needle)
            .unwrap_or_else(|| panic!("{key} present: {stdout}"))
            + needle.len();
        let rest = &stdout[start..];
        let value = &rest[..rest.find('"').expect("closing quote")];
        let (date_part, time_part) = value
            .split_once(' ')
            .unwrap_or_else(|| panic!("{key} JSON value lacks a time component: {value}"));
        assert_eq!(date_part.len(), 10, "{key} date part: {date_part}");
        assert!(
            looks_like_hms(time_part),
            "{key} time part is not HH:MM:SS: {time_part}"
        );
    }

    // due stays date-only: no time component appended.
    assert!(stdout.contains("\"due\":\"2026-05-01\""));
    assert!(!stdout.contains("\"due\":\"2026-05-01 "));
}
