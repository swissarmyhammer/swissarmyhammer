//! Languages module - shows language icons based on code-context indexed files.
//!
//! Derives language presence from the actual file extensions tracked in the
//! code-context database via `distinct_extensions()`, and maps extensions to
//! icons using the LSP registry rather than hardcoded tables.

use std::collections::BTreeSet;

use crate::module::{ModuleContext, ModuleOutput};
use crate::style::Style;

/// Default icon used when an LSP spec has no icon configured.
const DEFAULT_ICON: &str = "\u{1f4e6}";

/// A language icon entry with LSP availability info.
struct LangIcon {
    icon: String,
    has_lsp: bool,
}

/// Evaluate the languages module.
///
/// Queries the code-context database for actual file extensions, then looks up
/// matching LSP server specs from the registry. Shows the spec's icon for each
/// detected language. Icons are dimmed when the corresponding LSP server is not
/// found in PATH. Deduplicates icons so that multiple servers for the same
/// language (e.g. pyright and pylsp) only show one icon.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    try_eval(ctx).unwrap_or_else(ModuleOutput::hidden)
}

fn try_eval(ctx: &ModuleContext) -> Option<ModuleOutput> {
    let cwd = std::env::current_dir().ok()?;
    let ws = swissarmyhammer_code_context::CodeContextWorkspace::open(&cwd).ok()?;
    let conn = ws.db();

    let present_exts = swissarmyhammer_code_context::distinct_extensions(&conn).ok()?;
    if present_exts.is_empty() {
        return Some(ModuleOutput::hidden());
    }

    let ext_refs: Vec<&str> = present_exts.iter().map(|s| s.as_str()).collect();
    let matching_servers = swissarmyhammer_lsp::servers_for_extensions(&ext_refs);

    let lang_icons: Vec<LangIcon> = matching_servers
        .iter()
        .map(|spec| {
            let icon = spec.icon.as_deref().unwrap_or(DEFAULT_ICON).to_string();
            let has_lsp = swissarmyhammer_code_context::find_executable(&spec.command).is_some();
            LangIcon { icon, has_lsp }
        })
        .collect();

    Some(render_language_icons(&lang_icons, &ctx.config.languages))
}

/// Render language icons with optional dimming for missing LSP servers.
/// Deduplicates icons so that multiple servers for the same language show only one icon.
fn render_language_icons(
    lang_icons: &[LangIcon],
    cfg: &crate::config::LanguagesModuleConfig,
) -> ModuleOutput {
    if lang_icons.is_empty() {
        return ModuleOutput::hidden();
    }

    let mut seen = BTreeSet::new();
    let mut icons = Vec::new();
    for entry in lang_icons {
        if !seen.insert(&entry.icon) {
            continue;
        }
        if entry.has_lsp {
            icons.push(entry.icon.clone());
        } else {
            let indicator = &cfg.missing_lsp_indicator;
            if cfg.dim_without_lsp {
                icons.push(format!("\x1b[2m{}{}\x1b[22m", entry.icon, indicator));
            } else {
                icons.push(format!("{}{}", entry.icon, indicator));
            }
        }
    }

    let text = icons.join(" ");
    ModuleOutput::new(text, Style::parse(&cfg.style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatuslineConfig;
    use crate::input::StatuslineInput;

    #[test]
    fn test_languages_in_repo() {
        let input = StatuslineInput::default();
        let config = StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_languages_dim_disabled() {
        let input = StatuslineInput::default();
        let mut config = StatuslineConfig::default();
        config.languages.dim_without_lsp = false;
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        let _out = eval(&ctx);
    }

    #[test]
    fn test_render_language_icons_empty() {
        let cfg = StatuslineConfig::default().languages;
        let out = render_language_icons(&[], &cfg);
        assert!(out.is_empty());
    }

    #[test]
    fn test_render_language_icons_with_lsp() {
        let cfg = StatuslineConfig::default().languages;
        let icons = vec![LangIcon {
            icon: "\u{e7a8}".into(),
            has_lsp: true,
        }];
        let out = render_language_icons(&icons, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("\u{e7a8}"));
    }

    #[test]
    fn test_render_language_icons_missing_lsp_dimmed() {
        let cfg = StatuslineConfig::default().languages;
        let icons = vec![LangIcon {
            icon: "\u{e7a8}".into(),
            has_lsp: false,
        }];
        let out = render_language_icons(&icons, &cfg);
        assert!(!out.is_empty());
        assert!(out.text.contains("\x1b[2m"));
    }

    #[test]
    fn test_render_language_icons_missing_lsp_not_dimmed() {
        let mut cfg = StatuslineConfig::default().languages;
        cfg.dim_without_lsp = false;
        let icons = vec![LangIcon {
            icon: "\u{e7a8}".into(),
            has_lsp: false,
        }];
        let out = render_language_icons(&icons, &cfg);
        assert!(!out.is_empty());
        assert!(!out.text.contains("\x1b[2m"));
    }

    #[test]
    fn test_render_language_icons_mixed() {
        let cfg = StatuslineConfig::default().languages;
        let icons = vec![
            LangIcon {
                icon: "R".into(),
                has_lsp: true,
            },
            LangIcon {
                icon: "P".into(),
                has_lsp: false,
            },
        ];
        let out = render_language_icons(&icons, &cfg);
        assert!(out.text.contains("R"));
        assert!(out.text.contains("P"));
    }

    #[test]
    fn test_render_language_icons_dedup() {
        let cfg = StatuslineConfig::default().languages;
        let icons = vec![
            LangIcon {
                icon: "\u{e7a8}".into(),
                has_lsp: true,
            },
            LangIcon {
                icon: "\u{e7a8}".into(),
                has_lsp: false,
            },
        ];
        let out = render_language_icons(&icons, &cfg);
        // Should only show the icon once (first occurrence wins)
        assert_eq!(out.text.matches("\u{e7a8}").count(), 1);
    }
}
