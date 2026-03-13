//! Code-context configuration with stacked YAML config loading.
//!
//! Supports three layers of configuration with increasing precedence:
//! 1. **Builtin** — embedded at compile time from `builtin/code-context/config.yaml`
//! 2. **User** — `~/.code-context/config.yaml`
//! 3. **Project** — `./.code-context/config.yaml` (at git root)
//!
//! Stderr filter lists are additive across layers. Settings from later layers
//! override earlier ones.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use swissarmyhammer_directory::{CodeContextConfig as DirConfig, VirtualFileSystem};
use tracing::{debug, warn};

/// A single stderr filter pattern rule.
///
/// When an LSP server writes to stderr, lines matching any filter pattern
/// are suppressed from debug logs to reduce noise.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StderrFilterRule {
    /// Regex pattern to match against the stderr line.
    pub pattern: String,
    /// Human-readable explanation of why this pattern is filtered.
    pub reason: String,
}

/// Settings that control code-context behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeContextSettings {
    /// Log level for unfiltered stderr lines: "debug", "trace", or "off".
    #[serde(default = "default_stderr_log_level")]
    pub stderr_log_level: String,
}

/// Default stderr log level when not specified in config.
fn default_stderr_log_level() -> String {
    "debug".to_string()
}

impl Default for CodeContextSettings {
    fn default() -> Self {
        Self {
            stderr_log_level: default_stderr_log_level(),
        }
    }
}

/// Code-context configuration parsed from YAML.
///
/// Contains stderr filter patterns and settings. Multiple config files
/// are merged with additive filter lists and overriding settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CodeContextConfigYaml {
    /// Patterns that suppress matching LSP stderr lines from debug logs.
    #[serde(default)]
    pub stderr_filters: Vec<StderrFilterRule>,

    /// Behavior settings (log level, etc.).
    #[serde(default)]
    pub settings: CodeContextSettings,
}

/// The builtin config YAML, embedded at compile time.
pub const BUILTIN_CONFIG_YAML: &str = include_str!("../../builtin/code-context/config.yaml");

/// Parse a YAML string into a [`CodeContextConfigYaml`].
pub fn parse_code_context_config(
    yaml: &str,
) -> Result<CodeContextConfigYaml, serde_yaml_ng::Error> {
    serde_yaml_ng::from_str(yaml)
}

impl CodeContextConfigYaml {
    /// Merge another config into this one (additive for filters, override for settings).
    ///
    /// - `stderr_filters` are concatenated (other's rules appended).
    /// - `settings` from `other` override `self` (later layer wins).
    pub fn merge(&mut self, other: CodeContextConfigYaml) {
        self.stderr_filters.extend(other.stderr_filters);
        self.settings = other.settings;
    }
}

/// The logical name for the config file inside `.code-context/` directories.
const CONFIG_NAME: &str = "config";

/// Load and merge code-context config from all layers.
///
/// Uses [`VirtualFileSystem`] with dot-directory paths to discover config
/// files at `~/.code-context/` (user) and `{git_root}/.code-context/` (project),
/// then merges the full stack with the compile-time builtin via [`get_stack`].
///
/// Stderr filter lists are additive. Settings from later layers override.
/// Missing layers are silently skipped. This function never caches —
/// each call reads fresh from disk.
pub fn load_code_context_config() -> CodeContextConfigYaml {
    let mut vfs = VirtualFileSystem::<DirConfig>::new("code-context");
    vfs.add_builtin(CONFIG_NAME, BUILTIN_CONFIG_YAML);
    vfs.use_dot_directory_paths();

    if let Err(e) = vfs.load_all() {
        warn!(
            "Failed to load code-context config overlays: {}. Using builtin only.",
            e
        );
    }

    merge_config_stack(&vfs)
}

/// Load code-context config using explicit overlay directories (for testing).
///
/// Each path should be a directory containing YAML config files.
/// The builtin config is always loaded first regardless.
pub fn load_code_context_config_from_paths(overlay_paths: &[PathBuf]) -> CodeContextConfigYaml {
    use swissarmyhammer_directory::FileSource;

    let mut vfs = VirtualFileSystem::<DirConfig>::new("code-context");
    vfs.add_builtin(CONFIG_NAME, BUILTIN_CONFIG_YAML);

    for (i, path) in overlay_paths.iter().enumerate() {
        let source = if i == 0 && overlay_paths.len() > 1 {
            FileSource::User
        } else {
            FileSource::Local
        };
        if let Err(e) = vfs.load_files_from_dir(path, source) {
            warn!(
                "Failed to load code-context config from {}: {}",
                path.display(),
                e
            );
        }
    }

    merge_config_stack(&vfs)
}

