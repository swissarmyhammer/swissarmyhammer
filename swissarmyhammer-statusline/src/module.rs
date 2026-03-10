//! Module framework: ModuleOutput, registry, and evaluation.

use std::collections::HashMap;

use crate::config::StatuslineConfig;
use crate::format::{parse_format, FormatSegment};
use crate::input::StatuslineInput;
use crate::style::Style;

/// The output of a module evaluation.
#[derive(Debug, Clone)]
pub struct ModuleOutput {
    /// The rendered text (with format string interpolated).
    pub text: String,
    /// The style to apply.
    pub style: Style,
}

impl ModuleOutput {
    /// Create a new module output with text and style.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// Create an empty (hidden) output.
    pub fn hidden() -> Self {
        Self {
            text: String::new(),
            style: Style::default(),
        }
    }

    /// Returns true if this output has no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Apply the style to the text and return the rendered string.
    pub fn render(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        self.style.apply(&self.text)
    }
}

/// Context passed to module evaluation functions.
pub struct ModuleContext<'a> {
    pub input: &'a StatuslineInput,
    pub config: &'a StatuslineConfig,
}

/// A module evaluation function.
pub type ModuleFn = fn(&ModuleContext) -> ModuleOutput;

/// Registry of all available modules.
pub struct ModuleRegistry {
    modules: HashMap<String, ModuleFn>,
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRegistry {
    /// Create a new registry with all built-in modules registered.
    pub fn new() -> Self {
        use crate::modules;
        let mut modules = HashMap::new();

        // Claude modules (stdin JSON data)
        modules.insert("directory".into(), modules::directory::eval as ModuleFn);
        modules.insert("model".into(), modules::model::eval as ModuleFn);
        modules.insert(
            "context_bar".into(),
            modules::context_bar::eval as ModuleFn,
        );
        modules.insert("cost".into(), modules::cost::eval as ModuleFn);
        modules.insert("session".into(), modules::session::eval as ModuleFn);
        modules.insert("vim_mode".into(), modules::vim_mode::eval as ModuleFn);
        modules.insert("agent".into(), modules::agent::eval as ModuleFn);
        modules.insert("worktree".into(), modules::worktree::eval as ModuleFn);
        modules.insert("version".into(), modules::version::eval as ModuleFn);

        // Tool modules (sah libraries)
        modules.insert("git_branch".into(), modules::git_branch::eval as ModuleFn);
        modules.insert("git_status".into(), modules::git_status::eval as ModuleFn);
        modules.insert("git_state".into(), modules::git_state::eval as ModuleFn);
        modules.insert("kanban".into(), modules::kanban::eval as ModuleFn);
        modules.insert("index".into(), modules::index::eval as ModuleFn);
        modules.insert("languages".into(), modules::languages::eval as ModuleFn);

        Self { modules }
    }

    /// Look up a module by name.
    pub fn get(&self, name: &str) -> Option<&ModuleFn> {
        self.modules.get(name)
    }
}

/// Interpolate a module-level format string with variables.
///
/// Variables are module-specific (e.g., `$branch`, `$bar`).
/// Unknown variables are silently omitted.
pub fn interpolate(format: &str, vars: &HashMap<String, String>) -> String {
    let segments = parse_format(format);
    let mut out = String::new();
    for seg in segments {
        match seg {
            FormatSegment::Literal(s) => out.push_str(&s),
            FormatSegment::Variable(name) => {
                if let Some(val) = vars.get(&name) {
                    out.push_str(val);
                }
            }
        }
    }
    out
}
