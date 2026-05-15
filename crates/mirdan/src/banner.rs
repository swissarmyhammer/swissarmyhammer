//! Branded ASCII banner for Mirdan CLI.
//!
//! Displays a brilliant-cut diamond gem icon alongside colored "MIRDAN" block
//! text, with an orange gradient matching the Mirdan brand (#fb8c00 → #ff5722).

use std::io::{self, IsTerminal, Write};

/// ANSI 256-color codes for the orange gradient (light → dark).
const COLORS: [&str; 6] = [
    "\x1b[38;5;214m", // light orange
    "\x1b[38;5;208m", // bright orange  (#fb8c00)
    "\x1b[38;5;202m", // red-orange     (#ff5722)
    "\x1b[38;5;166m", // dark orange
    "\x1b[38;5;130m", // brown
    "\x1b[38;5;94m",  // dark brown
];
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Brilliant-cut diamond gem + ANSI Shadow "MIRDAN".
///
/// V-shaped: narrow table → wide girdle → 4 lines of pavilion to culet.
/// Text starts at column 18 on every line.
const LOGO: [&str; 6] = [
    "   ▄██████▄       ███╗   ███╗██╗██████╗ ██████╗  █████╗ ███╗   ██╗",
    "▄██▀▀████▀▀██▄    ████╗ ████║██║██╔══██╗██╔══██╗██╔══██╗████╗  ██║",
    " ▀██▄▀██▀▄██▀     ██╔████╔██║██║██████╔╝██║  ██║███████║██╔██╗ ██║",
    "   ▀██████▀       ██║╚██╔╝██║██║██╔══██╗██║  ██║██╔══██║██║╚██╗██║",
    "     ▀██▀         ██║ ╚═╝ ██║██║██║  ██║██████╔╝██║  ██║██║ ╚████║",
    "      ▀▀          ╚═╝     ╚═╝╚═╝╚═╝  ╚═╝╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═══╝",
];

/// Print the branded banner to stdout.
///
/// Respects `NO_COLOR` env var and non-TTY output by falling back to
/// plain (uncolored) text.
pub fn print_banner() {
    let use_color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();

    let mut out = io::stdout().lock();

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
        let _ = writeln!(out, "  {}The forge for AI coding agents{}", DIM, RESET);
    } else {
        let _ = writeln!(out, "  The forge for AI coding agents");
    }
    let _ = writeln!(out);
}
