//! Branded ASCII banner for the Kanban CLI.
//!
//! Displays a kanban board motif alongside colored "KANBAN" block text,
//! with a blue-cyan gradient.

use std::io::{self, IsTerminal, Write};

/// ANSI 256-color codes for a 17-step blue/cyan gradient (bright -> dark).
const COLORS: [&str; 17] = [
    "\x1b[38;5;159m", // light cyan
    "\x1b[38;5;123m", // cyan
    "\x1b[38;5;87m",  // bright cyan
    "\x1b[38;5;81m",  // sky blue
    "\x1b[38;5;75m",  // medium blue
    "\x1b[38;5;69m",  // blue
    "\x1b[38;5;63m",  // blue-purple
    "\x1b[38;5;33m",  // deeper blue
    "\x1b[38;5;32m",  // ocean blue
    "\x1b[38;5;31m",  // dark cyan
    "\x1b[38;5;30m",  // teal
    "\x1b[38;5;29m",  // dark teal
    "\x1b[38;5;24m",  // deep blue
    "\x1b[38;5;23m",  // darker blue
    "\x1b[38;5;23m",  // darker blue
    "\x1b[38;5;17m",  // navy
    "\x1b[38;5;17m",  // navy
];

/// ANSI escape code for dim/faint text.
const DIM: &str = "\x1b[2m";

/// ANSI escape code to reset all text formatting.
const RESET: &str = "\x1b[0m";

/// Kanban board motif + block-letter "KANBAN".
const LOGO: [&str; 17] = [
    r"",
    r"  ┌────────┬────────┬────────┐",
    r"  │  TODO  │ DOING  │  DONE  │",
    r"  ├────────┼────────┼────────┤",
    r"  │ ┌────┐ │ ┌────┐ │ ┌────┐ │  ██╗  ██╗ █████╗ ███╗   ██╗",
    r"  │ │ ** │ │ │ >> │ │ │ ok │ │  ██║ ██╔╝██╔══██╗████╗  ██║",
    r"  │ └────┘ │ └────┘ │ └────┘ │  █████╔╝ ███████║██╔██╗ ██║",
    r"  │ ┌────┐ │        │ ┌────┐ │  ██╔═██╗ ██╔══██║██║╚██╗██║",
    r"  │ │ ** │ │        │ │ ok │ │  ██║  ██╗██║  ██║██║ ╚████║",
    r"  │ └────┘ │        │ └────┘ │  ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝",
    r"  │ ┌────┐ │        │        │",
    r"  │ │ ** │ │        │  ████████╗  █████╗  ███╗   ██╗",
    r"  │ └────┘ │        │  ██╔═══██║ ██╔══██╗ ████╗  ██║",
    r"  │        │        │  ████████║ ███████║ ██╔██╗ ██║",
    r"  │        │        │  ██╔═══██║ ██╔══██║ ██║╚██╗██║",
    r"  └────────┴────────┘  ████████║ ██║  ██║ ██║ ╚████║",
    r"                       ╚═══════╝ ╚═╝  ╚═╝ ╚═╝  ╚═══╝",
];

/// Render the banner to the given writer.
fn render_banner(out: &mut dyn Write, use_color: bool) {
    for (i, line) in LOGO.iter().enumerate() {
        if use_color {
            let _ = writeln!(out, "{}{}{}", COLORS[i], line, RESET);
        } else {
            let _ = writeln!(out, "{}", line);
        }
    }
    if use_color {
        let _ = writeln!(
            out,
            "  {}Kanban — Agent-driven project management{}",
            DIM, RESET
        );
    } else {
        let _ = writeln!(out, "  Kanban — Agent-driven project management");
    }
    let _ = writeln!(out);
}

/// Check whether the banner should be shown based on CLI arguments.
pub fn should_show_banner(args: &[String]) -> bool {
    match args.len() {
        1 => io::stdin().is_terminal(),
        2 => args[1] == "--help" || args[1] == "-h",
        _ => false,
    }
}

/// Print the branded banner to stderr.
pub fn print_banner() {
    let use_color = io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut out = io::stderr().lock();
    render_banner(&mut out, use_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_and_colors_same_length() {
        assert_eq!(LOGO.len(), COLORS.len());
    }

    #[test]
    fn banner_plain_contains_expected_text() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("Kanban"));
        assert!(output.contains("TODO"));
        assert!(output.contains("DOING"));
        assert!(output.contains("DONE"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn banner_colored_contains_ansi_codes() {
        let mut buf = Vec::new();
        render_banner(&mut buf, true);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("\x1b[38;5;"));
        assert!(output.contains(RESET));
    }

    #[test]
    fn no_banner_with_subcommand() {
        let args = vec!["kanban".to_string(), "task".to_string()];
        assert!(!should_show_banner(&args));
    }

    #[test]
    fn show_banner_with_help_flag() {
        let args = vec!["kanban".to_string(), "--help".to_string()];
        assert!(should_show_banner(&args));
    }
}
