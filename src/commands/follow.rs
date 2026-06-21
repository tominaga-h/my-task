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

/// Number of fixed header rows at the top of the follow view: the title line
/// plus one blank spacer line.
const HEADER_LINES: usize = 2;

/// Scroll action derived from a key event in the follow view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAction {
    Up,
    Down,
    None,
}

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

/// Map a key event to a scroll action.
///
/// `j` / Down scroll one line down, `k` / Up scroll one line up, and any other
/// key produces no scroll. Quit/refresh keys are handled separately and are not
/// the concern of this function.
pub fn scroll_action(key: KeyEvent) -> ScrollAction {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => ScrollAction::Down,
        KeyCode::Char('k') | KeyCode::Up => ScrollAction::Up,
        _ => ScrollAction::None,
    }
}

/// Clamp a scroll offset so it never goes past the last full screen of content.
///
/// `max_offset` is `total_lines - visible_height` (saturating at 0). When all
/// content fits (`total_lines <= visible_height`) the result is 0. An offset
/// beyond range is pulled back to the maximum rather than reset to the top.
pub fn clamp_scroll_offset(offset: usize, total_lines: usize, visible_height: usize) -> usize {
    let max_offset = total_lines.saturating_sub(visible_height);
    offset.min(max_offset)
}

/// Return the half-open `[start, end)` range of line indices that are visible
/// for a given offset and viewport height.
///
/// The range is always within `0..=total` and never panics, even when `offset`
/// is out of range or `visible` is 0.
pub fn visible_range(total: usize, offset: usize, visible: usize) -> (usize, usize) {
    let start = offset.min(total);
    let end = start.saturating_add(visible).min(total);
    (start, end)
}

/// Format the scroll position indicator shown at the bottom of the follow view.
///
/// Uses a 1-based inclusive range: `[start-end/total]`. When there is no content
/// (`total == 0`) it returns `[0-0/0]`. When everything is visible the range
/// still spans the whole list (e.g. `[1-8/8]`).
pub fn format_scroll_indicator(offset: usize, visible: usize, total: usize) -> String {
    if total == 0 {
        return "[0-0/0]".to_string();
    }
    let (start, end) = visible_range(total, offset, visible);
    if start >= end {
        // Nothing is visible (visible == 0); still report a sane indicator.
        return format!("[0-0/{total}]");
    }
    // Convert the half-open [start, end) to a 1-based inclusive range.
    format!("[{}-{}/{}]", start + 1, end, total)
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

    // Vertical scroll offset, preserved across redraws and key polling.
    let mut scroll_offset: usize = 0;

    'outer: loop {
        let (term_width, term_height) = crossterm::terminal::size().unwrap_or((80, 24));

        let header = render_header(interval);
        let body = render_body(&query, &color_map, term_width);

        let total_lines = body.split('\n').count();
        let visible_height = visible_height_for(term_height);

        // Clamp before drawing so a shrunken list lands on the last full screen
        // instead of snapping back to the top.
        scroll_offset = clamp_scroll_offset(scroll_offset, total_lines, visible_height);

        if draw(&header, &body, scroll_offset, (term_width, term_height)).is_err() {
            break;
        }

        // Wait up to `interval` seconds, polling for key input. On scroll keys
        // (j/k or arrows) adjust the offset and break early to re-render; on
        // `r`/`R` break early to refresh; on q/Q/Esc/^C return to trigger the
        // RAII terminal restore.
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
                        match scroll_action(key) {
                            ScrollAction::Down => {
                                scroll_offset = scroll_offset.saturating_add(1);
                                continue 'outer;
                            }
                            ScrollAction::Up => {
                                scroll_offset = scroll_offset.saturating_sub(1);
                                continue 'outer;
                            }
                            ScrollAction::None => {}
                        }
                    }
                }
                Ok(false) => break,
                Err(_) => break,
            }
        }
    }
}

