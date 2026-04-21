//! Mirdan Search - Search the registry for skills and validators.
//!
//! Supports two modes:
//! - **Direct**: `mirdan search <query>` — single API call, table output
//! - **Interactive**: `mirdan search` — TUI with live fuzzy search, arrow navigation

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};

use crate::registry::types::FuzzySearchResult;
use crate::registry::{RegistryClient, RegistryError};
use crate::table;

const MIN_QUERY_LEN: usize = 2;
/// Lines per result card: name line + description line + meta line + blank separator.
const LINES_PER_RESULT: usize = 4;
/// Fixed overhead lines: header(3) + prompt(2) + gap before footer(1) + footer(2).
const OVERHEAD_LINES: usize = 8;

/// Poll interval used when no pending query is waiting; only drives the
/// terminal redraw loop.
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Debounce window for short (≤ `SHORT_QUERY_LEN`) queries. Longer keystroke
/// gaps reduce noisy queries when the user is still typing.
const SHORT_QUERY_DEBOUNCE: Duration = Duration::from_millis(250);
/// Debounce window for longer queries where typing is more deliberate.
const LONG_QUERY_DEBOUNCE: Duration = Duration::from_millis(150);
/// Query-length threshold for switching between short and long debounce.
const SHORT_QUERY_LEN: usize = 3;
/// Minimum number of results to request from the registry regardless of
/// terminal height — ensures the user always sees some options.
const MIN_VISIBLE_RESULTS: usize = 2;
/// Maximum number of results to request to avoid over-fetching when the
/// terminal is very tall.
const MAX_VISIBLE_RESULTS: usize = 20;

/// Horizontal padding reserved around rendered content: 4-char left indent
/// plus a 2-char right gutter so text never touches the terminal edge.
const CONTENT_HORIZONTAL_PADDING: usize = 6;

/// Run the search command with a query string (non-interactive).
///
/// Queries the registry for packages matching `query` and prints results to
/// stdout. When `json` is true, emits pretty-printed JSON instead of the
/// human-readable table.
///
/// # Errors
///
/// Returns an error if the registry request fails or JSON serialization fails.
pub async fn run_search(query: &str, json: bool) -> Result<(), RegistryError> {
    let client = RegistryClient::new();
    let response = client.search(query, None, None).await?;

    if json {
        let output = serde_json::to_string_pretty(&response)?;
        println!("{}", output);
        return Ok(());
    }

    if response.packages.is_empty() {
        println!("No packages found matching \"{}\".", query);
        return Ok(());
    }

    println!(
        "Found {} package(s) matching \"{}\":\n",
        response.total, query
    );

    let mut tbl = table::new_table();
    tbl.set_header(vec!["Name", "Type", "Version", "Description", "Downloads"]);

    for pkg in &response.packages {
        let name = table::short_name(&pkg.name);
        let description = table::truncate_str(&pkg.description, 50);
        let pkg_type = pkg.package_type.as_deref().unwrap_or("unknown");

        tbl.add_row(vec![
            name,
            pkg_type.to_string(),
            pkg.latest.clone(),
            description,
            format_downloads(pkg.downloads),
        ]);
    }

    println!("{tbl}");
    println!("\nRun 'mirdan info <name>' for more details.");

    Ok(())
}

/// Run interactive fuzzy search mode.
///
/// Enters a TUI for live fuzzy search. On selection, installs the package.
///
/// # Errors
///
/// Returns an error if stdin is not a terminal, if the TUI fails to
/// initialize, if a registry request fails, or if the subsequent install
/// fails.
pub async fn run_interactive_search() -> Result<(), RegistryError> {
    if !io::stdin().is_terminal() {
        return Err(RegistryError::Validation(
            "Interactive search requires a terminal. Use 'mirdan search <query>' instead.".into(),
        ));
    }

    let selected = tokio::task::spawn_blocking(interactive_search_loop)
        .await
        .map_err(|e| RegistryError::Io(io::Error::other(e)))??;

    if let Some(name) = selected {
        let results = crate::install::run_install(&name, None, false, false, None).await?;
        crate::format_deploy_results(&results);
        println!("\nTo remove: mirdan uninstall {}", name);
    }

    Ok(())
}

