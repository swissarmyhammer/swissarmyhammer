//! Shell security configuration with stacked YAML config loading.
//!
//! Supports three layers of configuration with increasing precedence:
//! 1. **Builtin** — embedded at compile time from `builtin/shell/config.yaml`
//! 2. **User** — `~/.shell/config.yaml`
//! 3. **Project** — `./.shell/config.yaml` (at git root)
//!
//! Deny/permit lists are additive across layers. Settings from later layers
//! override earlier ones.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use swissarmyhammer_directory::{ShellConfig, VirtualFileSystem};
use tracing::{debug, warn};

use crate::security::ShellSecurityError;

/// A single permit or deny pattern rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternRule {
    /// Regex pattern to match against the command string.
    pub pattern: String,
    /// Human-readable explanation of why this pattern exists.
    pub reason: String,
}

/// Settings that control shell security validation behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellSettings {
    /// Maximum allowed command length in characters.
    #[serde(default = "default_max_command_length")]
    pub max_command_length: usize,

    /// Maximum allowed environment variable value length in characters.
    #[serde(default = "default_max_env_value_length")]
    pub max_env_value_length: usize,

    /// Enable audit logging of all command executions.
    #[serde(default = "default_enable_audit_logging")]
    pub enable_audit_logging: bool,
}

fn default_max_command_length() -> usize {
    4096
}

fn default_max_env_value_length() -> usize {
    1024
}

fn default_enable_audit_logging() -> bool {
    true
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            max_command_length: default_max_command_length(),
            max_env_value_length: default_max_env_value_length(),
            enable_audit_logging: default_enable_audit_logging(),
        }
    }
}

/// Shell security configuration parsed from YAML.
///
/// Contains permit patterns (checked first, short-circuit allow),
/// deny patterns (checked second, block if matched), and settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ShellSecurityConfig {
    /// Patterns that explicitly allow commands. Evaluated before deny patterns.
    /// A permit match short-circuits — the command is allowed even if a deny
    /// pattern would also match.
    #[serde(default)]
    pub permit: Vec<PatternRule>,

    /// Patterns that block commands. Evaluated after permit patterns.
    #[serde(default)]
    pub deny: Vec<PatternRule>,

    /// Validation settings (command length limits, audit logging, etc.).
    #[serde(default)]
    pub settings: ShellSettings,
}

/// The builtin config YAML, embedded at compile time.
pub const BUILTIN_CONFIG_YAML: &str = include_str!("../../builtin/shell/config.yaml");

/// Parse a YAML string into a [`ShellSecurityConfig`].
pub fn parse_shell_config(yaml: &str) -> Result<ShellSecurityConfig, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

impl ShellSecurityConfig {
    /// Merge another config into this one (additive).
    ///
    /// - `permit` and `deny` lists are concatenated (other's rules appended).
    /// - `settings` from `other` override `self` field-by-field only when the
    ///   other config explicitly provides them. Since we can't distinguish
    ///   "explicitly set to default" from "not set" with serde defaults,
    ///   the later layer always wins for settings.
    pub fn merge(&mut self, other: ShellSecurityConfig) {
        self.permit.extend(other.permit);
        self.deny.extend(other.deny);
        self.settings = other.settings;
    }
}

/// The logical name for the config file inside `.shell/` directories.
const CONFIG_NAME: &str = "config";

/// Load and merge shell security config from all layers.
///
/// Uses [`VirtualFileSystem`] with dot-directory paths to discover config
/// files at `~/.shell/` (user) and `{git_root}/.shell/` (project), then
/// merges the full stack with the compile-time builtin via [`get_stack`].
///
/// Deny/permit lists are additive. Settings from later layers override.
/// Missing layers are silently skipped. This function never caches —
/// each call reads fresh from disk.
pub fn load_shell_config() -> ShellSecurityConfig {
    let mut vfs = VirtualFileSystem::<ShellConfig>::new("shell");
    vfs.add_builtin(CONFIG_NAME, BUILTIN_CONFIG_YAML);
    vfs.use_dot_directory_paths();

    if let Err(e) = vfs.load_all() {
        warn!(
            "Failed to load shell config overlays: {}. Using builtin only.",
            e
        );
    }

    merge_config_stack(&vfs)
}

