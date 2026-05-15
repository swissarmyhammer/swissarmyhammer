//! Language detection and parser registry for tree-sitter
//!
//! This module provides language detection from file extensions and
//! maintains a registry of all supported tree-sitter language parsers.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Language;

/// Configuration for a supported language
#[derive(Debug, Clone)]
pub struct LanguageConfig {
    /// Language name (e.g., "rust", "python")
    pub name: &'static str,
    /// Tree-sitter language function
    pub language_fn: fn() -> Language,
    /// File extensions for this language (without dot)
    pub extensions: &'static [&'static str],
}

impl LanguageConfig {
    /// Get the tree-sitter Language
    pub fn language(&self) -> Language {
        (self.language_fn)()
    }
}

// Helper functions that return Language for each parser
// These handle the different APIs across tree-sitter packages

fn lang_rust() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn lang_python() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

fn lang_typescript() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn lang_tsx() -> Language {
    tree_sitter_typescript::LANGUAGE_TSX.into()
}

fn lang_javascript() -> Language {
    tree_sitter_javascript::LANGUAGE.into()
}

fn lang_dart() -> Language {
    tree_sitter_dart::language()
}

fn lang_go() -> Language {
    tree_sitter_go::LANGUAGE.into()
}

fn lang_java() -> Language {
    tree_sitter_java::LANGUAGE.into()
}

fn lang_c() -> Language {
    tree_sitter_c::LANGUAGE.into()
}

fn lang_cpp() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}

fn lang_c_sharp() -> Language {
    tree_sitter_c_sharp::LANGUAGE.into()
}

fn lang_ruby() -> Language {
    tree_sitter_ruby::LANGUAGE.into()
}

fn lang_php() -> Language {
    tree_sitter_php::LANGUAGE_PHP.into()
}

fn lang_swift() -> Language {
    tree_sitter_swift::LANGUAGE.into()
}

fn lang_kotlin() -> Language {
    tree_sitter_kotlin_ng::LANGUAGE.into()
}

fn lang_scala() -> Language {
    tree_sitter_scala::LANGUAGE.into()
}

fn lang_lua() -> Language {
    tree_sitter_lua::LANGUAGE.into()
}

fn lang_elixir() -> Language {
    tree_sitter_elixir::LANGUAGE.into()
}

fn lang_haskell() -> Language {
    tree_sitter_haskell::LANGUAGE.into()
}

fn lang_ocaml() -> Language {
    tree_sitter_ocaml::LANGUAGE_OCAML.into()
}

fn lang_ocaml_interface() -> Language {
    tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into()
}

fn lang_zig() -> Language {
    tree_sitter_zig::LANGUAGE.into()
}

fn lang_bash() -> Language {
    tree_sitter_bash::LANGUAGE.into()
}

fn lang_html() -> Language {
    tree_sitter_html::LANGUAGE.into()
}

fn lang_css() -> Language {
    tree_sitter_css::LANGUAGE.into()
}

fn lang_json() -> Language {
    tree_sitter_json::LANGUAGE.into()
}

fn lang_yaml() -> Language {
    tree_sitter_yaml::LANGUAGE.into()
}

fn lang_toml() -> Language {
    tree_sitter_toml_ng::LANGUAGE.into()
}

fn lang_markdown() -> Language {
    tree_sitter_md::LANGUAGE.into()
}

fn lang_sql() -> Language {
    tree_sitter_sequel::LANGUAGE.into()
}

