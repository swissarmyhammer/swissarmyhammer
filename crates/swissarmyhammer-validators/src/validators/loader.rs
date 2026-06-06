//! Validator loader with directory stacking precedence.
//!
//! Loads validators from multiple directories with precedence:
//! 1. Builtin validators (embedded in the binary) - lowest precedence
//! 2. User validators (~/.validators)
//! 3. Project validators (./.validators) - highest precedence
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
use std::path::{Path, PathBuf};

use swissarmyhammer_directory::{
    DirectoryConfig, ManagedDirectory, ValidatorsConfig, YamlExpander,
};
use swissarmyhammer_templating::partials::TemplateContentProvider;

use crate::error::AvpError;

use super::parser::{parse_ruleset_directory, parse_validator_with_expansion};
use super::types::{MatchContext, RuleSet, Validator, ValidatorSource};

/// Resolve the user (global) validators store directory, `~/.validators`.
///
/// Resolves home the same raw way the shared `mirdan::store` does
/// (`dirs::home_dir().join(ValidatorsConfig::DIR_NAME)`), so the loader and the
/// store agree on a single resolution mechanism for the same path. Returns
/// `None` when the home directory cannot be determined.
fn user_validators_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(ValidatorsConfig::DIR_NAME))
}

/// Loader for validators with directory stacking precedence.
///
/// The loader manages validators from multiple sources and provides
/// methods to find validators matching specific criteria.
///
/// Supports both legacy standalone validators and new RuleSet packages.
#[derive(Debug)]
pub struct ValidatorLoader {
    /// Map of validator names to validators (legacy format).
    validators: HashMap<String, Validator>,
    /// Map of RuleSet names to RuleSets (new format).
    rulesets: HashMap<String, RuleSet>,
    /// YAML expander for `@` include references.
    expander: YamlExpander<ValidatorsConfig>,
    /// RuleSet directories that failed to parse and were skipped, retained so a
    /// malformed validator is reported (by `check validators` / `sah doctor`)
    /// instead of silently dropped.
    load_failures: Vec<LoadFailure>,
}