/// Compute the visible table height: total rows minus the fixed header rows and
/// one row reserved for the scroll indicator. Never goes below 0.
fn visible_height_for(term_height: u16) -> usize {
    (term_height as usize)
        .saturating_sub(HEADER_LINES)
        .saturating_sub(1)
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

/// Build the fixed header line for the follow view (single line of text). The
/// blank spacer line beneath it is appended by the caller / draw routine.
fn render_header(interval: u64) -> String {
    let now = Local::now().format("%H:%M:%S");
    format!(
        "my-task (follow) - q:quit r:refresh j/k:scroll  every {}s  {}",
        interval, now
    )
}

/// Build the scrollable body (task table + footer) as a string.
fn render_body(
    query: &list::ListQuery,
    color_map: &std::collections::HashMap<String, comfy_table::Color>,
    term_width: u16,
) -> String {
    match load_tasks(query) {
        Ok(tasks) => {
            if tasks.is_empty() {
                "No tasks. Add one with: my-task add \"task title\"".to_string()
            } else {
                list::build_task_table_string(&tasks, query.all, color_map, term_width)
            }
        }
        Err(_) => "Error: failed to read database".to_string(),
    }
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

/// Clear the screen and draw the frame with a fixed header, a scrolled body
/// window, and a scroll-position indicator pinned to the bottom row.
///
/// The header occupies the top `HEADER_LINES` rows. The body is split into
/// lines and only the window starting at `scroll_offset` (clamped here as a
/// second line of defence) is drawn beneath the header. Each line is placed
/// with `MoveTo` because raw mode does not translate `\n` into `\r\n`.
fn draw(
    header: &str,
    body: &str,
    scroll_offset: usize,
    term_size: (u16, u16),
) -> std::io::Result<()> {
    let (_term_width, term_height) = term_size;
    let mut stdout = std::io::stdout();
    queue!(stdout, Clear(ClearType::All))?;

    // Fixed header: the title line on row 0, leaving row 1 blank as a spacer.
    queue!(stdout, MoveTo(0, 0))?;
    write!(stdout, "{header}")?;

    let visible_height = visible_height_for(term_height);
    let lines: Vec<&str> = body.split('\n').collect();
    let total_lines = lines.len();

    // Re-clamp defensively in case the caller passed a stale offset.
    let offset = clamp_scroll_offset(scroll_offset, total_lines, visible_height);
    let (start, end) = visible_range(total_lines, offset, visible_height);

    for (row, line) in lines[start..end].iter().enumerate() {
        let y = (HEADER_LINES + row) as u16;
        queue!(stdout, MoveTo(0, y))?;
        write!(stdout, "{line}")?;
    }

    // Scroll indicator pinned to the last terminal row.
    let indicator = format_scroll_indicator(offset, visible_height, total_lines);
    let indicator_row = term_height.saturating_sub(1);
    queue!(stdout, MoveTo(0, indicator_row))?;
    write!(stdout, "{indicator}")?;

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

    // ----- clamp_scroll_offset -----

    #[test]
    fn test_clamp_offset_within_range() {
        // Fits comfortably: offset unchanged.
        assert_eq!(clamp_scroll_offset(0, 100, 20), 0);
        assert_eq!(clamp_scroll_offset(30, 100, 20), 30);
    }

    #[test]
    fn test_clamp_offset_beyond_max_pulls_to_max() {
        // max_offset = 100 - 20 = 80.
        assert_eq!(clamp_scroll_offset(200, 100, 20), 80);
    }

    #[test]
    fn test_clamp_offset_all_visible_is_zero() {
        // total <= visible -> everything fits, offset clamps to 0.
        assert_eq!(clamp_scroll_offset(50, 10, 20), 0);
        assert_eq!(clamp_scroll_offset(5, 20, 20), 0);
    }

    #[test]
    fn test_clamp_offset_empty_no_panic() {
        assert_eq!(clamp_scroll_offset(0, 0, 20), 0);
    }

    #[test]
    fn test_clamp_offset_zero_visible_no_panic() {
        // visible == 0: max_offset == total, so offset clamps to total.
        assert_eq!(clamp_scroll_offset(5, 100, 0), 5);
        assert_eq!(clamp_scroll_offset(200, 100, 0), 100);
    }

    // ----- visible_range -----

    #[test]
    fn test_visible_range_middle_window() {
        // total=10, offset=2, visible=3 -> indices 2,3,4 => [2,5).
        assert_eq!(visible_range(10, 2, 3), (2, 5));
    }

    #[test]
    fn test_visible_range_from_top() {
        assert_eq!(visible_range(10, 0, 3), (0, 3));
    }

    #[test]
    fn test_visible_range_at_max_includes_last() {
        // max offset for visible=3 is 7; window covers the final line (index 9).
        assert_eq!(visible_range(10, 7, 3), (7, 10));
    }

    #[test]
    fn test_visible_range_all_fits() {
        // total <= visible -> whole range.
        assert_eq!(visible_range(5, 0, 20), (0, 5));
    }

    #[test]
    fn test_visible_range_empty_no_panic() {
        assert_eq!(visible_range(0, 0, 20), (0, 0));
    }

    #[test]
    fn test_visible_range_offset_out_of_bounds_no_panic() {
        // offset past the end yields an empty window, not a panic.
        assert_eq!(visible_range(10, 50, 3), (10, 10));
    }

    // ----- scroll_action -----

    #[test]
    fn test_scroll_action_j_is_down() {
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::Down);
    }

    #[test]
    fn test_scroll_action_arrow_down_is_down() {
        let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::Down);
    }

    #[test]
    fn test_scroll_action_k_is_up() {
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::Up);
    }

    #[test]
    fn test_scroll_action_arrow_up_is_up() {
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::Up);
    }

    #[test]
    fn test_scroll_action_q_is_none() {
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::None);
    }

    #[test]
    fn test_scroll_action_r_is_none() {
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::None);
    }

    #[test]
    fn test_scroll_action_other_char_is_none() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(scroll_action(key), ScrollAction::None);
    }

    // ----- format_scroll_indicator -----

    #[test]
    fn test_indicator_middle_window() {
        // offset 11 (0-based), visible 19, total 45 -> 1-based inclusive [12-30/45].
        assert_eq!(format_scroll_indicator(11, 19, 45), "[12-30/45]");
    }

    #[test]
    fn test_indicator_all_visible() {
        // Everything fits: [1-8/8].
        assert_eq!(format_scroll_indicator(0, 20, 8), "[1-8/8]");
    }

    #[test]
    fn test_indicator_empty() {
        assert_eq!(format_scroll_indicator(0, 20, 0), "[0-0/0]");
    }

    #[test]
    fn test_indicator_at_tail() {
        // total=45, visible=19, max offset = 26 -> [27-45/45].
        assert_eq!(format_scroll_indicator(26, 19, 45), "[27-45/45]");
    }

    #[test]
    fn test_indicator_zero_visible_no_panic() {
        // visible == 0: no rows visible, still a sane indicator.
        assert_eq!(format_scroll_indicator(0, 0, 10), "[0-0/10]");
    }
}
