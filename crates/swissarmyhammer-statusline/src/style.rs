//! Parse style strings like "green bold" into ANSI escape codes.

/// A parsed ANSI style that can wrap text.
#[derive(Debug, Clone, Default)]
pub struct Style {
    codes: Vec<u8>,
}

impl Style {
    /// Parse a style string like "green bold" or "red dim".
    pub fn parse(s: &str) -> Self {
        let mut codes = Vec::new();
        for token in s.split_whitespace() {
            if let Some(code) = token_to_code(token) {
                codes.push(code);
            }
        }
        Self { codes }
    }

    /// Apply this style to text, returning styled text with ANSI codes.
    pub fn apply(&self, text: &str) -> String {
        if self.codes.is_empty() || text.is_empty() {
            return text.to_string();
        }
        let open: String = self.codes.iter().map(|c| format!("\x1b[{}m", c)).collect();
        format!("{}{}\x1b[0m", open, text)
    }

    /// Return a dimmed version of this style.
    pub fn dimmed(&self) -> Self {
        let mut codes = self.codes.clone();
        if !codes.contains(&2) {
            codes.push(2);
        }
        Self { codes }
    }
}

fn token_to_code(token: &str) -> Option<u8> {
    match token.to_lowercase().as_str() {
        // Modifiers
        "bold" => Some(1),
        "dim" | "dimmed" => Some(2),
        "italic" => Some(3),
        "underline" => Some(4),
        // Foreground colors
        "black" => Some(30),
        "red" => Some(31),
        "green" => Some(32),
        "yellow" => Some(33),
        "blue" => Some(34),
        "magenta" | "purple" => Some(35),
        "cyan" => Some(36),
        "white" => Some(37),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_color() {
        let s = Style::parse("green");
        assert_eq!(s.apply("hello"), "\x1b[32mhello\x1b[0m");
    }

    #[test]
    fn test_parse_color_with_modifier() {
        let s = Style::parse("red bold");
        let result = s.apply("err");
        assert!(result.contains("\x1b[31m"));
        assert!(result.contains("\x1b[1m"));
        assert!(result.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_empty_style() {
        let s = Style::parse("");
        assert_eq!(s.apply("hello"), "hello");
    }

    #[test]
    fn test_empty_text() {
        let s = Style::parse("green");
        assert_eq!(s.apply(""), "");
    }

    #[test]
    fn test_purple_alias() {
        let s = Style::parse("purple");
        assert_eq!(s.apply("x"), "\x1b[35mx\x1b[0m");
    }

    #[test]
    fn test_dim() {
        let s = Style::parse("dim");
        assert_eq!(s.apply("x"), "\x1b[2mx\x1b[0m");
    }

    #[test]
    fn test_dimmed_method() {
        let s = Style::parse("green");
        let d = s.dimmed();
        let result = d.apply("x");
        assert!(result.contains("\x1b[32m"));
        assert!(result.contains("\x1b[2m"));
    }

    #[test]
    fn test_dimmed_already_dim() {
        let s = Style::parse("dim");
        let d = s.dimmed();
        // Should not add dim twice
        let result = d.apply("x");
        assert_eq!(result.matches("\x1b[2m").count(), 1);
    }

    #[test]
    fn test_italic() {
        let s = Style::parse("italic");
        assert_eq!(s.apply("x"), "\x1b[3mx\x1b[0m");
    }

    #[test]
    fn test_underline() {
        let s = Style::parse("underline");
        assert_eq!(s.apply("x"), "\x1b[4mx\x1b[0m");
    }

    #[test]
    fn test_all_colors() {
        for (name, code) in &[
            ("black", 30),
            ("red", 31),
            ("green", 32),
            ("yellow", 33),
            ("blue", 34),
            ("magenta", 35),
            ("cyan", 36),
            ("white", 37),
        ] {
            let s = Style::parse(name);
            assert_eq!(s.apply("x"), format!("\x1b[{}mx\x1b[0m", code));
        }
    }

    #[test]
    fn test_unknown_token() {
        let s = Style::parse("unknown");
        // Unknown tokens are ignored, so no codes
        assert_eq!(s.apply("x"), "x");
    }

    #[test]
    fn test_dimmed_alias() {
        let s = Style::parse("dimmed");
        assert_eq!(s.apply("x"), "\x1b[2mx\x1b[0m");
    }

    #[test]
    fn test_default_style() {
        let s = Style::default();
        assert_eq!(s.apply("x"), "x");
    }

    #[test]
    fn test_multiple_modifiers() {
        let s = Style::parse("bold italic underline");
        let result = s.apply("text");
        assert!(result.contains("\x1b[1m"));
        assert!(result.contains("\x1b[3m"));
        assert!(result.contains("\x1b[4m"));
    }

    #[test]
    fn test_case_insensitive() {
        let s = Style::parse("GREEN BOLD");
        let result = s.apply("x");
        assert!(result.contains("\x1b[32m"));
        assert!(result.contains("\x1b[1m"));
    }

    #[test]
    fn test_dimmed_from_default() {
        let s = Style::default();
        let d = s.dimmed();
        let result = d.apply("x");
        assert!(result.contains("\x1b[2m"));
    }

    #[test]
    fn test_multiple_unknown_tokens() {
        let s = Style::parse("foo bar baz");
        assert_eq!(s.apply("x"), "x");
    }

    #[test]
    fn test_mixed_known_and_unknown() {
        let s = Style::parse("red foo bold");
        let result = s.apply("x");
        assert!(result.contains("\x1b[31m"));
        assert!(result.contains("\x1b[1m"));
    }

    #[test]
    fn test_clone_style() {
        let s = Style::parse("green bold");
        let c = s.clone();
        assert_eq!(s.apply("x"), c.apply("x"));
    }

    #[test]
    fn test_debug_style() {
        let s = Style::parse("red");
        let debug = format!("{:?}", s);
        assert!(debug.contains("31"));
    }
}
