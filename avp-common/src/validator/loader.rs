//! Validator loader with directory stacking precedence.
//!
//! Loads validators from multiple directories with precedence:
//! 1. Builtin validators (embedded in the binary) - lowest precedence
//! 2. User validators (~/<AVP_DIR>/validators)
//! 3. Project validators (./<AVP_DIR>/validators) - highest precedence
//!
//! Later sources override earlier ones with the same name.
//!
//! The loader implements [`TemplateContentProvider`] from `swissarmyhammer_templating`,
//! allowing it to be used with the unified [`LibraryPartialAdapter`] for partial
//! template support. This follows the same pattern as prompts and rules.
//!
//! # YAML Include Expansion
//!
//! The loader supports `@path/to/file` references in validator frontmatter that
//! expand to YAML file contents. See [`YamlExpander`] for details.

use std::collections::HashMap;
use std::path::Path;

use swissarmyhammer_directory::{AvpConfig, FileSource, ManagedDirectory, VirtualFileSystem, YamlExpander};
use swissarmyhammer_templating::partials::TemplateContentProvider;

use crate::context::AvpContext;
use crate::error::AvpError;

use super::parser::parse_validator_with_expansion;
use super::types::{MatchContext, Validator, ValidatorSource};

/// Loader for validators with directory stacking precedence.
///
/// The loader manages validators from multiple sources and provides
/// methods to find validators matching specific criteria.
#[derive(Debug)]
pub struct ValidatorLoader {
    /// Map of validator names to validators.
    validators: HashMap<String, Validator>,
    /// YAML expander for `@` include references.
    expander: YamlExpander<AvpConfig>,
}

