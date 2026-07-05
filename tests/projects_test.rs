use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_projects_empty() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects."));
}

#[test]
fn test_projects_shows_project_names() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Task A", "-p", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Task B", "-p", "beta"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Task C", "-p", "alpha"])
        .assert()
        .success();

    cmd(&db_path).args(["projects"]).assert().success().stdout(
        predicate::str::contains("alpha")
            .and(predicate::str::contains("beta"))
            .and(predicate::str::contains("2 projects")),
    );
}

#[test]
fn test_projects_counts_by_status() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Open task", "-p", "proj"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "Done task", "-p", "proj"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "2"]).assert().success();
    cmd(&db_path)
        .args(["add", "Closed task", "-p", "proj"])
        .assert()
        .success();
    cmd(&db_path).args(["close", "3"]).assert().success();

    cmd(&db_path)
        .args(["projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 projects"));
}

#[test]
fn test_projects_no_project_tasks_not_shown() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "No project task"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No projects."));
}

#[test]
fn test_projects_shows_category_column() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Task A", "-p", "job"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["project", "job", "--set-category", "work"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Category").and(predicate::str::contains("work")));
}

#[test]
fn test_projects_json_multiple_with_category_and_counts() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    // alpha: 2 open, 1 done, 0 closed
    cmd(&db_path)
        .args(["add", "A1", "-p", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "A2", "-p", "alpha"])
        .assert()
        .success();
    cmd(&db_path)
        .args(["add", "A3", "-p", "alpha"])
        .assert()
        .success();
    cmd(&db_path).args(["done", "3"]).assert().success();

    // beta: 1 open only
    cmd(&db_path)
        .args(["add", "B1", "-p", "beta"])
        .assert()
        .success();

    // Set a category on alpha.
    cmd(&db_path)
        .args(["project", "alpha", "--set-category", "work"])
        .assert()
        .success();

    let output = cmd(&db_path)
        .args(["projects", "--json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    let arr = value.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    let alpha = arr.iter().find(|p| p["name"] == "alpha").unwrap();
    assert_eq!(alpha["category"], "work");
    assert_eq!(alpha["open"], 2);
    assert_eq!(alpha["done"], 1);
    assert_eq!(alpha["closed"], 0);
    assert_eq!(alpha["total"], 3);
    // total integrity: total == open + done + closed
    assert_eq!(
        alpha["total"].as_u64().unwrap(),
        alpha["open"].as_u64().unwrap()
            + alpha["done"].as_u64().unwrap()
            + alpha["closed"].as_u64().unwrap()
    );

    let beta = arr.iter().find(|p| p["name"] == "beta").unwrap();
    // No category set -> null.
    assert!(beta["category"].is_null());
    assert_eq!(beta["open"], 1);
    assert_eq!(beta["total"], 1);
}

#[test]
fn test_projects_json_empty_is_bracket_pair() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    let output = cmd(&db_path)
        .args(["projects", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    let value: serde_json::Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    assert!(value.as_array().unwrap().is_empty());
    assert_eq!(stdout.trim(), "[]");
    assert!(!stdout.contains("No projects"));
}
