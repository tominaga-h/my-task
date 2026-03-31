use std::path::PathBuf;

pub fn db_path() -> PathBuf {
    if let Ok(p) = std::env::var("MY_TASK_DATA_FILE") {
        return PathBuf::from(p);
    }
    let base = dirs::data_dir().expect("Could not determine data directory");
    base.join("my-task").join("tasks.db")
}