/// Ensures raw mode and alternate screen are cleaned up on drop.
struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// What to do with the event loop after handling a single key.
enum KeyOutcome {
    /// Keep looping (state was updated in place).
    Continue,
    /// Exit the loop with this result (None = cancel, Some(name) = install).
    Exit(Option<String>),
}

/// Mutable state threaded through the interactive search event loop.
struct SearchState {
    query: String,
    results: Vec<FuzzySearchResult>,
    total: usize,
    selected: usize,
    last_keypress: Instant,
    last_sent_query: String,
    loading: bool,
    error_msg: Option<String>,
}

impl SearchState {
    fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            total: 0,
            selected: 0,
            last_keypress: Instant::now(),
            last_sent_query: String::new(),
            loading: false,
            error_msg: None,
        }
    }

    fn render(&self, stdout: &mut io::Stdout) -> Result<(), RegistryError> {
        render(
            stdout,
            &self.query,
            &self.results,
            self.total,
            self.selected,
            self.loading,
            &self.error_msg,
        )
    }
}

fn interactive_search_loop() -> Result<Option<String>, RegistryError> {
    let handle = tokio::runtime::Handle::current();
    let client = RegistryClient::new();
    let mut stdout = io::stdout();

    // Enter raw mode with cleanup guard
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let _guard = RawModeGuard;

    let mut state = SearchState::new();

    let action = loop {
        state.render(&mut stdout)?;

        let needs_query =
            state.query.len() >= MIN_QUERY_LEN && state.query != state.last_sent_query;
        let poll_timeout = compute_poll_timeout(&state, needs_query);

        if event::poll(poll_timeout)? {
            match read_and_handle_key(&mut state)? {
                KeyOutcome::Continue => continue,
                KeyOutcome::Exit(result) => break result,
            }
        } else if needs_query {
            fetch_and_apply_results(&mut state, &mut stdout, &handle, &client)?;
        }
    };

    drop(_guard);

    Ok(action)
}

/// Compute how long to block in `event::poll` before either firing a query
/// (debounce expired) or simply re-rendering.
fn compute_poll_timeout(state: &SearchState, needs_query: bool) -> Duration {
    if !needs_query {
        return IDLE_POLL_INTERVAL;
    }
    // Adaptive debounce: longer for short queries (more ambiguous)
    let debounce = if state.query.len() <= SHORT_QUERY_LEN {
        SHORT_QUERY_DEBOUNCE
    } else {
        LONG_QUERY_DEBOUNCE
    };
    let elapsed = state.last_keypress.elapsed();
    debounce.saturating_sub(elapsed)
}

/// Read the next terminal event (we know `poll` just returned true) and apply
/// it to `state`. Only keyboard press events cause state changes.
fn read_and_handle_key(state: &mut SearchState) -> Result<KeyOutcome, RegistryError> {
    let Event::Key(key) = event::read()? else {
        return Ok(KeyOutcome::Continue);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(KeyOutcome::Continue);
    }

    match key.code {
        KeyCode::Esc => Ok(KeyOutcome::Exit(None)),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Ok(KeyOutcome::Exit(None))
        }
        KeyCode::Enter => {
            let selected_name = state.results.get(state.selected).map(|r| r.name.clone());
            match selected_name {
                Some(name) => Ok(KeyOutcome::Exit(Some(name))),
                None => Ok(KeyOutcome::Continue),
            }
        }
        KeyCode::Backspace => {
            handle_backspace(state);
            Ok(KeyOutcome::Continue)
        }
        KeyCode::Char(c) => {
            state.query.push(c);
            state.last_keypress = Instant::now();
            state.error_msg = None;
            Ok(KeyOutcome::Continue)
        }
        KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            Ok(KeyOutcome::Continue)
        }
        KeyCode::Down if !state.results.is_empty() && state.selected < state.results.len() - 1 => {
            state.selected += 1;
            Ok(KeyOutcome::Continue)
        }
        _ => Ok(KeyOutcome::Continue),
    }
}

