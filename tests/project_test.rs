use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("my-task").unwrap();
    c.env("MY_TASK_DATA_FILE", db_path);
    c
}

#[test]
fn test_project_set_category_success() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    // Create the project by adding a task to it.
    cmd(&db_path)
        .args(["add", "Task A", "-p", "job"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["project", "job", "--set-category", "work"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Set category 'work' for project 'job'",
        ));

    // The category shows up in the projects listing.
    cmd(&db_path)
        .args(["projects"])
        .assert()
        .success()
        .stdout(predicate::str::contains("work"));
}

#[test]
fn test_project_set_category_missing_project() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["project", "ghost", "--set-category", "work"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("project 'ghost' not found"));
}

#[test]
fn test_project_clear_category() {
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
        .args(["project", "job", "--clear-category"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Cleared category for project 'job'",
        ));
}

#[test]
fn test_project_no_option_errors() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("tasks.db");

    cmd(&db_path)
        .args(["add", "Task A", "-p", "job"])
        .assert()
        .success();

    cmd(&db_path)
        .args(["project", "job"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "specify --set-category <name> or --clear-category",
        ));
}
