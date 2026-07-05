use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_search_finds_matching_tasks() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Buy groceries"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Write report"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Buy flowers"])
        .assert()
        .success();

    let output = cmd(&db_path).args(["search", "Buy"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Buy groceries"));
    assert!(stdout.contains("Buy flowers"));
    assert!(!stdout.contains("Write report"));
    assert!(stdout.contains("2 tasks"));
}

#[test]
fn test_search_default_open_only() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Open match task"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Done match task"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "2"]).assert().success();

    let output = cmd(&db_path).args(["search", "match"]).assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Open match task"));
    assert!(!stdout.contains("Done match task"));
    assert!(stdout.contains("1 tasks"));
}

#[test]
fn test_search_all_flag() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Open match task"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Done match task"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "2"]).assert().success();

    let output = cmd(&db_path)
        .args(["search", "match", "--all"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Open match task"));
    assert!(stdout.contains("Done match task"));
    assert!(stdout.contains("2 tasks"));
}

#[test]
fn test_search_with_project_filter() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Task alpha one", "--project", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Task beta one", "--project", "beta"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Task alpha two", "--project", "alpha"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["search", "Task", "--project", "alpha"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Task alpha one"));
    assert!(stdout.contains("Task alpha two"));
    assert!(!stdout.contains("Task beta one"));
    assert!(stdout.contains("2 tasks"));
}

#[test]
fn test_search_no_results() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Some task"]).assert().success();

    cmd(&db_path)
        .args(["search", "nonexistent"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No tasks found"));
}

#[test]
fn test_search_table_format() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Searchable task"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["search", "Searchable"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("ID"));
    assert!(stdout.contains("Title"));
    assert!(stdout.contains("Status"));
}

#[test]
fn test_search_combined_all_and_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Open alpha task", "--project", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Done alpha task", "--project", "alpha"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "2"]).assert().success();
    cmd(&db_path)
        .args(["add", "Open beta task", "--project", "beta"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["search", "task", "--all", "--project", "alpha"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Open alpha task"));
    assert!(stdout.contains("Done alpha task"));
    assert!(!stdout.contains("Open beta task"));
    assert!(stdout.contains("2 tasks"));
}

#[test]
fn test_search_json_matching_only() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Buy groceries"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Write report"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Buy flowers"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["search", "Buy", "--json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    let titles: Vec<&str> = arr.iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Buy groceries"));
    assert!(titles.contains(&"Buy flowers"));
    assert!(!titles.contains(&"Write report"));
}

#[test]
fn test_search_json_empty_is_bracket_pair() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path).args(["add", "Something"]).assert().success();

    let output = cmd(&db_path)
        .args(["search", "nomatch", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    assert!(value.as_array().unwrap().is_empty());
    assert_eq!(stdout.trim(), "[]");
    assert!(!stdout.contains("No tasks found"));
}
