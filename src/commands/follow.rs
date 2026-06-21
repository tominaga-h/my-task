use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use chrono::Local;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, queue};

use crate::commands::list::{self, ListArgs};
use crate::config;
use crate::db;

/// Clamp the polling interval so it is never below 1 second.
pub fn normalize_interval(secs: u64) -> u64 {
    secs.max(1)
}

/// Return true for keys that should quit the follow view: q / Q / Esc / Ctrl-C.
pub fn is_quit_key(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => true,
        KeyCode::Esc => true,
        KeyCode::Char('c') | KeyCode::Char('C')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            true
        }
        _ => false,
    }
}

/// RAII guard that restores the terminal on drop: leaves raw mode, leaves the
/// alternate screen, and shows the cursor again. Errors during cleanup are
/// ignored so that the terminal is restored even on panic / early return.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        if let Err(e) = execute!(stdout, EnterAlternateScreen, Hide) {
            // Undo raw mode so the terminal is not left in a half-entered state
            // when entering the alternate screen fails.
            let _ = disable_raw_mode();
            return Err(e);
        }
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut stdout = std::io::stdout();
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Entry point for `list --follow`.
///
/// In a non-TTY context (pipe, redirect, test harness) it does not enter the
/// alternate screen; it prints the list output once and returns so callers do
/// not hang.
pub fn run_follow(args: &ListArgs) {
    let interval = normalize_interval(args.interval);

    // Non-TTY fallback: behave like a single `list` invocation.
    if !std::io::stdout().is_terminal() {
        run_once_plain(args);
        return;
    }

    let _guard = match TerminalGuard::enter() {
        Ok(g) => g,
        Err(_) => {
            // If we cannot set up the terminal, fall back to a single render.
            run_once_plain(args);
            return;
        }
    };

    let query = list::resolve_query(args);

    // Generate the project color map once so colors stay stable across redraws.
    let color_map = build_color_map_once(&query);

    loop {
        let frame = render_frame(&query, &color_map, interval);
        if draw(&frame).is_err() {
            break;
        }

        // Wait up to `interval` seconds, polling for key input. On `r`/`R`
        // break early so the outer loop re-renders immediately; on q/Q/Esc/^C
        // return to trigger the RAII terminal restore.
        let deadline = Instant::now() + Duration::from_secs(interval);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match event::poll(remaining) {
                Ok(true) => {
                    if let Ok(Event::Key(key)) = event::read() {
                        if is_quit_key(key) {
                            return;
                        }
                        if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
                            break;
                        }
                    }
                }
                Ok(false) => break,
                Err(_) => break,
            }
        }
    }
}

/// Render the list output once to stdout via the normal (non-TUI) path.
fn run_once_plain(args: &ListArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let query = list::resolve_query(args);
    let tasks = match db::list_tasks(
        &conn,
        query.all,
        query.project.as_deref(),
        &query.sorts,
        query.order,
        query.important_only,
    ) {
        Ok(t) => t,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if tasks.is_empty() {
        println!("No tasks. Add one with: my-task add \"task title\"");
        return;
    }

    list::print_task_table(&tasks, query.all, &conn);
}

/// Build the project color map once, reading the current task set.
fn build_color_map_once(
    query: &list::ListQuery,
) -> std::collections::HashMap<String, comfy_table::Color> {
    let db_path = config::db_path();
    let tasks = match db::open(&db_path) {
        Ok(conn) => db::list_tasks(
            &conn,
            query.all,
            query.project.as_deref(),
            &query.sorts,
            query.order,
            query.important_only,
        )
        .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    list::build_project_color_map(&tasks)
}

/// Build the full frame body (header line + table) as a string.
fn render_frame(
    query: &list::ListQuery,
    color_map: &std::collections::HashMap<String, comfy_table::Color>,
    interval: u64,
) -> String {
    let now = Local::now().format("%H:%M:%S");
    let header = format!(
        "my-task (follow) - q:quit r:refresh  every {}s  {}",
        interval, now
    );

    let term_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);

    let body = match load_tasks(query) {
        Ok(tasks) => {
            if tasks.is_empty() {
                "No tasks. Add one with: my-task add \"task title\"".to_string()
            } else {
                list::build_task_table_string(&tasks, query.all, color_map, term_width)
            }
        }
        Err(_) => "Error: failed to read database".to_string(),
    };

    format!("{header}\n\n{body}")
}

/// Open the DB and load tasks (with reminds filled), matching the list command.
fn load_tasks(query: &list::ListQuery) -> Result<Vec<crate::model::Task>, rusqlite::Error> {
    let db_path = config::db_path();
    let conn = db::open(&db_path)?;
    let mut tasks = db::list_tasks(
        &conn,
        query.all,
        query.project.as_deref(),
        &query.sorts,
        query.order,
        query.important_only,
    )?;
    for task in &mut tasks {
        task.reminds = db::get_reminds_for_task(&conn, task.id).unwrap_or_default();
    }
    Ok(tasks)
}

/// Clear the screen and draw the frame. In raw mode each newline must be
/// emitted as `\r\n`, so split the frame into lines and move the cursor to the
/// start of each successive row.
fn draw(frame: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    for (i, line) in frame.split('\n').enumerate() {
        queue!(stdout, MoveTo(0, i as u16))?;
        write!(stdout, "{line}")?;
    }
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};

    #[test]
    fn test_normalize_interval_zero_becomes_one() {
        assert_eq!(normalize_interval(0), 1);
    }

    #[test]
    fn test_normalize_interval_preserves_positive() {
        assert_eq!(normalize_interval(2), 2);
        assert_eq!(normalize_interval(5), 5);
    }

    #[test]
    fn test_is_quit_key_q_lower() {
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(is_quit_key(key));
    }

    #[test]
    fn test_is_quit_key_q_upper() {
        let key = KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::NONE);
        assert!(is_quit_key(key));
    }

    #[test]
    fn test_is_quit_key_esc() {
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(is_quit_key(key));
    }

    #[test]
    fn test_is_quit_key_ctrl_c() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(is_quit_key(key));
    }

    #[test]
    fn test_is_quit_key_plain_char_is_false() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(!is_quit_key(key));
    }

    #[test]
    fn test_is_quit_key_plain_c_is_false() {
        // 'c' without the Ctrl modifier must not quit.
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert!(!is_quit_key(key));
    }
}
