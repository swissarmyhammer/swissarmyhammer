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
        modules.insert("context_bar".into(), modules::context_bar::eval as ModuleFn);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_output_new() {
        let out = ModuleOutput::new("hello", Style::parse("green"));
        assert_eq!(out.text, "hello");
        assert!(!out.is_empty());
    }

    #[test]
    fn test_module_output_hidden() {
        let out = ModuleOutput::hidden();
        assert!(out.is_empty());
        assert_eq!(out.render(), "");
    }

    #[test]
    fn test_module_output_render_with_style() {
        let out = ModuleOutput::new("hello", Style::parse("green"));
        let rendered = out.render();
        assert!(rendered.contains("hello"));
        assert!(rendered.contains("\x1b[32m"));
    }

    #[test]
    fn test_module_output_render_empty() {
        let out = ModuleOutput::new("", Style::parse("green"));
        assert_eq!(out.render(), "");
    }

    #[test]
    fn test_module_registry_new() {
        let reg = ModuleRegistry::new();
        assert!(reg.get("directory").is_some());
        assert!(reg.get("model").is_some());
        assert!(reg.get("context_bar").is_some());
        assert!(reg.get("cost").is_some());
        assert!(reg.get("session").is_some());
        assert!(reg.get("vim_mode").is_some());
        assert!(reg.get("agent").is_some());
        assert!(reg.get("worktree").is_some());
        assert!(reg.get("version").is_some());
        assert!(reg.get("git_branch").is_some());
        assert!(reg.get("git_status").is_some());
        assert!(reg.get("git_state").is_some());
        assert!(reg.get("kanban").is_some());
        assert!(reg.get("index").is_some());
        assert!(reg.get("languages").is_some());
    }

    #[test]
    fn test_module_registry_default() {
        let reg = ModuleRegistry::default();
        assert!(reg.get("directory").is_some());
    }

    #[test]
    fn test_module_registry_get_nonexistent() {
        let reg = ModuleRegistry::new();
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_interpolate_basic() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "world".into());
        assert_eq!(interpolate("hello $name!", &vars), "hello world!");
    }

    #[test]
    fn test_interpolate_missing_var() {
        let vars = HashMap::new();
        assert_eq!(interpolate("hello $name", &vars), "hello ");
    }

    #[test]
    fn test_interpolate_no_vars() {
        let vars = HashMap::new();
        assert_eq!(interpolate("literal text", &vars), "literal text");
    }

    #[test]
    fn test_interpolate_multiple_vars() {
        let mut vars = HashMap::new();
        vars.insert("a".into(), "1".into());
        vars.insert("b".into(), "2".into());
        assert_eq!(interpolate("$a+$b", &vars), "1+2");
    }

    #[test]
    fn test_module_output_hidden_render_is_empty() {
        let out = ModuleOutput::hidden();
        assert_eq!(out.render(), "");
        assert!(out.is_empty());
    }

    #[test]
    fn test_module_output_new_with_default_style() {
        let out = ModuleOutput::new("text", Style::default());
        assert_eq!(out.text, "text");
        assert!(!out.is_empty());
        // Default style has no codes, so render returns raw text
        assert_eq!(out.render(), "text");
    }

    #[test]
    fn test_module_output_render_non_empty_with_style() {
        let out = ModuleOutput::new("styled", Style::parse("bold red"));
        let rendered = out.render();
        assert!(rendered.contains("styled"));
        assert!(rendered.contains("\x1b[1m"));
        assert!(rendered.contains("\x1b[31m"));
        assert!(rendered.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_module_registry_has_all_modules() {
        let reg = ModuleRegistry::new();
        let expected = [
            "directory",
            "model",
            "context_bar",
            "cost",
            "session",
            "vim_mode",
            "agent",
            "worktree",
            "version",
            "git_branch",
            "git_status",
            "git_state",
            "kanban",
            "index",
            "languages",
        ];
        for name in &expected {
            assert!(reg.get(name).is_some(), "missing module: {}", name);
        }
    }

    #[test]
    fn test_module_registry_eval_functions_callable() {
        let reg = ModuleRegistry::new();
        let input = crate::input::StatuslineInput::default();
        let config = crate::config::StatuslineConfig::default();
        let ctx = ModuleContext {
            input: &input,
            config: &config,
        };
        // Call each module's eval function to ensure they don't panic
        for name in [
            "directory",
            "model",
            "context_bar",
            "cost",
            "session",
            "vim_mode",
            "agent",
            "worktree",
            "version",
        ] {
            let eval_fn = reg.get(name).unwrap();
            let out = eval_fn(&ctx);
            // Just ensure it doesn't panic; output may be empty or not
            let _ = out.render();
        }
    }

    #[test]
    fn test_interpolate_empty_format() {
        let vars = HashMap::new();
        assert_eq!(interpolate("", &vars), "");
    }

    #[test]
    fn test_interpolate_only_variable() {
        let mut vars = HashMap::new();
        vars.insert("x".into(), "value".into());
        assert_eq!(interpolate("$x", &vars), "value");
    }

    #[test]
    fn test_interpolate_adjacent_vars() {
        let mut vars = HashMap::new();
        vars.insert("a".into(), "X".into());
        vars.insert("b".into(), "Y".into());
        assert_eq!(interpolate("$a$b", &vars), "XY");
    }

    #[test]
    fn test_interpolate_with_brackets() {
        let mut vars = HashMap::new();
        vars.insert("bar".into(), "###".into());
        assert_eq!(interpolate("[$bar]", &vars), "[###]");
    }

    #[test]
    fn test_module_output_clone() {
        let out = ModuleOutput::new("hello", Style::parse("green"));
        let cloned = out.clone();
        assert_eq!(cloned.text, out.text);
        assert_eq!(cloned.render(), out.render());
    }

    #[test]
    fn test_module_output_debug() {
        let out = ModuleOutput::new("test", Style::parse("red"));
        let debug = format!("{:?}", out);
        assert!(debug.contains("test"));
    }
}
