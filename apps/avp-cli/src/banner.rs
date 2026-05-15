//! Branded ASCII banner for AVP CLI.
//!
//! Displays a validation hub icon alongside colored "AVP" block text,
//! with a charcoal/grey gradient matching the AVP logo.

use std::io::{self, IsTerminal, Write};

/// ANSI 256-color codes for a white-to-charcoal gradient.
const COLORS: [&str; 7] = [
    "\x1b[38;5;255m", // bright white
    "\x1b[38;5;252m", // light grey
    "\x1b[38;5;249m", // silver
    "\x1b[38;5;245m", // mid grey
    "\x1b[38;5;242m", // dark grey
    "\x1b[38;5;239m", // charcoal
    "\x1b[38;5;236m", // deep charcoal
];

/// ANSI escape code for dim/faint text.
const DIM: &str = "\x1b[2m";

/// ANSI escape code to reset all text formatting.
const RESET: &str = "\x1b[0m";

/// Validation hub icon + ANSI Shadow "AVP".
///
/// Hub/node with spokes and corner brackets, inspired by the AVP logo:
/// a central bullseye with radiating spokes and corner frames.
const LOGO: [&str; 7] = [
    r"  ╔═╗       ●       ╔═╗     █████╗ ██╗   ██╗██████╗ ",
    r"  ║ ╚╗    ● │ ●    ╔╝ ║    ██╔══██╗██║   ██║██╔══██╗",
    r"  ╚╗  ● ╭───────╮ ●  ╔╝   ███████║██║   ██║██████╔╝",
    r"   ╚──●─┤  (●)  ├─●──╝    ██╔══██║╚██╗ ██╔╝██╔═══╝ ",
    r"  ╔╗  ● ╰───────╯ ●  ╗╔   ██║  ██║ ╚████╔╝ ██║     ",
    r"  ║ ╔╝    ● │ ●    ╚╗ ║   ╚═╝  ╚═╝  ╚═══╝  ╚═╝     ",
    r"  ╚═╝       ●       ╚═╝                              ",
];

/// Render the banner to the given writer.
///
/// When `use_color` is true, each line gets a gradient color code.
/// When false, plain text is emitted.
fn render_banner(out: &mut dyn Write, use_color: bool) {
    // Banner output is best-effort; nothing useful to do if stdout is unavailable.
    let _ = writeln!(out);
    for (i, line) in LOGO.iter().enumerate() {
        if use_color {
            let _ = writeln!(out, "{}{}{}", COLORS[i], line, RESET);
        } else {
            let _ = writeln!(out, "{}", line);
        }
    }
    let _ = writeln!(out);
    if use_color {
        let _ = writeln!(out, "  {}Agent Validator Protocol{}", DIM, RESET);
    } else {
        let _ = writeln!(out, "  Agent Validator Protocol");
    }
    let _ = writeln!(out);
}

/// Print the branded banner to stdout.
///
/// Respects `NO_COLOR` env var and non-TTY output by falling back to
/// plain (uncolored) text.
pub fn print_banner() {
    let use_color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut out = io::stdout().lock();
    render_banner(&mut out, use_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_plain_contains_expected_text() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("Agent Validator Protocol"));
        // Block-letter P fragment from AVP
        assert!(output.contains("██████╔╝"));
        // No ANSI codes in plain mode
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn banner_colored_contains_ansi_codes() {
        let mut buf = Vec::new();
        render_banner(&mut buf, true);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("Agent Validator Protocol"));
        // Block-letter P fragment from AVP
        assert!(output.contains("██████╔╝"));
        // Should contain ANSI color codes
        assert!(output.contains("\x1b[38;5;"));
        assert!(output.contains(RESET));
    }

    #[test]
    fn banner_has_logo_icon_elements() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        // Hub/spoke icon elements
        assert!(output.contains("(●)"));
        // Corner bracket elements
        assert!(output.contains("╔═╗"));
    }
}