/// All supported language configurations
static LANGUAGES: &[LanguageConfig] = &[
    LanguageConfig {
        name: "rust",
        language_fn: lang_rust,
        extensions: &["rs"],
    },
    LanguageConfig {
        name: "python",
        language_fn: lang_python,
        extensions: &["py", "pyi", "pyw"],
    },
    LanguageConfig {
        name: "typescript",
        language_fn: lang_typescript,
        extensions: &["ts", "mts", "cts"],
    },
    LanguageConfig {
        name: "tsx",
        language_fn: lang_tsx,
        extensions: &["tsx"],
    },
    LanguageConfig {
        name: "javascript",
        language_fn: lang_javascript,
        extensions: &["js", "mjs", "cjs", "jsx"],
    },
    LanguageConfig {
        name: "dart",
        language_fn: lang_dart,
        extensions: &["dart"],
    },
    LanguageConfig {
        name: "go",
        language_fn: lang_go,
        extensions: &["go"],
    },
    LanguageConfig {
        name: "java",
        language_fn: lang_java,
        extensions: &["java"],
    },
    LanguageConfig {
        name: "c",
        language_fn: lang_c,
        extensions: &["c", "h"],
    },
    LanguageConfig {
        name: "cpp",
        language_fn: lang_cpp,
        extensions: &["cpp", "cc", "cxx", "hpp", "hxx", "hh"],
    },
    LanguageConfig {
        name: "c_sharp",
        language_fn: lang_c_sharp,
        extensions: &["cs"],
    },
    LanguageConfig {
        name: "ruby",
        language_fn: lang_ruby,
        extensions: &["rb", "rake", "gemspec"],
    },
    LanguageConfig {
        name: "php",
        language_fn: lang_php,
        extensions: &["php", "phtml", "php3", "php4", "php5", "phps"],
    },
    LanguageConfig {
        name: "swift",
        language_fn: lang_swift,
        extensions: &["swift"],
    },
    LanguageConfig {
        name: "kotlin",
        language_fn: lang_kotlin,
        extensions: &["kt", "kts"],
    },
    LanguageConfig {
        name: "scala",
        language_fn: lang_scala,
        extensions: &["scala", "sc"],
    },
    LanguageConfig {
        name: "lua",
        language_fn: lang_lua,
        extensions: &["lua"],
    },
    LanguageConfig {
        name: "elixir",
        language_fn: lang_elixir,
        extensions: &["ex", "exs"],
    },
    LanguageConfig {
        name: "haskell",
        language_fn: lang_haskell,
        extensions: &["hs", "lhs"],
    },
    LanguageConfig {
        name: "ocaml",
        language_fn: lang_ocaml,
        extensions: &["ml"],
    },
    LanguageConfig {
        name: "ocaml_interface",
        language_fn: lang_ocaml_interface,
        extensions: &["mli"],
    },
    LanguageConfig {
        name: "zig",
        language_fn: lang_zig,
        extensions: &["zig"],
    },
    LanguageConfig {
        name: "bash",
        language_fn: lang_bash,
        extensions: &["sh", "bash", "zsh"],
    },
    LanguageConfig {
        name: "html",
        language_fn: lang_html,
        extensions: &["html", "htm", "xhtml"],
    },
    LanguageConfig {
        name: "css",
        language_fn: lang_css,
        extensions: &["css"],
    },
    LanguageConfig {
        name: "json",
        language_fn: lang_json,
        extensions: &["json", "jsonc"],
    },
    LanguageConfig {
        name: "yaml",
        language_fn: lang_yaml,
        extensions: &["yaml", "yml"],
    },
    LanguageConfig {
        name: "toml",
        language_fn: lang_toml,
        extensions: &["toml"],
    },
    LanguageConfig {
        name: "markdown",
        language_fn: lang_markdown,
        extensions: &["md", "markdown", "mdx"],
    },
    LanguageConfig {
        name: "sql",
        language_fn: lang_sql,
        extensions: &["sql"],
    },
];

/// Registry of all supported languages
pub struct LanguageRegistry {
    /// Map from file extension to language config
    by_extension: HashMap<&'static str, &'static LanguageConfig>,
    /// Map from language name to language config
    by_name: HashMap<&'static str, &'static LanguageConfig>,
}

impl LanguageRegistry {
    /// Create a new language registry
    fn new() -> Self {
        let mut by_extension = HashMap::new();
        let mut by_name = HashMap::new();

        for config in LANGUAGES {
            by_name.insert(config.name, config);
            for ext in config.extensions {
                by_extension.insert(*ext, config);
            }
        }

        Self {
            by_extension,
            by_name,
        }
    }