/// Merge all versions of the config file from the VFS stack.
///
/// Iterates the stack in load order (builtin -> user -> project) and
/// merges each layer additively.
fn merge_config_stack(vfs: &VirtualFileSystem<DirConfig>) -> CodeContextConfigYaml {
    let stack = match vfs.get_stack(CONFIG_NAME) {
        Some(entries) => entries,
        None => {
            warn!("No code-context config found in VFS (not even builtin). Using defaults.");
            return parse_code_context_config(BUILTIN_CONFIG_YAML).unwrap_or_default();
        }
    };

    let mut config: Option<CodeContextConfigYaml> = None;

    for entry in stack {
        match parse_code_context_config(&entry.content) {
            Ok(layer) => {
                debug!(
                    "Merging code-context config layer from {} ({})",
                    entry.path.display(),
                    entry.source
                );
                match &mut config {
                    Some(existing) => existing.merge(layer),
                    None => config = Some(layer),
                }
            }
            Err(e) => {
                warn!(
                    "Failed to parse code-context config at {}: {}. Skipping layer.",
                    entry.path.display(),
                    e
                );
            }
        }
    }

    config.unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Compiled configuration with pre-compiled regex patterns
// ---------------------------------------------------------------------------

/// A single compiled stderr filter rule.
#[derive(Debug)]
pub struct CompiledStderrFilter {
    /// The compiled regex pattern.
    pub regex: Regex,
    /// Human-readable explanation of why this pattern is filtered.
    pub reason: String,
}

/// Error returned when a config pattern fails to compile as regex.
#[derive(Debug, thiserror::Error)]
#[error("Invalid regex pattern '{pattern}': {source}")]
pub struct PatternCompileError {
    /// The pattern string that failed to compile.
    pub pattern: String,
    /// The reason string from the config rule.
    pub reason: String,
    /// The underlying regex error.
    pub source: regex::Error,
}

/// Compiled form of [`CodeContextConfigYaml`] with pre-compiled regex patterns.
///
/// All regex patterns are compiled once at construction time, not per stderr line.
#[derive(Debug)]
pub struct CompiledCodeContextConfig {
    /// Compiled stderr filter rules.
    pub stderr_filters: Vec<CompiledStderrFilter>,
    /// Behavior settings.
    pub settings: CodeContextSettings,
}

impl CompiledCodeContextConfig {
    /// Compile a [`CodeContextConfigYaml`] into a [`CompiledCodeContextConfig`].
    ///
    /// Returns an error if any filter pattern is not valid regex.
    /// This ensures invalid patterns are caught at load time, not at filter time.
    pub fn compile(config: &CodeContextConfigYaml) -> Result<Self, PatternCompileError> {
        let filters = config
            .stderr_filters
            .iter()
            .map(|rule| {
                Regex::new(&rule.pattern)
                    .map(|regex| CompiledStderrFilter {
                        regex,
                        reason: rule.reason.clone(),
                    })
                    .map_err(|e| PatternCompileError {
                        pattern: rule.pattern.clone(),
                        reason: rule.reason.clone(),
                        source: e,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            stderr_filters: filters,
            settings: config.settings.clone(),
        })
    }
}

/// Check if an LSP stderr line should be filtered (suppressed).
///
/// Returns `true` if the line matches any filter pattern, meaning it should
/// not be logged. Returns `false` if the line should be logged normally.
pub fn should_filter_stderr(line: &str, config: &CompiledCodeContextConfig) -> bool {
    config.stderr_filters.iter().any(|f| f.regex.is_match(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_builtin_config() {
        let config = parse_code_context_config(BUILTIN_CONFIG_YAML)
            .expect("builtin config.yaml should parse successfully");

        // Should have the default stderr filter patterns
        assert!(
            config.stderr_filters.len() >= 7,
            "expected at least 7 stderr filter patterns, got {}",
            config.stderr_filters.len()
        );

        // Settings should have defaults
        assert_eq!(config.settings.stderr_log_level, "debug");
    }

    #[test]
    fn test_all_filter_patterns_are_valid_regex() {
        let config = parse_code_context_config(BUILTIN_CONFIG_YAML)
            .expect("builtin config.yaml should parse");

        for rule in &config.stderr_filters {
            regex::Regex::new(&rule.pattern).unwrap_or_else(|e| {
                panic!(
                    "stderr filter pattern '{}' is not valid regex: {}",
                    rule.pattern, e
                )
            });
        }
    }

    #[test]
    fn test_all_filter_patterns_have_reasons() {
        let config = parse_code_context_config(BUILTIN_CONFIG_YAML)
            .expect("builtin config.yaml should parse");

        for rule in &config.stderr_filters {
            assert!(
                !rule.reason.is_empty(),
                "stderr filter pattern '{}' has empty reason",
                rule.pattern
            );
        }
    }

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
stderr_filters: []
"#;
        let config = parse_code_context_config(yaml).expect("minimal config should parse");
        assert!(config.stderr_filters.is_empty());
        // Settings should use defaults
        assert_eq!(config.settings.stderr_log_level, "debug");
    }

    #[test]
    fn test_default_config_is_empty() {
        let config = CodeContextConfigYaml::default();
        assert!(config.stderr_filters.is_empty());
        assert_eq!(config.settings.stderr_log_level, "debug");
    }

    #[test]
    fn test_merge_filters_are_additive() {
        let mut base = parse_code_context_config(BUILTIN_CONFIG_YAML).unwrap();
        let base_filter_count = base.stderr_filters.len();

        let overlay = CodeContextConfigYaml {
            stderr_filters: vec![StderrFilterRule {
                pattern: r"custom noise".to_string(),
                reason: "Project-specific noise".to_string(),
            }],
            settings: CodeContextSettings::default(),
        };

        base.merge(overlay);
        assert_eq!(base.stderr_filters.len(), base_filter_count + 1);
        assert_eq!(base.stderr_filters.last().unwrap().pattern, "custom noise");
    }

    #[test]
    fn test_merge_settings_from_later_layer_wins() {
        let mut base = parse_code_context_config(BUILTIN_CONFIG_YAML).unwrap();
        assert_eq!(base.settings.stderr_log_level, "debug");

        let overlay = CodeContextConfigYaml {
            stderr_filters: vec![],
            settings: CodeContextSettings {
                stderr_log_level: "trace".to_string(),
            },
        };

        base.merge(overlay);
        assert_eq!(base.settings.stderr_log_level, "trace");
    }

    #[test]
    fn test_load_builtin_only() {
        // Pass empty overlay paths -- only builtin should load
        let config = load_code_context_config_from_paths(&[]);
        assert!(
            !config.stderr_filters.is_empty(),
            "should have builtin stderr filter patterns"
        );
    }

    #[test]
    fn test_load_with_overlay() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let overlay_dir = temp_dir.path().join("overlay");
        std::fs::create_dir_all(&overlay_dir).unwrap();

        let overlay_yaml = r#"
stderr_filters:
  - pattern: 'custom pattern'
    reason: "Project custom filter"
settings:
  stderr_log_level: "off"
"#;
        std::fs::write(overlay_dir.join("config.yaml"), overlay_yaml).unwrap();

        let config = load_code_context_config_from_paths(&[overlay_dir]);

        // Builtin filters + project filter
        assert!(config
            .stderr_filters
            .iter()
            .any(|r| r.pattern == "custom pattern"));
        // Project settings override
        assert_eq!(config.settings.stderr_log_level, "off");
    }

    #[test]
    fn test_load_missing_dirs_skipped() {
        let config = load_code_context_config_from_paths(&[
            PathBuf::from("/nonexistent/overlay"),
            PathBuf::from("/also/missing"),
        ]);
        // Should still have builtin patterns
        assert!(!config.stderr_filters.is_empty());
    }

    #[test]
    fn test_load_malformed_overlay_skipped() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let overlay_dir = temp_dir.path().join("bad");
        std::fs::create_dir_all(&overlay_dir).unwrap();
        std::fs::write(
            overlay_dir.join("config.yaml"),
            "this: is: not: valid: yaml: [[[",
        )
        .unwrap();

        let config = load_code_context_config_from_paths(&[overlay_dir]);
        // Should still have builtin patterns (malformed overlay skipped)
        assert!(!config.stderr_filters.is_empty());
    }

    #[test]
    fn test_should_filter_matches() {
        let config = CodeContextConfigYaml {
            stderr_filters: vec![
                StderrFilterRule {
                    pattern: r"inference diagnostic".to_string(),
                    reason: "noise".to_string(),
                },
                StderrFilterRule {
                    pattern: r"^\s*$".to_string(),
                    reason: "empty lines".to_string(),
                },
            ],
            settings: CodeContextSettings::default(),
        };
        let compiled = CompiledCodeContextConfig::compile(&config).unwrap();

        assert!(should_filter_stderr(
            "some inference diagnostic here",
            &compiled
        ));
        assert!(should_filter_stderr("", &compiled));
        assert!(should_filter_stderr("   ", &compiled));
    }

    #[test]
    fn test_should_filter_no_match() {
        let config = CodeContextConfigYaml {
            stderr_filters: vec![StderrFilterRule {
                pattern: r"inference diagnostic".to_string(),
                reason: "noise".to_string(),
            }],
            settings: CodeContextSettings::default(),
        };
        let compiled = CompiledCodeContextConfig::compile(&config).unwrap();

        assert!(!should_filter_stderr(
            "Actual error: file not found",
            &compiled
        ));
        assert!(!should_filter_stderr(
            "WARNING: something important",
            &compiled
        ));
    }

    #[test]
    fn test_compile_builtin_succeeds() {
        let config = parse_code_context_config(BUILTIN_CONFIG_YAML).unwrap();
        let compiled = CompiledCodeContextConfig::compile(&config);
        assert!(compiled.is_ok());
        assert!(!compiled.unwrap().stderr_filters.is_empty());
    }

    #[test]
    fn test_invalid_regex_compile_error() {
        let config = CodeContextConfigYaml {
            stderr_filters: vec![StderrFilterRule {
                pattern: "[invalid(regex".to_string(),
                reason: "Bad pattern".to_string(),
            }],
            settings: CodeContextSettings::default(),
        };
        let result = CompiledCodeContextConfig::compile(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.pattern, "[invalid(regex");
    }
}
