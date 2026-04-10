//! Branded ASCII banner for code-context CLI.
//!
//! Displays a magnifying glass ASCII art alongside colored "CODE-CONTEXT" block
//! text, with a blue-to-cyan gradient matching the search/lens theme.

use std::io::{self, IsTerminal, Write};

/// ANSI 256-color codes for a bright-cyan-to-deep-blue gradient.
const COLORS: [&str; 7] = [
    "\x1b[38;5;45m", // bright cyan
    "\x1b[38;5;39m", // medium cyan
    "\x1b[38;5;33m", // blue-cyan
    "\x1b[38;5;27m", // medium blue
    "\x1b[38;5;21m", // bright blue
    "\x1b[38;5;20m", // dark blue
    "\x1b[38;5;19m", // deep blue
];

/// ANSI escape code for dim/faint text.
const DIM: &str = "\x1b[2m";

/// ANSI escape code to reset all text formatting.
const RESET: &str = "\x1b[0m";

/// Magnifying glass ASCII art + "CODE" in ANSI Shadow block font (7 lines).
///
/// Left column: magnifying glass icon (lens + handle).
/// Right column: ANSI Shadow block letters spelling "CODE".
/// A second pass renders "CONTEXT" below to complete "CODE-CONTEXT".
const CODE_LINES: [&str; 7] = [
    r"  .---.    ██████╗ ██████╗ ██████╗ ███████╗",
    r" /  O  \   ██╔════╝██╔═══██╗██╔══██╗██╔════╝",
    r"|  ( )  |  ██║     ██║   ██║██║  ██║█████╗  ",
    r" \  O  /   ██║     ██║   ██║██║  ██║██╔══╝  ",
    r"  '---'    ╚██████╗╚██████╔╝██████╔╝███████╗",
    r"    |       ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝",
    r"    |___   ██████╗ ██████╗ ███╗   ██╗████████╗███████╗██╗  ██╗████████╗",
];

/// "CONTEXT" in ANSI Shadow block font (5 lines) -- continues below CODE_LINES.
const CONTEXT_LINES: [&str; 5] = [
    r"          ██╔════╝██╔═══██╗████╗  ██║╚══██╔══╝██╔════╝╚██╗██╔╝╚══██╔══╝",
    r"          ██║     ██║   ██║██╔██╗ ██║   ██║   █████╗   ╚███╔╝    ██║   ",
    r"          ██║     ██║   ██║██║╚██╗██║   ██║   ██╔══╝   ██╔██╗    ██║   ",
    r"          ╚██████╗╚██████╔╝██║ ╚████║   ██║   ███████╗██╔╝ ██╗   ██║   ",
    r"           ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝   ╚═╝   ╚══════╝╚═╝  ╚═╝   ╚═╝   ",
];

/// Render the banner to the given writer.
///
/// When `use_color` is true, each line gets a gradient color code drawn from
/// blue-to-cyan tones. When false, plain text is emitted (for NO_COLOR / non-TTY).
pub fn render_banner(out: &mut dyn Write, use_color: bool) {
    // Banner output is best-effort; nothing useful to do if stdout is unavailable.
    let _ = writeln!(out);
    for (i, line) in CODE_LINES.iter().enumerate() {
        if use_color {
            let _ = writeln!(out, "{}{}{}", COLORS[i], line, RESET);
        } else {
            let _ = writeln!(out, "{}", line);
        }
    }
    // Render the "CONTEXT" portion of the block font
    for line in &CONTEXT_LINES {
        if use_color {
            let _ = writeln!(out, "{}{}{}", COLORS[6], line, RESET);
        } else {
            let _ = writeln!(out, "{}", line);
        }
    }
    let _ = writeln!(out);
    if use_color {
        let _ = writeln!(
            out,
            "  {}Code intelligence for AI agents — symbols, search, and call graphs{}",
            DIM, RESET
        );
    } else {
        let _ = writeln!(
            out,
            "  Code intelligence for AI agents — symbols, search, and call graphs"
        );
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

/// Determine whether the banner should be shown given the CLI arguments.
///
/// Returns `true` when:
/// - No arguments are given and stdin is a terminal (interactive use), or
/// - The first argument is `--help` or `-h`
///
/// Returns `false` otherwise (piped input, subcommands, etc.).
pub fn should_show_banner(args: &[String]) -> bool {
    match args.len() {
        // args[0] is the program name; 1 means no user-supplied arguments
        1 => io::stdin().is_terminal(),
        2 => args[1] == "--help" || args[1] == "-h",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_plain_contains_tagline() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(
            output.contains("Code intelligence for AI agents — symbols, search, and call graphs")
        );
        // No ANSI codes in plain mode
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn banner_colored_contains_ansi_codes() {
        let mut buf = Vec::new();
        render_banner(&mut buf, true);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(
            output.contains("Code intelligence for AI agents — symbols, search, and call graphs")
        );
        // Should contain ANSI color codes (blue-to-cyan gradient)
        assert!(output.contains("\x1b[38;5;"));
        assert!(output.contains(RESET));
    }

    #[test]
    fn banner_has_magnifying_glass() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        // Magnifying glass lens element in the ASCII art
        assert!(output.contains(".---."));
        // Handle portion
        assert!(output.contains("|___"));
    }

    #[test]
    fn should_show_banner_help_flags() {
        // --help and -h should always show the banner
        let args_help = vec!["code-context".to_string(), "--help".to_string()];
        assert!(should_show_banner(&args_help));

        let args_h = vec!["code-context".to_string(), "-h".to_string()];
        assert!(should_show_banner(&args_h));
    }

    #[test]
    fn should_show_banner_subcommand_returns_false() {
        let args = vec!["code-context".to_string(), "serve".to_string()];
        assert!(!should_show_banner(&args));
    }
}