/// Pop a character from the query and reset any pending result view if we're
/// now below the minimum query length.
fn handle_backspace(state: &mut SearchState) {
    state.query.pop();
    state.last_keypress = Instant::now();
    if state.query.len() < MIN_QUERY_LEN {
        state.results.clear();
        state.total = 0;
        state.selected = 0;
        state.last_sent_query.clear();
    }
    state.error_msg = None;
}

/// Fire the fuzzy-search API call and write the response into `state`. If the
/// user typed more characters while the request was in flight we discard the
/// response so the newer query wins.
fn fetch_and_apply_results(
    state: &mut SearchState,
    stdout: &mut io::Stdout,
    handle: &tokio::runtime::Handle,
    client: &RegistryClient,
) -> Result<(), RegistryError> {
    state.loading = true;
    state.render(stdout)?;

    let snapshot = state.query.clone();
    let (_cols, rows) = terminal::size().unwrap_or((80, 24));
    let max_results = (rows as usize).saturating_sub(OVERHEAD_LINES) / LINES_PER_RESULT;
    let max_results = max_results.clamp(MIN_VISIBLE_RESULTS, MAX_VISIBLE_RESULTS);

    let response = handle.block_on(client.fuzzy_search(&snapshot, Some(max_results)));

    // Only apply the response if the query hasn't changed while we waited.
    if state.query == snapshot {
        match response {
            Ok(response) => {
                state.results = response.results;
                state.total = response.total;
                state.selected = state.selected.min(state.results.len().saturating_sub(1));
                state.last_sent_query = snapshot;
                state.error_msg = None;
            }
            Err(e) => {
                state.error_msg = Some(e.to_string());
                state.last_sent_query = snapshot;
            }
        }
    }

    state.loading = false;
    Ok(())
}

fn render(
    stdout: &mut io::Stdout,
    query: &str,
    results: &[FuzzySearchResult],
    total: usize,
    selected: usize,
    loading: bool,
    error_msg: &Option<String>,
) -> Result<(), RegistryError> {
    let (cols, _rows) = terminal::size().unwrap_or((80, 24));
    let w = cols as usize;
    let content_w = w.saturating_sub(CONTENT_HORIZONTAL_PADDING);

    execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;
    render_header(stdout)?;
    render_prompt(stdout, query, loading)?;
    render_body(
        stdout, query, results, selected, loading, content_w, error_msg,
    )?;
    render_footer(stdout, results, total, w)?;
    stdout.flush()?;
    Ok(())
}

fn render_header(stdout: &mut io::Stdout) -> Result<(), RegistryError> {
    raw_line(stdout, "")?;
    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold)
    )?;
    raw_line(stdout, "  mirdan search")?;
    execute!(stdout, SetAttribute(Attribute::Reset))?;
    Ok(())
}

fn render_prompt(stdout: &mut io::Stdout, query: &str, loading: bool) -> Result<(), RegistryError> {
    raw_line(stdout, "")?;
    let loading_ind = if loading { " ..." } else { "" };
    let prompt = format!("  > {}{}", query, loading_ind);
    execute!(
        stdout,
        SetForegroundColor(Color::White),
        SetAttribute(Attribute::Bold)
    )?;
    raw_line(stdout, &prompt)?;
    execute!(stdout, SetAttribute(Attribute::Reset))?;
    raw_line(stdout, "")?;
    Ok(())
}

fn render_body(
    stdout: &mut io::Stdout,
    query: &str,
    results: &[FuzzySearchResult],
    selected: usize,
    loading: bool,
    content_w: usize,
    error_msg: &Option<String>,
) -> Result<(), RegistryError> {
    if let Some(err) = error_msg {
        return render_centered_line(
            stdout,
            Color::Red,
            &format!("  {}", tui_truncate(err, content_w)),
        );
    }
    if query.len() < MIN_QUERY_LEN {
        return render_centered_line(
            stdout,
            Color::DarkGrey,
            "  Type at least 2 characters to search...",
        );
    }
    if results.is_empty() && !loading {
        return render_centered_line(stdout, Color::DarkGrey, "  No results found.");
    }
    for (i, result) in results.iter().enumerate() {
        render_result_card(stdout, result, i == selected, content_w)?;
    }
    Ok(())
}

