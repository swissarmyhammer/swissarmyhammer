//! Terminal-aware table utilities.
//!
//! Provides a pre-configured comfy_table that respects terminal width,
//! preventing ugly line wrapping on narrow screens.

use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};

/// Create a table pre-configured for terminal-width-aware output.
///
/// Uses crossterm to detect the actual terminal width, falling back to
/// 120 columns when not connected to a TTY.
pub fn new_table() -> Table {
    let width = crossterm::terminal::size()
        .map(|(w, _)| w)
        .unwrap_or(120);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_width(width);
    table
}

/// Truncate a string to `max` characters, appending "..." if truncated.
///
/// Safe for multi-byte (UTF-8) strings.
pub fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

/// Shorten a package name for display.
///
/// Strips the `https://github.com/` prefix to save space in tables.
pub fn short_name(name: &str) -> String {
    name.strip_prefix("https://github.com/")
        .unwrap_or(name)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_str_unicode() {
        // Japanese characters are multi-byte
        let s = "こんにちは世界テスト";
        assert_eq!(truncate_str(s, 6), "こんに...");
    }

    #[test]
    fn test_short_name_github() {
        assert_eq!(
            short_name("https://github.com/owner/repo/skill"),
            "owner/repo/skill"
        );
    }

    #[test]
    fn test_short_name_no_prefix() {
        assert_eq!(short_name("my-local-skill"), "my-local-skill");
    }

    #[test]
    fn test_new_table() {
        let table = new_table();
        // Should not panic, and should have dynamic arrangement
        drop(table);
    }
}