    /// Get the global language registry instance
    pub fn global() -> &'static Self {
        static INSTANCE: Lazy<LanguageRegistry> = Lazy::new(LanguageRegistry::new);
        &INSTANCE
    }

    /// Detect language from file path based on extension
    pub fn detect_language(&self, path: &Path) -> Option<&'static LanguageConfig> {
        let extension = path.extension()?.to_str()?;
        self.by_extension.get(extension).copied()
    }

    /// Get language config by name
    pub fn get_by_name(&self, name: &str) -> Option<&'static LanguageConfig> {
        self.by_name.get(name).copied()
    }

    /// Get all supported file extensions
    pub fn all_extensions(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_extension.keys().copied()
    }

    /// Check if an extension is supported
    pub fn is_supported(&self, extension: &str) -> bool {
        self.by_extension.contains_key(extension)
    }

    /// Get all supported language names
    pub fn all_languages(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_name.keys().copied()
    }

    /// Get the number of supported languages
    pub fn language_count(&self) -> usize {
        self.by_name.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_rust() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("src/main.rs"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "rust");
    }

    #[test]
    fn test_detect_typescript() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("src/app.ts"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "typescript");
    }

    #[test]
    fn test_detect_tsx() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("src/App.tsx"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "tsx");
    }

    #[test]
    fn test_detect_markdown() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("README.md"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "markdown");
    }

    #[test]
    fn test_detect_yaml() {
        let registry = LanguageRegistry::global();

        let config = registry.detect_language(Path::new("config.yaml"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "yaml");

        let config = registry.detect_language(Path::new("config.yml"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "yaml");
    }

    #[test]
    fn test_detect_json() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("package.json"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "json");
    }

    #[test]
    fn test_detect_toml() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("Cargo.toml"));
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "toml");
    }

    #[test]
    fn test_unsupported_extension() {
        let registry = LanguageRegistry::global();
        let config = registry.detect_language(Path::new("file.xyz"));
        assert!(config.is_none());
    }

    #[test]
    fn test_get_by_name() {
        let registry = LanguageRegistry::global();

        let config = registry.get_by_name("rust");
        assert!(config.is_some());
        assert_eq!(config.unwrap().name, "rust");

        let config = registry.get_by_name("nonexistent");
        assert!(config.is_none());
    }

    #[test]
    fn test_all_extensions() {
        let registry = LanguageRegistry::global();
        let extensions: Vec<_> = registry.all_extensions().collect();

        assert!(extensions.contains(&"rs"));
        assert!(extensions.contains(&"py"));
        assert!(extensions.contains(&"ts"));
        assert!(extensions.contains(&"md"));
        assert!(extensions.contains(&"json"));
        assert!(extensions.contains(&"yaml"));
        assert!(extensions.contains(&"toml"));
    }

    #[test]
    fn test_language_count() {
        let registry = LanguageRegistry::global();
        // We have 31 language configs (including tsx, ocaml_interface separately)
        assert!(registry.language_count() >= 25);
    }

    #[test]
    fn test_language_can_create_parser() {
        let registry = LanguageRegistry::global();

        // Test that we can actually create parsers for key languages
        for name in [
            "rust",
            "python",
            "typescript",
            "markdown",
            "json",
            "yaml",
            "toml",
        ] {
            let config = registry
                .get_by_name(name)
                .unwrap_or_else(|| panic!("Should have {}", name));
            let language = config.language();

            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&language)
                .unwrap_or_else(|_| panic!("Should set {} language", name));
        }
    }

    #[test]
    fn test_is_supported() {
        let registry = LanguageRegistry::global();

        // Test supported extensions
        assert!(registry.is_supported("rs"));
        assert!(registry.is_supported("py"));
        assert!(registry.is_supported("ts"));
        assert!(registry.is_supported("md"));
        assert!(registry.is_supported("json"));
        assert!(registry.is_supported("yaml"));
        assert!(registry.is_supported("toml"));

        // Test unsupported extensions
        assert!(!registry.is_supported("xyz"));
        assert!(!registry.is_supported("unknown"));
        assert!(!registry.is_supported(""));
    }

    #[test]
    fn test_all_languages() {
        let registry = LanguageRegistry::global();
        let languages: Vec<_> = registry.all_languages().collect();

        // Check that key languages are present
        assert!(languages.contains(&"rust"));
        assert!(languages.contains(&"python"));
        assert!(languages.contains(&"typescript"));
        assert!(languages.contains(&"markdown"));
        assert!(languages.contains(&"json"));
        assert!(languages.contains(&"yaml"));
        assert!(languages.contains(&"toml"));

        // Verify count matches
        assert_eq!(languages.len(), registry.language_count());
    }

    #[test]
    fn test_language_config_fields() {
        let registry = LanguageRegistry::global();
        let config = registry.get_by_name("rust").unwrap();

        // Test public fields are accessible
        assert_eq!(config.name, "rust");
        assert!(config.extensions.contains(&"rs"));

        // Test language_fn can be called directly
        let lang = (config.language_fn)();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&lang).is_ok());
    }

    #[test]
    fn test_language_config_language_method() {
        let registry = LanguageRegistry::global();

        // Test the language() method for multiple languages
        for name in ["rust", "python", "javascript", "go", "java", "c", "cpp"] {
            let config = registry.get_by_name(name).unwrap();
            let language = config.language();

            // Verify we can use the language to create a parser
            let mut parser = tree_sitter::Parser::new();
            assert!(
                parser.set_language(&language).is_ok(),
                "Failed to set language for {}",
                name
            );
        }
    }

    #[test]
    fn test_helper_functions() {
        // Test that all helper functions work correctly
        let _rust = lang_rust();
        let _python = lang_python();
        let _typescript = lang_typescript();
        let _tsx = lang_tsx();
        let _javascript = lang_javascript();
        let _dart = lang_dart();
        let _go = lang_go();
        let _java = lang_java();
        let _c = lang_c();
        let _cpp = lang_cpp();
        let _c_sharp = lang_c_sharp();
        let _ruby = lang_ruby();
        let _php = lang_php();
        let _swift = lang_swift();
        let _kotlin = lang_kotlin();
        let _scala = lang_scala();
        let _lua = lang_lua();
        let _elixir = lang_elixir();
        let _haskell = lang_haskell();
        let _ocaml = lang_ocaml();
        let _ocaml_interface = lang_ocaml_interface();
        let _zig = lang_zig();
        let _bash = lang_bash();
        let _html = lang_html();
        let _css = lang_css();
        let _json = lang_json();
        let _yaml = lang_yaml();
        let _toml = lang_toml();
        let _markdown = lang_markdown();
        let _sql = lang_sql();
    }
}
