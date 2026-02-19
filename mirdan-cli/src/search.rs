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

/// Run the search command with a query string (non-interactive).
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
        crate::install::run_install(&name, None, false, false, None).await?;
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

fn interactive_search_loop() -> Result<Option<String>, RegistryError> {
    let handle = tokio::runtime::Handle::current();
    let client = RegistryClient::new();
    let mut stdout = io::stdout();

    // Enter raw mode with cleanup guard
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let _guard = RawModeGuard;

    let mut query = String::new();
    let mut results: Vec<FuzzySearchResult> = Vec::new();
    let mut total: usize = 0;
    let mut selected: usize = 0;
    let mut last_keypress = Instant::now();
    let mut last_sent_query = String::new();
    let mut loading = false;
    let mut error_msg: Option<String> = None;

    let action = loop {
        render(
            &mut stdout,
            &query,
            &results,
            total,
            selected,
            loading,
            &error_msg,
        )?;

        // Adaptive debounce: longer for short queries (more ambiguous)
        let debounce = if query.len() <= 3 {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(150)
        };

        let needs_query = query.len() >= MIN_QUERY_LEN && query != last_sent_query;
        let poll_timeout = if needs_query {
            let elapsed = last_keypress.elapsed();
            if elapsed >= debounce {
                Duration::ZERO
            } else {
                debounce - elapsed
            }
        } else {
            Duration::from_millis(100)
        };

        if event::poll(poll_timeout)? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (ignore release/repeat)
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => break None,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break None;
                    }
                    KeyCode::Enter => {
                        if let Some(result) = results.get(selected) {
                            break Some(result.name.clone());
                        }
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        last_keypress = Instant::now();
                        if query.len() < MIN_QUERY_LEN {
                            results.clear();
                            total = 0;
                            selected = 0;
                            last_sent_query.clear();
                        }
                        error_msg = None;
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                        last_keypress = Instant::now();
                        error_msg = None;
                    }
                    KeyCode::Up => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        if !results.is_empty() && selected < results.len() - 1 {
                            selected += 1;
                        }
                    }
                    _ => {}
                }
            }
        } else if needs_query {
            // Debounce expired — fire API call
            loading = true;
            render(
                &mut stdout,
                &query,
                &results,
                total,
                selected,
                loading,
                &error_msg,
            )?;

            let snapshot = query.clone();
            let (_cols, rows) = terminal::size().unwrap_or((80, 24));
            let max_results = (rows as usize).saturating_sub(OVERHEAD_LINES) / LINES_PER_RESULT;
            let max_results = max_results.clamp(2, 20);
            match handle.block_on(client.fuzzy_search(&snapshot, Some(max_results))) {
                Ok(response) => {
                    if query == snapshot {
                        results = response.results;
                        total = response.total;
                        selected = selected.min(results.len().saturating_sub(1));
                        last_sent_query = snapshot;
                        error_msg = None;
                    }
                }
                Err(e) => {
                    if query == snapshot {
                        error_msg = Some(e.to_string());
                        last_sent_query = snapshot;
                    }
                }
            }
            loading = false;
        }
    };

    // Guard handles cleanup via Drop
    drop(_guard);

    Ok(action)
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
    // Usable content width inside the 4-char left margin
    let content_w = w.saturating_sub(6);

    execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

    // Header
    raw_line(stdout, "")?;
    execute!(
        stdout,
        SetForegroundColor(Color::Cyan),
        SetAttribute(Attribute::Bold)
    )?;
    raw_line(stdout, "  mirdan search")?;
    execute!(stdout, SetAttribute(Attribute::Reset))?;

    // Query prompt
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

    // Body
    if let Some(err) = error_msg {
        execute!(stdout, SetForegroundColor(Color::Red))?;
        raw_line(stdout, &format!("  {}", tui_truncate(err, content_w)))?;
        execute!(stdout, ResetColor)?;
    } else if query.len() < MIN_QUERY_LEN {
        execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
        raw_line(stdout, "  Type at least 2 characters to search...")?;
        execute!(stdout, ResetColor)?;
    } else if results.is_empty() && !loading {
        execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
        raw_line(stdout, "  No results found.")?;
        execute!(stdout, ResetColor)?;
    } else {
        for (i, result) in results.iter().enumerate() {
            let is_sel = i == selected;
            let marker = if is_sel { "▸" } else { " " };
            let dl = format_downloads(result.downloads);
            let pkg_type = result.package_type.as_deref().unwrap_or("package");

            // --- Line 1: marker + name (bold) ... downloads right-aligned ---
            if is_sel {
                execute!(
                    stdout,
                    SetForegroundColor(Color::Cyan),
                    SetAttribute(Attribute::Bold)
                )?;
            } else {
                execute!(
                    stdout,
                    SetForegroundColor(Color::White),
                    SetAttribute(Attribute::Bold)
                )?;
            }
            // "  ▸ name" on the left, "1.2k ↓" on the right
            let dl_display = format!("{} dl", dl);
            let name_budget = content_w.saturating_sub(dl_display.len() + 2);
            let name = tui_truncate(&result.name, name_budget);
            let pad = content_w.saturating_sub(name.len() + dl_display.len());
            raw_line(
                stdout,
                &format!("  {} {}{:>pad$}", marker, name, dl_display, pad = pad),
            )?;
            execute!(stdout, SetAttribute(Attribute::Reset))?;

            // --- Line 2: description (full width, wraps naturally via truncation) ---
            if is_sel {
                execute!(stdout, SetForegroundColor(Color::Cyan))?;
            }
            let desc = tui_truncate(&result.description, content_w);
            raw_line(stdout, &format!("    {}", desc))?;

            // --- Line 3: author · type ---
            execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
            let meta = format!("by {} · {}", result.author, pkg_type);
            raw_line(stdout, &format!("    {}", tui_truncate(&meta, content_w)))?;
            execute!(stdout, SetAttribute(Attribute::Reset))?;

            // --- Blank separator between cards ---
            raw_line(stdout, "")?;
        }
    }

    // Footer
    execute!(stdout, SetForegroundColor(Color::DarkGrey))?;
    if !results.is_empty() {
        raw_line(
            stdout,
            &tui_truncate(
                &format!(
                    "  {} of {} results  |  up/down navigate  enter install  esc quit",
                    results.len(),
                    total
                ),
                w,
            ),
        )?;
    } else {
        raw_line(stdout, "  esc quit")?;
    }
    execute!(stdout, ResetColor)?;

    stdout.flush()?;
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
