//! Branded ASCII banner for shelltool CLI.
//!
//! Displays a soldier turtle ASCII art alongside colored "SHELLTOOL" block text,
//! with a green gradient matching the turtle's military theme.

use std::io::{self, IsTerminal, Write};

/// ANSI 256-color codes for a bright-to-deep green gradient.
const COLORS: [&str; 7] = [
    "\x1b[38;5;154m", // bright yellow-green
    "\x1b[38;5;148m", // lime green
    "\x1b[38;5;112m", // medium green
    "\x1b[38;5;76m",  // forest green
    "\x1b[38;5;70m",  // dark green
    "\x1b[38;5;64m",  // olive green
    "\x1b[38;5;58m",  // deep olive
];

/// ANSI escape code for dim/faint text.
const DIM: &str = "\x1b[2m";

/// ANSI escape code to reset all text formatting.
const RESET: &str = "\x1b[0m";

/// Soldier turtle ASCII art + "SHELL" in ANSI Shadow block font (7 lines).
///
/// Left column: cartoon turtle in military gear (helmet, shell, rifle, bandolier).
/// Right column: ANSI Shadow block letters spelling "SHELL".
/// A second pass renders "TOOL" below to complete "SHELLTOOL".
const SHELL_LINES: [&str; 7] = [
    r"  ,--,     ███████╗██╗  ██╗███████╗██╗     ██╗",
    r" (o__o)    ██╔════╝██║  ██║██╔════╝██║     ██║",
    r"(  ___  )  ███████╗███████║█████╗  ██║     ██║",
    r" \(___)/=| ╚════██║██╔══██║██╔══╝  ██║     ██║",
    r" /|___|\ | ███████║██║  ██║███████╗███████╗███████╗",
    r"( shell ) \\╚══════╝╚═╝  ╚═╝╚══════╝╚══════╝╚══════╝",
    r" ~(___)~  ████████╗ ██████╗  ██████╗ ██╗     ",
];

/// "TOOL" in ANSI Shadow block font (5 lines) — continues below SHELL_LINES.
const TOOL_LINES: [&str; 5] = [
    r"          ╚══██╔══╝██╔═══██╗██╔═══██╗██║     ",
    r"             ██║   ██║   ██║██║   ██║██║     ",
    r"             ██║   ██║   ██║██║   ██║██║     ",
    r"             ██║   ╚██████╔╝╚██████╔╝███████╗",
    r"             ╚═╝    ╚═════╝  ╚═════╝ ╚══════╝",
];

/// Render the banner to the given writer.
///
/// When `use_color` is true, each line gets a gradient color code drawn from
/// green tones. When false, plain text is emitted (for NO_COLOR / non-TTY).
pub fn render_banner(out: &mut dyn Write, use_color: bool) {
    // Banner output is best-effort; nothing useful to do if stdout is unavailable.
    let _ = writeln!(out);
    for (i, line) in SHELL_LINES.iter().enumerate() {
        if use_color {
            let _ = writeln!(out, "{}{}{}", COLORS[i], line, RESET);
        } else {
            let _ = writeln!(out, "{}", line);
        }
    }
    // Render the "TOOL" portion of the block font
    for line in &TOOL_LINES {
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
            "  {}Replaces Bash/exec — searchable shell that saves tokens{}",
            DIM, RESET
        );
    } else {
        let _ = writeln!(
            out,
            "  Replaces Bash/exec — searchable shell that saves tokens"
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
    fn banner_plain_contains_shelltool_text() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("Replaces Bash/exec — searchable shell that saves tokens"));
        // SHELL block-letter fragment
        assert!(output.contains("███████╗"));
        // TOOL block-letter fragment
        assert!(output.contains("╚══════╝"));
        // No ANSI codes in plain mode
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn banner_colored_contains_ansi_codes() {
        let mut buf = Vec::new();
        render_banner(&mut buf, true);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("Replaces Bash/exec — searchable shell that saves tokens"));
        // Should contain ANSI color codes (green gradient)
        assert!(output.contains("\x1b[38;5;"));
        assert!(output.contains(RESET));
    }

    #[test]
    fn banner_has_turtle_ascii_art() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        // Turtle shell element in the ASCII art
        assert!(output.contains("shell"));
    }

    #[test]
    fn should_show_banner_help_flags() {
        // --help and -h should always show the banner
        let args_help = vec!["shelltool".to_string(), "--help".to_string()];
        assert!(should_show_banner(&args_help));

        let args_h = vec!["shelltool".to_string(), "-h".to_string()];
        assert!(should_show_banner(&args_h));
    }

    #[test]
    fn should_show_banner_subcommand_returns_false() {
        let args = vec!["shelltool".to_string(), "serve".to_string()];
        assert!(!should_show_banner(&args));
    }

    #[test]
    fn should_show_banner_many_args_returns_false() {
        let args = vec![
            "shelltool".to_string(),
            "--help".to_string(),
            "extra".to_string(),
        ];
        assert!(!should_show_banner(&args));
    }
}