/// A validator directory the loader could not parse, retained for reporting.
///
/// The loader keeps loading the rest of the stack when one validator is broken
/// (a broken validator never aborts the run); each failure is recorded here so
/// the lint surface can name the offending path and its parse problem.
#[derive(Debug, Clone)]
pub struct LoadFailure {
    /// The RuleSet directory that failed to parse.
    pub path: PathBuf,
    /// Which precedence layer the directory came from.
    pub source: ValidatorSource,
    /// The parse problem, formatted for display.
    pub error: String,
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
            rulesets: HashMap::new(),
            expander: YamlExpander::new(),
            load_failures: Vec::new(),
        }
    }

    /// Create a new loader with a pre-configured expander.
    pub fn with_expander(expander: YamlExpander<ValidatorsConfig>) -> Self {
        Self {
            validators: HashMap::new(),
            rulesets: HashMap::new(),
            expander,
            load_failures: Vec::new(),
        }
    }

    /// Get a mutable reference to the expander for adding builtins.
    pub fn expander_mut(&mut self) -> &mut YamlExpander<ValidatorsConfig> {
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

    /// Load all validators with proper precedence (standalone, no context).
    ///
    /// This loads validators from:
    /// 1. Builtin validators (call `load_builtins()` first if needed)
    /// 2. User validators from ~/.validators
    /// 3. Project validators from ./.validators
    ///
    /// Later sources override earlier ones with the same name.
    ///
    /// Note: Call `load_includes()` before this to enable `@` reference expansion.
    pub fn load_all(&mut self) -> Result<(), AvpError> {
        // Load RuleSets from the user directory (~/.validators)
        if let Some(validators_dir) = user_validators_dir() {
            if validators_dir.exists() {
                self.load_rulesets_directory(&validators_dir, ValidatorSource::User)?;
            }
        }

        // Load RuleSets from the project directory (<git_root>/.validators)
        if let Ok(dir) = ManagedDirectory::<ValidatorsConfig>::from_git_root() {
            let validators_dir = dir.root();
            if validators_dir.exists() {
                self.load_rulesets_directory(validators_dir, ValidatorSource::Project)?;
            }
        }

        Ok(())
    }

    /// Parse content as a validator and insert into the collection.
    fn parse_and_insert_validator(&mut self, content: &str, path: &Path, source: ValidatorSource) {
        match parse_validator_with_expansion(content, path.to_path_buf(), source, &self.expander) {
            Ok(validator) => {
                tracing::debug!(
                    "Loaded validator '{}' from {} ({})",
                    validator.name(),
                    validator.source,
                    path.display()
                );
                self.validators
                    .insert(validator.name().to_string(), validator);
            }
            Err(e) => Self::log_parse_error(&e, path),
        }
    }

    /// Log validator parse errors appropriately.
    fn log_parse_error(error: &AvpError, path: &Path) {
        if error.is_partial() {
            tracing::trace!("Skipping partial: {}", path.display());
        } else {
            tracing::warn!("Failed to parse validator at {}: {}", path.display(), error);
        }
    }

    /// Load validators from a specific directory.
    pub fn load_directory(&mut self, path: &Path, source: ValidatorSource) -> Result<(), AvpError> {
        if !path.exists() {
            return Ok(());
        }

        for entry in Self::walk_markdown_files(path) {
            self.load_validator_file(entry.path(), source.clone());
        }

        Ok(())
    }

    /// Walk a directory and yield markdown file entries.
    fn walk_markdown_files(path: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
    }

    /// Load a single validator file.
    fn load_validator_file(&mut self, file_path: &Path, source: ValidatorSource) {
        match std::fs::read_to_string(file_path) {
            Ok(content) => {
                self.parse_and_insert_validator(&content, file_path, source);
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
                // Partials are expected to fail parsing - log at trace level
                if e.is_partial() {
                    tracing::trace!("Skipping partial '{}': {}", name, e);
                } else {
                    tracing::error!("Failed to parse builtin validator '{}': {}", name, e);
                }
            }
        }
    }

    /// Add a builtin YAML include for expansion.
    ///
    /// This should be called before adding validators that reference the include.
    pub fn add_builtin_include(&mut self, name: &str, content: &str) -> Result<(), AvpError> {
        self.expander.add_builtin(name, content).map_err(|e| {
            AvpError::Context(format!("Failed to add builtin include '{}': {}", name, e))
        })
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

        // User directory (~/.validators)
        if let Some(validators_dir) = user_validators_dir() {
            if validators_dir.exists() {
                dirs.push(validators_dir);
            }
        }

        // Project directory (<git_root>/.validators)
        if let Ok(dir) = ManagedDirectory::<ValidatorsConfig>::from_git_root() {
            let validators_dir = dir.root();
            if validators_dir.exists() {
                dirs.push(validators_dir.to_path_buf());
            }
        }

        dirs
    }

    // ========================================================================
    // RuleSet Management (New Architecture)
    // ========================================================================

    /// Load RuleSets from a directory.
    ///
    /// Scans for subdirectories containing VALIDATOR.md and loads them as RuleSets.
    /// Later-loaded RuleSets override earlier ones with the same name.
    pub fn load_rulesets_directory(
        &mut self,
        path: &Path,
        source: ValidatorSource,
    ) -> Result<(), AvpError> {
        if !path.exists() {
            return Ok(());
        }

        if !path.is_dir() {
            return Err(AvpError::Validator {
                validator: path.display().to_string(),
                message: "not a directory".to_string(),
            });
        }

        // Scan for subdirectories with VALIDATOR.md
        let entries = std::fs::read_dir(path).map_err(|e| AvpError::Validator {
            validator: path.display().to_string(),
            message: format!("failed to read directory: {}", e),
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| AvpError::Validator {
                validator: path.display().to_string(),
                message: format!("failed to read directory entry: {}", e),
            })?;

            let dir_path = entry.path();

            // Skip non-directories
            if !dir_path.is_dir() {
                continue;
            }

            // Skip _partials directories
            if dir_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('_'))
                .unwrap_or(false)
            {
                continue;
            }

            // Check if this directory contains VALIDATOR.md
            let manifest_path = dir_path.join("VALIDATOR.md");
            if !manifest_path.exists() {
                continue;
            }

            // Parse the RuleSet
            match parse_ruleset_directory(&dir_path, source.clone(), Some(&self.expander)) {
                Ok(ruleset) => {
                    tracing::debug!(
                        "Loaded RuleSet '{}' from {} with {} rules ({})",
                        ruleset.name(),
                        source,
                        ruleset.rules.len(),
                        dir_path.display()
                    );
                    self.rulesets.insert(ruleset.name().to_string(), ruleset);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse RuleSet at {}: {}", dir_path.display(), e);
                    self.load_failures.push(LoadFailure {
                        path: dir_path.clone(),
                        source: source.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Add a builtin RuleSet from embedded directory structure.
    ///
    /// This is used by the build system to load RuleSets embedded in the binary.
    pub fn add_builtin_ruleset(&mut self, ruleset: RuleSet) {
        tracing::debug!(
            "Adding builtin RuleSet '{}' with {} rules",
            ruleset.name(),
            ruleset.rules.len()
        );
        self.rulesets.insert(ruleset.name().to_string(), ruleset);
    }

    /// Get a RuleSet by name.
    pub fn get_ruleset(&self, name: &str) -> Option<&RuleSet> {
        self.rulesets.get(name)
    }

    /// List all loaded RuleSets.
    pub fn list_rulesets(&self) -> Vec<&RuleSet> {
        self.rulesets.values().collect()
    }

    /// Get the number of loaded RuleSets.
    pub fn ruleset_count(&self) -> usize {
        self.rulesets.len()
    }

    /// Find RuleSets matching a hook event context.
    ///
    /// Returns all RuleSets that match the given context criteria.
    pub fn matching_rulesets(&self, ctx: &MatchContext) -> Vec<&RuleSet> {
        self.rulesets
            .values()
            .filter(|rs| rs.matches(ctx))
            .collect()
    }

    /// List all RuleSet names.
    pub fn list_ruleset_names(&self) -> Vec<String> {
        self.rulesets.keys().cloned().collect()
    }

    /// The RuleSet directories that failed to parse during loading.
    ///
    /// A malformed validator is skipped (it never crashes the run) but recorded
    /// here so the lint surface can report it as an error rather than dropping it
    /// silently. Empty when every directory parsed cleanly.
    pub fn load_failures(&self) -> &[LoadFailure] {
        &self.load_failures
    }

    /// Get diagnostic information about validator loading.
    ///
    /// Returns information about:
    /// - Which directories are being searched
    /// - Whether each directory exists
    /// - How many validators were loaded from each source
    ///
    /// Useful for debugging why validators aren't being loaded.
    pub fn diagnostics(&self) -> ValidatorDiagnostics {
        let mut user_dir_info = DirectoryInfo {
            path: None,
            exists: false,
            error: None,
        };

        let mut project_dir_info = DirectoryInfo {
            path: None,
            exists: false,
            error: None,
        };

        // Check user directory (~/.validators)
        match user_validators_dir() {
            Some(validators_dir) => {
                user_dir_info.exists = validators_dir.exists();
                user_dir_info.path = Some(validators_dir);
            }
            None => {
                user_dir_info.error = Some("could not determine home directory".to_string());
            }
        }

        // Check project directory (<git_root>/.validators)
        match ManagedDirectory::<ValidatorsConfig>::from_git_root() {
            Ok(dir) => {
                let validators_dir = dir.root().to_path_buf();
                project_dir_info.exists = validators_dir.exists();
                project_dir_info.path = Some(validators_dir);
            }
            Err(e) => {
                project_dir_info.error = Some(format!("{}", e));
            }
        }

        // Count validators by source
        let mut builtin_count = 0;
        let mut user_count = 0;
        let mut project_count = 0;

        for v in self.validators.values() {
            match v.source {
                ValidatorSource::Builtin => builtin_count += 1,
                ValidatorSource::User => user_count += 1,
                ValidatorSource::Project => project_count += 1,
            }
        }

        ValidatorDiagnostics {
            user_directory: user_dir_info,
            project_directory: project_dir_info,
            builtin_count,
            user_count,
            project_count,
            total_count: self.validators.len(),
        }
    }
}

/// Information about a validator directory.
#[derive(Debug, Clone)]
pub struct DirectoryInfo {
    /// Path to the directory (if resolvable).
    pub path: Option<std::path::PathBuf>,
    /// Whether the directory exists.
    pub exists: bool,
    /// Error message if directory couldn't be resolved.
    pub error: Option<String>,
}

/// Diagnostic information about validator loading.
#[derive(Debug, Clone)]
pub struct ValidatorDiagnostics {
    /// Information about the user validators directory (~/.validators).
    pub user_directory: DirectoryInfo,
    /// Information about the project validators directory (./.validators).
    pub project_directory: DirectoryInfo,
    /// Number of builtin validators loaded.
    pub builtin_count: usize,
    /// Number of user validators loaded.
    pub user_count: usize,
    /// Number of project validators loaded.
    pub project_count: usize,
    /// Total number of validators loaded.
    pub total_count: usize,
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
    use std::fs;
    use tempfile::TempDir;

    /// RAII guard that restores the current working directory on drop.
    ///
    /// Tests that mutate the process CWD must be marked `#[serial_test::serial(cwd)]`
    /// so they do not race other CWD-sensitive tests.
    struct CwdGuard {
        original: std::path::PathBuf,
    }

    impl CwdGuard {
        fn change_to(dir: &Path) -> Self {
            let original = std::env::current_dir().expect("read cwd");
            std::env::set_current_dir(dir).expect("set cwd");
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    /// RAII guard that restores an environment variable on drop.
    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    /// Write a minimal RuleSet (VALIDATOR.md + one rule) under `base/<name>/`.
    fn write_ruleset(base: &Path, name: &str, description: &str) {
        let dir = base.join(name);
        fs::create_dir_all(dir.join("rules")).unwrap();
        fs::write(
            dir.join("VALIDATOR.md"),
            format!(
                "---\nname: {name}\ndescription: {description}\nseverity: error\n---\n\n# {name}\n"
            ),
        )
        .unwrap();
        fs::write(
            dir.join("rules/check.md"),
            "---\nname: check\ndescription: Check\n---\n\nCheck the code.\n",
        )
        .unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_load_all_discovers_user_and_project_validators_with_precedence() {
        // User store: ~/.validators/<name> (resolved via a temp HOME)
        let home = TempDir::new().unwrap();
        let user_validators = home.path().join(".validators");
        write_ruleset(&user_validators, "user-only", "User-only ruleset");
        write_ruleset(&user_validators, "shared", "User version");

        // Project store: <git_root>/.validators/<name>
        let project_root = TempDir::new().unwrap();
        fs::create_dir_all(project_root.path().join(".git")).unwrap();
        let project_validators = project_root.path().join(".validators");
        write_ruleset(&project_validators, "project-only", "Project-only ruleset");
        write_ruleset(&project_validators, "shared", "Project version");

        let _env = EnvVarGuard::set("HOME", home.path());
        let _cwd = CwdGuard::change_to(project_root.path());

        let mut loader = ValidatorLoader::new();
        loader.load_all().unwrap();

        // Both user-only and project-only RuleSets are discovered.
        assert!(
            loader.get_ruleset("user-only").is_some(),
            "user RuleSet from ~/.validators should load"
        );
        assert!(
            loader.get_ruleset("project-only").is_some(),
            "project RuleSet from ./.validators should load"
        );

        // Project overrides user for the same-named set.
        let shared = loader.get_ruleset("shared").expect("shared ruleset");
        assert_eq!(shared.source, ValidatorSource::Project);
        assert_eq!(shared.description(), "Project version");
    }

    /// Write a malformed RuleSet: a VALIDATOR.md whose frontmatter is broken
    /// (unterminated YAML), so the parser rejects it.
    fn write_malformed_ruleset(base: &Path, name: &str) {
        let dir = base.join(name);
        fs::create_dir_all(&dir).unwrap();
        // Missing the closing `---` and a name field → frontmatter parse failure.
        fs::write(
            dir.join("VALIDATOR.md"),
            "---\nseverity: not-a-real-severity\nmatch: [unterminated\n",
        )
        .unwrap();
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn load_all_collects_malformed_validator_as_a_failure_and_still_loads_the_valid_one() {
        // One valid and one malformed RuleSet side by side in the user store.
        let home = TempDir::new().unwrap();
        let user_validators = home.path().join(".validators");
        write_ruleset(&user_validators, "good-one", "A valid ruleset");
        write_malformed_ruleset(&user_validators, "broken-one");

        // An empty project root so only the user store contributes.
        let project_root = TempDir::new().unwrap();
        fs::create_dir_all(project_root.path().join(".git")).unwrap();

        let _env = EnvVarGuard::set("HOME", home.path());
        let _cwd = CwdGuard::change_to(project_root.path());

        let mut loader = ValidatorLoader::new();
        loader.load_all().unwrap();

        // The valid one loaded; the broken one did not crash the run.
        assert!(
            loader.get_ruleset("good-one").is_some(),
            "the valid ruleset alongside a broken one still loads"
        );

        // The broken one is recorded as a collected failure naming its path.
        let failures = loader.load_failures();
        assert_eq!(failures.len(), 1, "the malformed ruleset is collected once");
        let failure = &failures[0];
        assert!(
            failure.path.to_string_lossy().contains("broken-one"),
            "the failure names the offending validator path, got: {}",
            failure.path.display()
        );
        assert!(
            !failure.error.is_empty(),
            "the failure carries the parse problem"
        );
    }

    #[test]
    fn test_loader_new() {
        let loader = ValidatorLoader::new();
        assert!(loader.is_empty());
        assert_eq!(loader.len(), 0);
        assert!(
            loader.load_failures().is_empty(),
            "a fresh loader has no load failures"
        );
    }

    #[test]
    fn test_loader_add_builtin() {
        let mut loader = ValidatorLoader::new();

        let content = r#"---
name: test-builtin
description: A test builtin validator
severity: error
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

        // A validator with no match criteria matches everything.
        loader.add_builtin(
            "always",
            r#"---
name: always
description: Always-on validator
severity: error
---
Check everything.
"#,
        );

        // A validator scoped to the Write tool.
        loader.add_builtin(
            "write-only",
            r#"---
name: write-only
description: Only for Write tool
severity: error
match:
  tools: [Write]
---
Check Write tool.
"#,
        );

        // No tool context: only the always-on validator matches (write-only
        // requires a tool name).
        let ctx = MatchContext::new();
        assert_eq!(loader.matching(&ctx).len(), 1);

        // Write tool: both match.
        let ctx = MatchContext::new().with_tool("Write");
        assert_eq!(loader.matching(&ctx).len(), 2);

        // Bash tool: only the always-on validator matches.
        let ctx = MatchContext::new().with_tool("Bash");
        assert_eq!(loader.matching(&ctx).len(), 1);
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
    fn test_loader_skips_partials() {
        let mut loader = ValidatorLoader::new();

        // Add a regular validator
        loader.add_builtin(
            "regular-validator",
            r#"---
name: regular-validator
description: Regular validator
severity: error
---
Check for issues.
"#,
        );

        // Try to add a partial (identified by _partials/ prefix) - should be skipped
        loader.add_builtin(
            "_partials/shared-content",
            "{% partial %}\n\nShared content for templates.",
        );

        // Try to add content with {% partial %} marker - should be skipped
        loader.add_builtin("another-partial", "{% partial %}\n\nAnother partial.");

        // Only the regular validator should be loaded
        assert!(loader.get("regular-validator").is_some());
        assert!(loader.get("_partials/shared-content").is_none());
        assert!(loader.get("another-partial").is_none());
        assert_eq!(loader.len(), 1);
    }

    #[test]
    fn test_load_directory_skips_partials() {
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
---
Check issues.
"#,
        )
        .unwrap();

        // Create a partial in _partials directory - should be skipped
        fs::write(
            partials_dir.join("shared-content.md"),
            "{% partial %}\n\nShared content for templates.",
        )
        .unwrap();

        let mut loader = ValidatorLoader::new();
        loader
            .load_directory(&validators_dir, ValidatorSource::Project)
            .unwrap();

        // Should load only the validator, not the partial
        assert!(loader.get("test-validator").is_some());
        assert!(loader.get("_partials/shared-content").is_none());
        assert_eq!(loader.len(), 1);
    }

    #[test]
    fn test_loader_with_expander() {
        use swissarmyhammer_directory::{ValidatorsConfig, YamlExpander};

        let expander = YamlExpander::<ValidatorsConfig>::new();
        let mut loader = ValidatorLoader::with_expander(expander);

        assert!(loader.is_empty());
        // Verify expander is accessible via mutable reference
        let _ = loader.expander_mut();
    }

    #[test]
    fn test_loader_load_includes() {
        let mut loader = ValidatorLoader::new();
        // load_includes should succeed even if no includes exist
        let result = loader.load_includes();
        assert!(result.is_ok());
    }

    #[test]
    fn test_loader_load_all() {
        let mut loader = ValidatorLoader::new();
        // load_all loads from VirtualFileSystem directories
        // This may or may not find validators depending on the environment
        let result = loader.load_all();
        assert!(result.is_ok());
    }

    #[test]
    fn test_loader_get_directories() {
        let dirs = ValidatorLoader::get_directories();
        // Returns a list of validator directories (may be empty if none exist)
        // The function should not panic
        assert!(dirs.len() <= 2); // At most user + project directories
    }

    #[test]
    fn test_loader_diagnostics_empty() {
        let loader = ValidatorLoader::new();
        let diag = loader.diagnostics();

        // Empty loader should have zero counts
        assert_eq!(diag.builtin_count, 0);
        assert_eq!(diag.user_count, 0);
        assert_eq!(diag.project_count, 0);
        assert_eq!(diag.total_count, 0);
    }

    #[test]
    fn test_loader_diagnostics_with_validators() {
        let temp = TempDir::new().unwrap();
        let validators_dir = temp.path().join("validators");
        fs::create_dir_all(&validators_dir).unwrap();

        fs::write(
            validators_dir.join("user-validator.md"),
            r#"---
name: user-validator
description: User validator
severity: warn
---
Body.
"#,
        )
        .unwrap();

        let mut loader = ValidatorLoader::new();

        // Add a builtin
        loader.add_builtin(
            "builtin-test",
            r#"---
name: builtin-test
description: Builtin
severity: error
---
Body.
"#,
        );

        // Load user validator
        loader
            .load_directory(&validators_dir, ValidatorSource::User)
            .unwrap();

        let diag = loader.diagnostics();

        assert_eq!(diag.builtin_count, 1, "Should have 1 builtin");
        assert_eq!(diag.user_count, 1, "Should have 1 user validator");
        assert_eq!(diag.project_count, 0, "Should have 0 project validators");
        assert_eq!(diag.total_count, 2, "Should have 2 total");
    }

    #[test]
    fn test_directory_info_fields() {
        // Test DirectoryInfo struct fields are accessible
        let info = DirectoryInfo {
            path: Some(std::path::PathBuf::from("/test/path")),
            exists: true,
            error: None,
        };

        assert_eq!(info.path, Some(std::path::PathBuf::from("/test/path")));
        assert!(info.exists);
        assert!(info.error.is_none());

        let info_with_error = DirectoryInfo {
            path: None,
            exists: false,
            error: Some("Test error".to_string()),
        };

        assert!(info_with_error.path.is_none());
        assert!(!info_with_error.exists);
        assert_eq!(info_with_error.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_validator_diagnostics_fields() {
        // Test ValidatorDiagnostics struct fields are accessible
        let diag = ValidatorDiagnostics {
            user_directory: DirectoryInfo {
                path: Some(std::path::PathBuf::from("/home/user/.validators")),
                exists: true,
                error: None,
            },
            project_directory: DirectoryInfo {
                path: Some(std::path::PathBuf::from("/project/.validators")),
                exists: false,
                error: None,
            },
            builtin_count: 5,
            user_count: 2,
            project_count: 0,
            total_count: 7,
        };

        assert!(diag.user_directory.exists);
        assert!(!diag.project_directory.exists);
        assert_eq!(diag.builtin_count, 5);
        assert_eq!(diag.user_count, 2);
        assert_eq!(diag.project_count, 0);
        assert_eq!(diag.total_count, 7);
    }
}
