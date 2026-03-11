//! Languages module - shows language icons for detected project types.

use crate::module::{ModuleContext, ModuleOutput};
use crate::style::Style;
use swissarmyhammer_project_detection::ProjectType;

/// A project type with its display icon and known LSP servers.
struct LanguageIcon {
    project_type: ProjectType,
    icon: &'static str,
    lsp_servers: &'static [&'static str],
}

const LANGUAGE_ICONS: &[LanguageIcon] = &[
    LanguageIcon {
        project_type: ProjectType::Rust,
        icon: "\u{1f980}",
        lsp_servers: &["rust-analyzer"],
    },
    LanguageIcon {
        project_type: ProjectType::Python,
        icon: "\u{1f40d}",
        lsp_servers: &["pyright", "pylsp"],
    },
    LanguageIcon {
        project_type: ProjectType::NodeJs,
        icon: "\u{1f4dc}",
        lsp_servers: &["typescript-language-server"],
    },
    LanguageIcon {
        project_type: ProjectType::Go,
        icon: "\u{1f439}",
        lsp_servers: &["gopls"],
    },
    LanguageIcon {
        project_type: ProjectType::JavaMaven,
        icon: "\u{2615}",
        lsp_servers: &["jdtls"],
    },
    LanguageIcon {
        project_type: ProjectType::JavaGradle,
        icon: "\u{2615}",
        lsp_servers: &["jdtls"],
    },
    LanguageIcon {
        project_type: ProjectType::CSharp,
        icon: "\u{1f4bb}",
        lsp_servers: &["omnisharp"],
    },
    LanguageIcon {
        project_type: ProjectType::CMake,
        icon: "\u{2699}\u{fe0f}",
        lsp_servers: &["clangd"],
    },
    LanguageIcon {
        project_type: ProjectType::Makefile,
        icon: "\u{2699}\u{fe0f}",
        lsp_servers: &["clangd"],
    },
    LanguageIcon {
        project_type: ProjectType::Flutter,
        icon: "\u{1f426}",
        lsp_servers: &["dart"],
    },
];

/// Evaluate the languages module.
///
/// Detects project types in the current directory and shows icons for each.
/// Icons are dimmed when the corresponding LSP server is not found in PATH.
pub fn eval(ctx: &ModuleContext) -> ModuleOutput {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return ModuleOutput::hidden(),
    };

    let projects = match swissarmyhammer_project_detection::detect_projects(&cwd, Some(1)) {
        Ok(p) => p,
        Err(_) => return ModuleOutput::hidden(),
    };

    if projects.is_empty() {
        return ModuleOutput::hidden();
    }

    let cfg = &ctx.config.languages;
    let mut icons = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for lang_icon in LANGUAGE_ICONS {
        let has_project = projects
            .iter()
            .any(|p| p.project_type == lang_icon.project_type);
        if !has_project {
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