/// Load shell config using explicit overlay directories (for testing).
///
/// Each path should be a directory containing YAML config files.
/// The builtin config is always loaded first regardless.
pub fn load_shell_config_from_paths(overlay_paths: &[PathBuf]) -> ShellSecurityConfig {
    use swissarmyhammer_directory::FileSource;

    let mut vfs = VirtualFileSystem::<ShellConfig>::new("shell");
    vfs.add_builtin(CONFIG_NAME, BUILTIN_CONFIG_YAML);

    for (i, path) in overlay_paths.iter().enumerate() {
        let source = if i == 0 && overlay_paths.len() > 1 {
            FileSource::User
        } else {
            FileSource::Local
        };
        if let Err(e) = vfs.load_files_from_dir(path, source) {
            warn!("Failed to load shell config from {}: {}", path.display(), e);
        }
    }

    merge_config_stack(&vfs)
}

/// Merge all versions of the config file from the VFS stack.
///
/// Iterates the stack in load order (builtin → user → project) and
/// merges each layer additively.
fn merge_config_stack(vfs: &VirtualFileSystem<ShellConfig>) -> ShellSecurityConfig {
    let stack = match vfs.get_stack(CONFIG_NAME) {
        Some(entries) => entries,
        None => {
            warn!("No shell config found in VFS (not even builtin). Using defaults.");
            return parse_shell_config(BUILTIN_CONFIG_YAML).unwrap_or_default();
        }
    };

    let mut config: Option<ShellSecurityConfig> = None;

    for entry in stack {
        match parse_shell_config(&entry.content) {
            Ok(layer) => {
                debug!(
                    "Merging shell config layer from {} ({})",
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
                    "Failed to parse shell config at {}: {}. Skipping layer.",
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

/// A single compiled permit or deny rule.
#[derive(Debug)]
pub struct CompiledRule {
    /// The compiled regex pattern.
    pub regex: Regex,
    /// Human-readable explanation of why this rule exists.
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

/// Compiled form of [`ShellSecurityConfig`] with pre-compiled regex patterns.
///
/// All regex patterns are compiled once at construction time, not per command evaluation.
#[derive(Debug)]
pub struct CompiledShellConfig {
    /// Compiled permit rules (checked first, short-circuit allow).
    pub permit: Vec<CompiledRule>,
    /// Compiled deny rules (checked second, block if matched).
    pub deny: Vec<CompiledRule>,
    /// Validation settings.
    pub settings: ShellSettings,
}

impl CompiledShellConfig {
    /// Compile a [`ShellSecurityConfig`] into a [`CompiledShellConfig`].
    ///
    /// Returns an error if any permit or deny pattern is not valid regex.
    /// This ensures invalid patterns are caught at load time, not at validation time.
    pub fn compile(config: &ShellSecurityConfig) -> Result<Self, PatternCompileError> {
        let permit = config
            .permit
            .iter()
            .map(|rule| {
                Regex::new(&rule.pattern)
                    .map(|regex| CompiledRule {
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

        let deny = config
            .deny
            .iter()
            .map(|rule| {
                Regex::new(&rule.pattern)
                    .map(|regex| CompiledRule {
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
            permit,
            deny,
            settings: config.settings.clone(),
        })
    }
}

/// Evaluate a command against compiled permit/deny rules.
///
/// **Evaluation order:**
/// 1. Check permit patterns — if any match, the command is allowed immediately.
/// 2. Check deny patterns — if any match, return a `BlockedCommandPattern` error
///    with the reason from the matching rule.
/// 3. If no deny pattern matches, the command is allowed (default-allow).
pub fn evaluate_command(
    command: &str,
    config: &CompiledShellConfig,
) -> std::result::Result<(), ShellSecurityError> {
    // 1. Permit check (short-circuit allow)
    for rule in &config.permit {
        if rule.regex.is_match(command) {
            return Ok(());
        }
    }

    // 2. Deny check
    for rule in &config.deny {
        if rule.regex.is_match(command) {
            return Err(ShellSecurityError::BlockedCommandPattern {
                pattern: rule.reason.clone(),
                command: command.to_string(),
            });
        }
    }

    // 3. Default allow
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_builtin_config() {
        let config = parse_shell_config(BUILTIN_CONFIG_YAML)
            .expect("builtin config.yaml should parse successfully");

        // Should have all the deny patterns from security.rs defaults
        assert!(
            config.deny.len() >= 19,
            "expected at least 19 deny patterns, got {}",
            config.deny.len()
        );

        // Permit should be empty in builtin
        assert!(
            config.permit.is_empty(),
            "builtin should have no permit patterns"
        );

        // Settings should have defaults
        assert_eq!(config.settings.max_command_length, 4096);
        assert_eq!(config.settings.max_env_value_length, 1024);
        assert!(config.settings.enable_audit_logging);
    }

    #[test]
    fn test_all_deny_patterns_are_valid_regex() {
        let config =
            parse_shell_config(BUILTIN_CONFIG_YAML).expect("builtin config.yaml should parse");

        for rule in &config.deny {
            regex::Regex::new(&rule.pattern).unwrap_or_else(|e| {
                panic!("deny pattern '{}' is not valid regex: {}", rule.pattern, e)
            });
        }
    }

    #[test]
    fn test_all_deny_patterns_have_reasons() {
        let config =
            parse_shell_config(BUILTIN_CONFIG_YAML).expect("builtin config.yaml should parse");

        for rule in &config.deny {
            assert!(
                !rule.reason.is_empty(),
                "deny pattern '{}' has empty reason",
                rule.pattern
            );
        }
    }

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
deny: []
permit: []
"#;
        let config = parse_shell_config(yaml).expect("minimal config should parse");
        assert!(config.deny.is_empty());
        assert!(config.permit.is_empty());
        // Settings should use defaults
        assert_eq!(config.settings.max_command_length, 4096);
    }

    #[test]
    fn test_parse_config_with_permit_and_deny() {
        let yaml = r#"
permit:
  - pattern: 'sed --version'
    reason: "Allow version check"
deny:
  - pattern: 'sed\s+.*'
    reason: "Use edit tools instead"
settings:
  max_command_length: 8192
"#;
        let config = parse_shell_config(yaml).expect("config should parse");
        assert_eq!(config.permit.len(), 1);
        assert_eq!(config.deny.len(), 1);
        assert_eq!(config.permit[0].pattern, "sed --version");
        assert_eq!(config.deny[0].reason, "Use edit tools instead");
        assert_eq!(config.settings.max_command_length, 8192);
        // Unset settings should use defaults
        assert_eq!(config.settings.max_env_value_length, 1024);
    }

    #[test]
    fn test_default_config_is_empty() {
        let config = ShellSecurityConfig::default();
        assert!(config.permit.is_empty());
        assert!(config.deny.is_empty());
        assert_eq!(config.settings.max_command_length, 4096);
    }

    #[test]
    fn test_merge_deny_lists_are_additive() {
        let mut base = parse_shell_config(BUILTIN_CONFIG_YAML).unwrap();
        let base_deny_count = base.deny.len();

        let overlay = ShellSecurityConfig {
            deny: vec![PatternRule {
                pattern: r"docker\s+rm".to_string(),
                reason: "Block docker rm".to_string(),
            }],
            permit: vec![],
            settings: ShellSettings::default(),
        };

        base.merge(overlay);
        assert_eq!(base.deny.len(), base_deny_count + 1);
        assert_eq!(base.deny.last().unwrap().pattern, r"docker\s+rm");
    }

    #[test]
    fn test_merge_permit_lists_are_additive() {
        let mut base = parse_shell_config(BUILTIN_CONFIG_YAML).unwrap();
        assert!(base.permit.is_empty());

        let overlay = ShellSecurityConfig {
            deny: vec![],
            permit: vec![PatternRule {
                pattern: r"sed\s+-i".to_string(),
                reason: "Project uses sed".to_string(),
            }],
            settings: ShellSettings::default(),
        };

        base.merge(overlay);
        assert_eq!(base.permit.len(), 1);
        assert_eq!(base.permit[0].pattern, r"sed\s+-i");
    }

    #[test]
    fn test_merge_settings_from_later_layer_wins() {
        let mut base = parse_shell_config(BUILTIN_CONFIG_YAML).unwrap();
        assert_eq!(base.settings.max_command_length, 4096);

        let overlay = ShellSecurityConfig {
            deny: vec![],
            permit: vec![],
            settings: ShellSettings {
                max_command_length: 8192,
                ..ShellSettings::default()
            },
        };

        base.merge(overlay);
        assert_eq!(base.settings.max_command_length, 8192);
    }

    #[test]
    fn test_load_builtin_only_no_overlay_dirs() {
        // Pass empty overlay paths — only builtin should load
        let config = load_shell_config_from_paths(&[]);
        assert!(!config.deny.is_empty(), "should have builtin deny patterns");
    }

    #[test]
    fn test_load_with_project_overlay_dir() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let overlay_dir = temp_dir.path().join("overlay");
        std::fs::create_dir_all(&overlay_dir).unwrap();

        let overlay_yaml = r#"
permit:
  - pattern: 'sed\s+-i'
    reason: "Project uses sed"
deny:
  - pattern: 'docker\s+rm'
    reason: "No docker rm"
settings:
  max_command_length: 16384
"#;
        std::fs::write(overlay_dir.join("config.yaml"), overlay_yaml).unwrap();

        let config = load_shell_config_from_paths(&[overlay_dir]);

        // Builtin denies + project deny
        assert!(config.deny.iter().any(|r| r.pattern == r"docker\s+rm"));
        // Project permit
        assert!(config.permit.iter().any(|r| r.pattern == r"sed\s+-i"));
        // Project settings override
        assert_eq!(config.settings.max_command_length, 16384);
    }

    #[test]
    fn test_load_missing_dirs_are_skipped() {
        let config = load_shell_config_from_paths(&[
            PathBuf::from("/nonexistent/overlay"),
            PathBuf::from("/also/missing"),
        ]);
        // Should still have builtin patterns
        assert!(!config.deny.is_empty());
    }

    #[test]
    fn test_load_malformed_overlay_is_skipped() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let overlay_dir = temp_dir.path().join("bad");
        std::fs::create_dir_all(&overlay_dir).unwrap();
        std::fs::write(
            overlay_dir.join("config.yaml"),
            "this: is: not: valid: yaml: [[[",
        )
        .unwrap();

        let config = load_shell_config_from_paths(&[overlay_dir]);
        // Should still have builtin patterns (malformed overlay skipped)
        assert!(!config.deny.is_empty());
    }

    #[test]
    fn test_load_two_overlay_dirs_both_merge() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        // User overlay
        let user_dir = temp_dir.path().join("user");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(
            user_dir.join("config.yaml"),
            r#"
deny:
  - pattern: 'docker\s+rm'
    reason: "User blocks docker rm"
"#,
        )
        .unwrap();

        // Project overlay
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(
            project_dir.join("config.yaml"),
            r#"
permit:
  - pattern: 'sudo\s+apt'
    reason: "Allow apt via sudo"
settings:
  max_command_length: 2048
"#,
        )
        .unwrap();

        let config = load_shell_config_from_paths(&[user_dir, project_dir]);

        // User deny merged
        assert!(config.deny.iter().any(|r| r.pattern == r"docker\s+rm"));
        // Project permit merged
        assert!(config.permit.iter().any(|r| r.pattern == r"sudo\s+apt"));
        // Project settings win (last layer)
        assert_eq!(config.settings.max_command_length, 2048);
    }

    // -----------------------------------------------------------------------
    // CompiledShellConfig and evaluate_command tests
    // -----------------------------------------------------------------------

    fn make_config(permit: &[(&str, &str)], deny: &[(&str, &str)]) -> CompiledShellConfig {
        let config = ShellSecurityConfig {
            permit: permit
                .iter()
                .map(|(p, r)| PatternRule {
                    pattern: p.to_string(),
                    reason: r.to_string(),
                })
                .collect(),
            deny: deny
                .iter()
                .map(|(p, r)| PatternRule {
                    pattern: p.to_string(),
                    reason: r.to_string(),
                })
                .collect(),
            settings: ShellSettings::default(),
        };
        CompiledShellConfig::compile(&config).expect("test patterns should compile")
    }

    #[test]
    fn test_deny_pattern_blocks_command() {
        let compiled = make_config(&[], &[(r"rm\s+-rf", "Dangerous delete")]);
        let result = evaluate_command("rm -rf /tmp", &compiled);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ShellSecurityError::BlockedCommandPattern { pattern, .. } => {
                assert_eq!(pattern, "Dangerous delete");
            }
            _ => panic!("expected BlockedCommandPattern"),
        }
    }

    #[test]
    fn test_permit_and_deny_permit_wins() {
        let compiled = make_config(
            &[(r"sed\s+-i", "Project uses sed -i")],
            &[(r"sed\s+", "Use edit tools instead")],
        );
        assert!(evaluate_command("sed -i 's/foo/bar/' file.txt", &compiled).is_ok());
    }

    #[test]
    fn test_no_match_is_allowed() {
        let compiled = make_config(&[], &[(r"rm\s+-rf", "Dangerous delete")]);
        assert!(evaluate_command("echo hello", &compiled).is_ok());
    }

    #[test]
    fn test_permit_only_allows_everything() {
        let compiled = make_config(
            &[(r".*", "Allow all")],
            &[(r"rm\s+-rf", "Dangerous delete")],
        );
        assert!(evaluate_command("rm -rf /", &compiled).is_ok());
    }

    #[test]
    fn test_deny_error_includes_reason() {
        let compiled = make_config(&[], &[(r"sudo\s+", "sudo is not allowed in this project")]);
        let result = evaluate_command("sudo apt install", &compiled);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("sudo is not allowed in this project"));
    }

    #[test]
    fn test_invalid_regex_produces_compile_error() {
        let config = ShellSecurityConfig {
            permit: vec![],
            deny: vec![PatternRule {
                pattern: "[invalid(regex".to_string(),
                reason: "Bad pattern".to_string(),
            }],
            settings: ShellSettings::default(),
        };
        let result = CompiledShellConfig::compile(&config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.pattern, "[invalid(regex");
    }

    #[test]
    fn test_compile_builtin_config_succeeds() {
        let config = parse_shell_config(BUILTIN_CONFIG_YAML).unwrap();
        let compiled = CompiledShellConfig::compile(&config);
        assert!(compiled.is_ok());
        assert!(!compiled.unwrap().deny.is_empty());
    }

    #[test]
    fn test_empty_config_allows_everything() {
        let compiled = make_config(&[], &[]);
        assert!(evaluate_command("rm -rf /", &compiled).is_ok());
        assert!(evaluate_command("sudo reboot", &compiled).is_ok());
    }

    #[test]
    fn test_deny_without_permit_blocks() {
        let compiled = make_config(
            &[],
            &[
                (r"sudo\s+", "No sudo"),
                (r"rm\s+-rf", "No recursive delete"),
            ],
        );
        assert!(evaluate_command("sudo apt install", &compiled).is_err());
        assert!(evaluate_command("rm -rf /", &compiled).is_err());
        assert!(evaluate_command("echo hello", &compiled).is_ok());
    }

    #[test]
    fn test_permit_does_not_match_allows_deny_to_block() {
        let compiled = make_config(
            &[(r"sed --version", "Allow version check")],
            &[(r"sed\s+", "Use edit tools")],
        );
        assert!(evaluate_command("sed --version", &compiled).is_ok());
        assert!(evaluate_command("sed -i 's/a/b/' f", &compiled).is_err());
    }
}
