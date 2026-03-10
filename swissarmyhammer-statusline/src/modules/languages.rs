//! Languages module - shows language icons for detected languages.

use crate::module::{ModuleContext, ModuleOutput};
use crate::style::Style;

/// A language with its file extensions, display icon, and known LSP servers.
struct LanguageIcon {
    extensions: &'static [&'static str],
    icon: &'static str,
    lsp_servers: &'static [&'static str],
}

const LANGUAGE_ICONS: &[LanguageIcon] = &[
    LanguageIcon {
        extensions: &["rs"],
        icon: "\u{1f980}",
        lsp_servers: &["rust-analyzer"],
    },
    LanguageIcon {
        extensions: &["py"],
        icon: "\u{1f40d}",
        lsp_servers: &["pyright", "pylsp"],
    },
    LanguageIcon {
        extensions: &["ts", "tsx"],
        icon: "\u{1f4dc}",
        lsp_servers: &["typescript-language-server"],
    },
    LanguageIcon {
        extensions: &["js", "jsx"],
        icon: "\u{1f4dc}",
        lsp_servers: &["typescript-language-server"],
    },
    LanguageIcon {
        extensions: &["go"],
        icon: "\u{1f439}",
        lsp_servers: &["gopls"],
    },
    LanguageIcon {
        extensions: &["java"],
        icon: "\u{2615}",
        lsp_servers: &["jdtls"],
    },
    LanguageIcon {
        extensions: &["rb"],
        icon: "\u{1f48e}",
        lsp_servers: &["solargraph"],
    },
    LanguageIcon {
        extensions: &["swift"],
        icon: "\u{1f426}",
        lsp_servers: &["sourcekit-lsp"],
    },
];

/// Evaluate the languages module.
///
/// Queries the code-context database for indexed file extensions, then shows
/// icons for detected languages. Icons are dimmed when the corresponding
/// LSP server is not found in PATH.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    // Try to open code-context workspace to see what languages are indexed
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return ModuleOutput::hidden(),
    };

    let ws = match swissarmyhammer_code_context::CodeContextWorkspace::open(&cwd) {
        Ok(ws) => ws,
        Err(_) => return ModuleOutput::hidden(),
    };

    let conn = ws.db();

    // Query extensions from indexed_files
    let extensions: Vec<String> = match conn.prepare(
        "SELECT DISTINCT substr(path, instr(path, '.') + 1) FROM indexed_files WHERE path LIKE '%.%'",
    ) {
        Ok(mut stmt) => stmt
            .query_map([], |row| row.get(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default(),
        Err(_) => return ModuleOutput::hidden(),
    };

    if extensions.is_empty() {
        return ModuleOutput::hidden();
    }

    let cfg = &ctx.config.languages;
    let mut icons = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for lang_icon in LANGUAGE_ICONS {
        let has_ext = lang_icon
            .extensions
            .iter()
            .any(|ext| extensions.iter().any(|e| e == ext));
        if !has_ext {
            continue;
        }
        if !seen.insert(lang_icon.icon) {
            continue;
        }

        let has_lsp = lang_icon
            .lsp_servers
            .iter()
            .any(|server| swissarmyhammer_code_context::find_executable(server).is_some());

        if has_lsp || !cfg.dim_without_lsp {
            icons.push(lang_icon.icon.to_string());
        } else {
            // Dim the icon by wrapping in ANSI dim
            icons.push(format!("\x1b[2m{}\x1b[22m", lang_icon.icon));
        }
    }

    if icons.is_empty() {
        return ModuleOutput::hidden();
    }

    let text = icons.join(" ");
    ModuleOutput::new(text, Style::parse(&cfg.style))
}
