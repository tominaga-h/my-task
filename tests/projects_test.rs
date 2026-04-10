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

    cmd(&db_path)
        .args(["projects"])
        .assert()
        .success()
        .stdout(
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