fn render_centered_line(
    stdout: &mut io::Stdout,
    color: Color,
    text: &str,
) -> Result<(), RegistryError> {
    execute!(stdout, SetForegroundColor(color))?;
    raw_line(stdout, text)?;
    execute!(stdout, ResetColor)?;
    Ok(())
}

fn render_result_card(
    stdout: &mut io::Stdout,
    result: &FuzzySearchResult,
    is_sel: bool,
    content_w: usize,
) -> Result<(), RegistryError> {
    let marker = if is_sel { "▸" } else { " " };
    let dl = format_downloads(result.downloads);
    let pkg_type = result.package_type.as_deref().unwrap_or("package");

    // --- Line 1: marker + name (bold) ... downloads right-aligned ---
    let name_color = if is_sel { Color::Cyan } else { Color::White };
    execute!(
        stdout,
        SetForegroundColor(name_color),
        SetAttribute(Attribute::Bold)
    )?;
    let dl_display = format!("{} dl", dl);
    let name_budget = content_w.saturating_sub(dl_display.len() + 2);
    let name = tui_truncate(&result.name, name_budget);
    let pad = content_w.saturating_sub(name.len() + dl_display.len());
    raw_line(
        stdout,
        &format!("  {} {}{:>pad$}", marker, name, dl_display, pad = pad),
    )?;
    execute!(stdout, SetAttribute(Attribute::Reset))?;

    // --- Line 2: description ---
    if is_sel {
        execute!(stdout, SetForegroundColor(Color::Cyan))?;
    }
    raw_line(
        stdout,
        &format!("    {}", tui_truncate(&result.description, content_w)),
    )?;

    // --- Line 3: author · type ---
    execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
    let meta = format!("by {} · {}", result.author, pkg_type);
    raw_line(stdout, &format!("    {}", tui_truncate(&meta, content_w)))?;
    execute!(stdout, SetAttribute(Attribute::Reset))?;

    raw_line(stdout, "")?; // blank separator between cards
    Ok(())
}

fn render_footer(
    stdout: &mut io::Stdout,
    results: &[FuzzySearchResult],
    total: usize,
    w: usize,
) -> Result<(), RegistryError> {
    execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
    if results.is_empty() {
        raw_line(stdout, "  esc quit")?;
    } else {
        let line = format!(
            "  {} of {} results  |  up/down navigate  enter install  esc quit",
            results.len(),
            total
        );
        raw_line(stdout, &tui_truncate(&line, w))?;
    }
    execute!(stdout, ResetColor)?;
    Ok(())
}

/// Write a line in raw mode (needs explicit \r\n).
fn raw_line(stdout: &mut io::Stdout, text: &str) -> io::Result<()> {
    write!(stdout, "{}\r\n", text)
}

/// Truncate a string to fit within `max` display columns.
fn tui_truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max > 3 {
        let end = s
            .char_indices()
            .map(|(i, _)| i)
            .take(max - 2)
            .last()
            .unwrap_or(0);
        format!("{}..", &s[..end])
    } else {
        s.chars().take(max).collect()
    }
}

/// Format download count for display (e.g. 1234 -> "1.2k").
fn format_downloads(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_downloads() {
        assert_eq!(format_downloads(0), "0");
        assert_eq!(format_downloads(500), "500");
        assert_eq!(format_downloads(1234), "1.2k");
        assert_eq!(format_downloads(12345), "12.3k");
        assert_eq!(format_downloads(1_234_567), "1.2M");
    }

    #[test]
    fn test_tui_truncate_short() {
        assert_eq!(tui_truncate("hello", 10), "hello");
    }

    #[test]
    fn test_tui_truncate_exact() {
        assert_eq!(tui_truncate("hello", 5), "hello");
    }

    #[test]
    fn test_tui_truncate_long() {
        assert_eq!(tui_truncate("hello world", 8), "hello..");
    }

    #[test]
    fn test_tui_truncate_tiny() {
        assert_eq!(tui_truncate("hello", 3), "hel");
    }
}
