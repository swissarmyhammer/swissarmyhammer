//! Languages module - shows language icons based on code-context indexed files.
//!
//! Derives language presence from the actual file extensions tracked in the
//! code-context database, rather than project marker files. This correctly
//! handles monorepos where languages appear at any nesting depth.

use crate::module::{ModuleContext, ModuleOutput};
use crate::style::Style;

/// A language with its file extensions, display icon, and known LSP servers.
struct LanguageEntry {
    /// File extensions that indicate this language (without the dot).
    extensions: &'static [&'static str],
    /// Display icon for the statusline.
    icon: &'static str,
    /// LSP server executables for this language.
    lsp_servers: &'static [&'static str],
}

const LANGUAGES: &[LanguageEntry] = &[
    LanguageEntry {
        extensions: &["rs"],
        icon: "\u{1f980}",
        lsp_servers: &["rust-analyzer"],
    },
    LanguageEntry {
        extensions: &["py"],
        icon: "\u{1f40d}",
        lsp_servers: &["pyright", "pylsp"],
    },
    LanguageEntry {
        extensions: &["ts", "tsx", "js", "jsx"],
        icon: "\u{1f4dc}",
        lsp_servers: &["typescript-language-server"],
    },
    LanguageEntry {
        extensions: &["go"],
        icon: "\u{1f439}",
        lsp_servers: &["gopls"],
    },
    LanguageEntry {
        extensions: &["java"],
        icon: "\u{2615}",
        lsp_servers: &["jdtls"],
    },
    LanguageEntry {
        extensions: &["cs"],
        icon: "\u{1f4bb}",
        lsp_servers: &["omnisharp"],
    },
    LanguageEntry {
        extensions: &["c", "cpp", "cc", "cxx", "h", "hpp", "hxx"],
        icon: "\u{2699}\u{fe0f}",
        lsp_servers: &["clangd"],
    },
    LanguageEntry {
        extensions: &["dart"],
        icon: "\u{1f426}",
        lsp_servers: &["dart"],
    },
    LanguageEntry {
        extensions: &["php"],
        icon: "\u{1f418}",
        lsp_servers: &["intelephense"],
    },
    LanguageEntry {
        extensions: &["rb"],
        icon: "\u{1f48e}",
        lsp_servers: &["solargraph"],
    },
    LanguageEntry {
        extensions: &["swift"],
        icon: "\u{1f426}",
        lsp_servers: &["sourcekit-lsp"],
    },
    LanguageEntry {
        extensions: &["kt", "kts"],
        icon: "\u{1f4a0}",
        lsp_servers: &["kotlin-language-server"],
    },
];

/// Query whether the code-context database has any indexed files with the given extension.
fn has_extension(conn: &rusqlite::Connection, ext: &str) -> bool {
    let pattern = format!("%.{}", ext);
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM indexed_files WHERE file_path LIKE ?1 LIMIT 1)",
        [&pattern],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// Evaluate the languages module.
///
/// Queries the code-context database for actual file extensions, then shows
/// icons for each detected language. Icons are dimmed when the corresponding
/// LSP server is not found in PATH.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return ModuleOutput::hidden(),
    };

    let ws = match swissarmyhammer_code_context::CodeContextWorkspace::open(&cwd) {
        Ok(ws) => ws,
        Err(_) => return ModuleOutput::hidden(),
    };

    let conn = ws.db();
    let cfg = &ctx.config.languages;
    let mut icons = Vec::new();

    for lang in LANGUAGES {
        let has_files = lang.extensions.iter().any(|ext| has_extension(&conn, ext));
        if !has_files {
            continue;
        }

        let has_lsp = lang
            .lsp_servers
            .iter()
            .any(|server| swissarmyhammer_code_context::find_executable(server).is_some());

        if has_lsp || !cfg.dim_without_lsp {
            icons.push(lang.icon.to_string());
        } else {
            icons.push(format!("\x1b[2m{}\x1b[22m", lang.icon));
        }
    }

    if icons.is_empty() {
        return ModuleOutput::hidden();
    }

    let text = icons.join(" ");
    ModuleOutput::new(text, Style::parse(&cfg.style))
}
