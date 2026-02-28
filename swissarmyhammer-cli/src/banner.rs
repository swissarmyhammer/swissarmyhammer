//! Branded ASCII banner for SwissArmyHammer CLI.
//!
//! Displays the SwissArmyHammer logo alongside colored "SAH" block text,
//! with a red gradient matching the Swiss Army branding.

use std::io::{self, IsTerminal, Write};

/// ANSI 256-color codes for a 19-step red gradient (bright -> dark).
const COLORS: [&str; 19] = [
    "\x1b[38;5;224m", // very light pink
    "\x1b[38;5;224m", // very light pink
    "\x1b[38;5;217m", // light pink
    "\x1b[38;5;210m", // light salmon
    "\x1b[38;5;203m", // salmon
    "\x1b[38;5;196m", // bright red
    "\x1b[38;5;196m", // bright red
    "\x1b[38;5;167m", // medium pink
    "\x1b[38;5;160m", // red
    "\x1b[38;5;160m", // red
    "\x1b[38;5;131m", // dark pink
    "\x1b[38;5;124m", // dark red
    "\x1b[38;5;124m", // dark red
    "\x1b[38;5;95m",  // very dark pink
    "\x1b[38;5;88m",  // deep red
    "\x1b[38;5;88m",  // deep red
    "\x1b[38;5;52m",  // darkest red
    "\x1b[38;5;52m",  // darkest red
    "\x1b[38;5;52m",  // darkest red
];

/// ANSI escape code for dim/faint text.
const DIM: &str = "\x1b[2m";

/// ANSI escape code to reset all text formatting.
const RESET: &str = "\x1b[0m";

/// SwissArmyHammer logo + ANSI Shadow "SAH".
///
/// ASCII rendition of the SwissArmyHammer logo — hammer head at
/// upper-left with a diagonal handle — alongside block-letter "SAH".
const LOGO: [&str; 19] = [
    r"",
    r"",
    r"           =#+=#-=",
    r"         ====+=",
    r"         . -==*",
    r"      =#+:    :+",
    r"       +.       =-              ███████╗ █████╗ ██╗  ██╗",
    r"                 ==.            ██╔════╝██╔══██╗██║  ██║",
    r"                   -#:          ███████╗███████║███████║",
    r"                      *#        ╚════██║██╔══██║██╔══██║",
    r"                      ###=      ███████║██║  ██║██║  ██║",
    r"                        ###+    ╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝",
    r"                         ####+",
    r"                           ####*",
    r"                             ###+#",
    r"                               ###*=",
    r"                                #*#",
    r"",
    r"",
];

/// Render the banner to the given writer.
///
/// When `use_color` is true, each line gets a gradient color code.
/// When false, plain text is emitted.
fn render_banner(out: &mut dyn Write, use_color: bool) {
    // Best-effort: banner is cosmetic, so write failures are non-fatal.
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
            "  {}SwissArmyHammer — the coding agent's toolkit{}",
            DIM, RESET
        );
    } else {
        let _ = writeln!(out, "  SwissArmyHammer — the coding agent's toolkit");
    }
    let _ = writeln!(out);
}

/// Check whether the banner should be shown based on CLI arguments.
///
/// Returns true when no subcommand is given (bare invocation) or when
/// the only argument is `--help` / `-h`.
pub(crate) fn should_show_banner(args: &[String]) -> bool {
    match args.len() {
        1 => io::stdin().is_terminal(),
        2 => args[1] == "--help" || args[1] == "-h",
        _ => false,
    }
}

/// Print the branded banner to stdout.
///
/// Respects `NO_COLOR` env var and non-TTY output by falling back to
/// plain (uncolored) text.
pub(crate) fn print_banner() {
    let use_color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let mut out = io::stdout().lock();
    render_banner(&mut out, use_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_banner_bare_invocation_when_not_terminal() {
        // In tests (and hook mode), stdin is not a terminal — no banner
        let args = vec!["sah".to_string()];
        assert!(!should_show_banner(&args));
    }

    #[test]
    fn show_banner_with_help_flag() {
        let args = vec!["sah".to_string(), "--help".to_string()];
        assert!(should_show_banner(&args));
    }

    #[test]
    fn show_banner_with_short_help_flag() {
        let args = vec!["sah".to_string(), "-h".to_string()];
        assert!(should_show_banner(&args));
    }

    #[test]
    fn no_banner_with_subcommand() {
        let args = vec!["sah".to_string(), "serve".to_string()];
        assert!(!should_show_banner(&args));
    }

    #[test]
    fn no_banner_with_multiple_args() {
        let args = vec!["sah".to_string(), "files".to_string(), "read".to_string()];
        assert!(!should_show_banner(&args));
    }

    #[test]
    fn banner_plain_contains_expected_text() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("SwissArmyHammer"));
        // Block-letter H fragment from SAH
        assert!(output.contains("███████║"));
        // No ANSI codes in plain mode
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn banner_colored_contains_ansi_codes() {
        let mut buf = Vec::new();
        render_banner(&mut buf, true);
        let output = String::from_utf8(buf).expect("valid utf8");
        assert!(output.contains("SwissArmyHammer"));
        // Should contain ANSI color codes
        assert!(output.contains("\x1b[38;5;"));
        assert!(output.contains(RESET));
    }

    #[test]
    fn print_banner_does_not_panic() {
        // Smoke-test the entry point; NO_COLOR suppresses ANSI codes.
        std::env::set_var("NO_COLOR", "1");
        print_banner();
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn banner_plain_matches_logo() {
        let mut buf = Vec::new();
        render_banner(&mut buf, false);
        let output = String::from_utf8(buf).expect("valid utf8");
        // Every LOGO line must appear verbatim in the rendered output.
        for line in &LOGO {
            assert!(output.contains(line), "missing logo line: {:?}", line);
        }
    }

    #[test]
    fn logo_and_colors_same_length() {
        assert_eq!(LOGO.len(), COLORS.len());
    }
}