impl Default for ValidatorLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ValidatorLoader {
    /// Create a new empty validator loader.
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            expander: YamlExpander::new(),
        }
    }

    /// Create a new loader with a pre-configured expander.
    pub fn with_expander(expander: YamlExpander<AvpConfig>) -> Self {
        Self {
            validators: HashMap::new(),
            expander,
        }
    }

    /// Get a mutable reference to the expander for adding builtins.
    pub fn expander_mut(&mut self) -> &mut YamlExpander<AvpConfig> {
        &mut self.expander
    }

    /// Load YAML includes from all directories.
    ///
    /// This should be called before loading validators to ensure
    /// `@` references can be expanded.
    pub fn load_includes(&mut self) -> Result<(), AvpError> {
        self.expander
            .load_all()
            .map_err(|e| AvpError::Context(format!("Failed to load YAML includes: {}", e)))
    }

    /// Load validators from directories specified in the context.
    ///
    /// This loads validators using the AvpContext to get directory paths:
    /// 1. User validators from ~/<AVP_DIR>/validators (if exists)
    /// 2. Project validators from ./<AVP_DIR>/validators (if exists)
    ///
    /// Later sources override earlier ones with the same name.
    /// Call `load_builtins()` before this if you want builtin validators.
    pub fn load_from_context(&mut self, context: &AvpContext) -> Result<(), AvpError> {
        // Load from user directory first (lower precedence)
        if let Some(home_dir) = context.home_validators_dir() {
            if home_dir.exists() {
                self.load_directory(&home_dir, ValidatorSource::User)?;
            }
        }

        // Load from project directory (higher precedence, overrides user)
        let project_dir = context.project_validators_dir();
        if project_dir.exists() {
            self.load_directory(&project_dir, ValidatorSource::Project)?;
        }

        Ok(())
    }

    /// Load all validators with proper precedence (standalone, no context).
    ///
    /// This loads validators from:
    /// 1. Builtin validators (call `load_builtins()` first if needed)
    /// 2. User validators from ~/<AVP_DIR>/validators
    /// 3. Project validators from ./<AVP_DIR>/validators
    ///
    /// Later sources override earlier ones with the same name.
    ///
    /// Note: Prefer `load_from_context()` when an AvpContext is available.
    /// Note: Call `load_includes()` before this to enable `@` reference expansion.
    pub fn load_all(&mut self) -> Result<(), AvpError> {
        // Use VirtualFileSystem with AvpConfig for directory stacking
        let mut vfs = VirtualFileSystem::<AvpConfig>::new("validators");

        // Load from all directories following precedence
        if let Err(e) = vfs.load_all() {
            tracing::warn!("Failed to load validators from some directories: {}", e);
        }

        // Parse each loaded file as a validator
        for file_entry in vfs.list() {
            let source = match file_entry.source {
                FileSource::Builtin | FileSource::Dynamic => ValidatorSource::Builtin,
                FileSource::User => ValidatorSource::User,
                FileSource::Local => ValidatorSource::Project,
            };

            match parse_validator_with_expansion(
                &file_entry.content,
                file_entry.path.clone(),
                source,
                &self.expander,
            ) {
                Ok(validator) => {
                    tracing::debug!(
                        "Loaded validator '{}' from {} ({})",
                        validator.name(),
                        validator.source,
                        file_entry.path.display()
                    );
                    self.validators
                        .insert(validator.name().to_string(), validator);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse validator at {}: {}",
                        file_entry.path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Load validators from a specific directory.
    pub fn load_directory(&mut self, path: &Path, source: ValidatorSource) -> Result<(), AvpError> {
        if !path.exists() {
            return Ok(());
        }

        // Walk the directory looking for .md files
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let file_path = entry.path();

            // Only process .md files
            if file_path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }

            match std::fs::read_to_string(file_path) {
                Ok(content) => {
                    match parse_validator_with_expansion(
                        &content,
                        file_path.to_path_buf(),
                        source.clone(),
                        &self.expander,
                    ) {
                        Ok(validator) => {
                            tracing::debug!(
                                "Loaded validator '{}' from {} ({})",
                                validator.name(),
                                validator.source,
                                file_path.display()
                            );
                            self.validators
                                .insert(validator.name().to_string(), validator);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to parse validator at {}: {}",
                                file_path.display(),
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to read validator file {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Add a builtin validator from embedded content.
    pub fn add_builtin(&mut self, name: &str, content: &str) {
        use std::path::PathBuf;

        match parse_validator_with_expansion(
            content,
            PathBuf::from(format!("builtin:/{}.md", name)),
            ValidatorSource::Builtin,
            &self.expander,
        ) {
            Ok(validator) => {
                self.validators
                    .insert(validator.name().to_string(), validator);
            }
            Err(e) => {
                tracing::error!("Failed to parse builtin validator '{}': {}", name, e);
            }
        }
    }

    /// Add a builtin YAML include for expansion.
    ///
    /// This should be called before adding validators that reference the include.
    pub fn add_builtin_include(&mut self, name: &str, content: &str) -> Result<(), AvpError> {
        self.expander
            .add_builtin(name, content)
            .map_err(|e| AvpError::Context(format!("Failed to add builtin include '{}': {}", name, e)))
    }

    /// Get a validator by name.
    pub fn get(&self, name: &str) -> Option<&Validator> {
        self.validators.get(name)
    }

    /// List all loaded validators.
    pub fn list(&self) -> Vec<&Validator> {
        self.validators.values().collect()
    }

    /// Get the number of loaded validators.
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if there are no loaded validators.
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Find validators matching a hook event context.
    ///
    /// Returns all validators that match the given context criteria.
    pub fn matching(&self, ctx: &MatchContext) -> Vec<&Validator> {
        self.validators
            .values()
            .filter(|v| v.matches(ctx))
            .collect()
    }

    /// List all validator names.
    pub fn list_names(&self) -> Vec<String> {
        self.validators.keys().cloned().collect()
    }

    /// Get all directories that would be searched for validators.
    pub fn get_directories() -> Vec<std::path::PathBuf> {
        let mut dirs = Vec::new();

        // User directory
        if let Ok(dir) = ManagedDirectory::<AvpConfig>::from_user_home() {
            let validators_dir = dir.subdir("validators");
            if validators_dir.exists() {
                dirs.push(validators_dir);
            }
        }

        // Project directory
        if let Ok(dir) = ManagedDirectory::<AvpConfig>::from_git_root() {
            let validators_dir = dir.subdir("validators");
            if validators_dir.exists() {
                dirs.push(validators_dir);
            }
        }

        dirs
    }
}

/// Implement TemplateContentProvider for ValidatorLoader.
///
/// This allows the validator loader to be used with the unified LibraryPartialAdapter,
/// following the same pattern as PromptLibrary and RuleLibrary. Validators can then
/// use `{% include 'partial-name' %}` to include partials from the _partials/ directory.
impl TemplateContentProvider for ValidatorLoader {
    fn get_template_content(&self, name: &str) -> Option<String> {
        self.get(name).map(|v| v.body.clone())
    }

    fn list_template_names(&self) -> Vec<String> {
        self.list_names()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HookType;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_loader_new() {
        let loader = ValidatorLoader::new();
        assert!(loader.is_empty());
        assert_eq!(loader.len(), 0);
    }

    #[test]
    fn test_loader_add_builtin() {
        let mut loader = ValidatorLoader::new();

        let content = r#"---
name: test-builtin
description: A test builtin validator
severity: error
trigger: PreToolUse
---

Check for issues.
"#;

        loader.add_builtin("test-builtin", content);

        assert_eq!(loader.len(), 1);
        let validator = loader.get("test-builtin").unwrap();
        assert_eq!(validator.source, ValidatorSource::Builtin);
    }

    #[test]
    fn test_loader_load_directory() {
        let temp = TempDir::new().unwrap();
        let validators_dir = temp.path().join("validators");
        fs::create_dir_all(&validators_dir).unwrap();

        // Create a test validator file
        let validator_content = r#"---
name: test-file-validator
description: Test validator from file
severity: warn
trigger: PostToolUse
---

Validation instructions.
"#;

        fs::write(validators_dir.join("test.md"), validator_content).unwrap();

        let mut loader = ValidatorLoader::new();
        loader
            .load_directory(&validators_dir, ValidatorSource::Project)
            .unwrap();

        assert_eq!(loader.len(), 1);
        let validator = loader.get("test-file-validator").unwrap();
        assert_eq!(validator.source, ValidatorSource::Project);
    }

    #[test]
    fn test_loader_matching() {
        let mut loader = ValidatorLoader::new();

        // Add validators for different hooks
        loader.add_builtin(
            "pre-tool",
            r#"---
name: pre-tool
description: Pre-tool validator
severity: error
trigger: PreToolUse
---
Check before tool.
"#,
        );

        loader.add_builtin(
            "post-tool",
            r#"---
name: post-tool
description: Post-tool validator
severity: warn
trigger: PostToolUse
---
Check after tool.
"#,
        );

        loader.add_builtin(
            "write-only",
            r#"---
name: write-only
description: Only for Write tool
severity: error
trigger: PreToolUse
match:
  tools: [Write]
---
Check Write tool.
"#,
        );

        // Test matching by hook type
        let ctx = MatchContext::new(HookType::PreToolUse);
        let pre_validators = loader.matching(&ctx);
        assert_eq!(pre_validators.len(), 1); // pre-tool, but not write-only (needs tool)

        let ctx = MatchContext::new(HookType::PostToolUse);
        let post_validators = loader.matching(&ctx);
        assert_eq!(post_validators.len(), 1);

        // Test matching by tool
        let ctx = MatchContext::new(HookType::PreToolUse).with_tool("Write");
        let write_validators = loader.matching(&ctx);
        assert_eq!(write_validators.len(), 2); // pre-tool and write-only

        let ctx = MatchContext::new(HookType::PreToolUse).with_tool("Bash");
        let bash_validators = loader.matching(&ctx);
        assert_eq!(bash_validators.len(), 1); // only pre-tool
    }

    #[test]
    fn test_loader_precedence() {
        let mut loader = ValidatorLoader::new();

        // Add builtin
        loader.add_builtin(
            "override-test",
            r#"---
name: override-test
description: Builtin version
severity: info
trigger: PreToolUse
---
Builtin body.
"#,
        );

        assert_eq!(
            loader.get("override-test").unwrap().description(),
            "Builtin version"
        );

        // Create temp directory for user validators
        let temp = TempDir::new().unwrap();
        let validators_dir = temp.path();

        fs::write(
            validators_dir.join("override-test.md"),
            r#"---
name: override-test
description: User version
severity: error
trigger: PreToolUse
---
User body.
"#,
        )
        .unwrap();

        // Load user validators (should override builtin)
        loader
            .load_directory(validators_dir, ValidatorSource::User)
            .unwrap();

        assert_eq!(
            loader.get("override-test").unwrap().description(),
            "User version"
        );
        assert_eq!(
            loader.get("override-test").unwrap().source,
            ValidatorSource::User
        );
    }

    #[test]
    fn test_template_content_provider() {
        let mut loader = ValidatorLoader::new();

        loader.add_builtin(
            "test-validator",
            r#"---
name: test-validator
description: Test validator
severity: error
trigger: PreToolUse
---
Check for issues in the code.
"#,
        );

        // Test TemplateContentProvider implementation
        assert!(loader.get_template_content("test-validator").is_some());
        let content = loader.get_template_content("test-validator").unwrap();
        assert!(content.contains("Check for issues"));

        // Non-existent should return None
        assert!(loader.get_template_content("nonexistent").is_none());

        // list_template_names should work
        let names = loader.list_template_names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"test-validator".to_string()));
    }

    #[test]
    fn test_loader_with_partials() {
        let mut loader = ValidatorLoader::new();

        // Add a regular validator
        loader.add_builtin(
            "regular-validator",
            r#"---
name: regular-validator
description: Regular validator
severity: error
trigger: PreToolUse
---
Check for issues.
"#,
        );

        // Add a partial (identified by _partials/ prefix)
        loader.add_builtin(
            "_partials/response-format",
            r#"---
name: _partials/response-format
description: Response format partial
severity: info
trigger: PreToolUse
---
{% partial %}

Return JSON with status and message fields.
"#,
        );

        // Both should be accessible
        assert!(loader.get("regular-validator").is_some());
        assert!(loader.get("_partials/response-format").is_some());
        assert_eq!(loader.len(), 2);

        // TemplateContentProvider should return both
        let names = loader.list_template_names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_load_directory_with_partials() {
        let temp = TempDir::new().unwrap();
        let validators_dir = temp.path().join("validators");
        let partials_dir = validators_dir.join("_partials");
        fs::create_dir_all(&partials_dir).unwrap();

        // Create a regular validator
        fs::write(
            validators_dir.join("test-validator.md"),
            r#"---
name: test-validator
description: Test validator
severity: error
trigger: PreToolUse
---
Check issues. {% include 'response-helper' %}
"#,
        )
        .unwrap();

        // Create a partial in _partials directory
        fs::write(
            partials_dir.join("response-helper.md"),
            r#"---
name: _partials/response-helper
description: Response helper partial
severity: info
trigger: PreToolUse
---
{% partial %}

Return JSON response.
"#,
        )
        .unwrap();

        let mut loader = ValidatorLoader::new();
        loader
            .load_directory(&validators_dir, ValidatorSource::Project)
            .unwrap();

        // Should load both the validator and the partial
        assert!(loader.get("test-validator").is_some());
        assert!(loader.get("_partials/response-helper").is_some());

        // TemplateContentProvider should expose both
        assert!(loader.get_template_content("test-validator").is_some());
        assert!(loader
            .get_template_content("_partials/response-helper")
            .is_some());
    }
}
