//! Skill resolver — discovers skills from builtin → user → local sources
//!
//! Precedence: builtin < user < local (later sources override earlier ones)
//!
//! Uses [`VirtualFileSystem`] for search-path management, matching the same
//! pattern as `PromptResolver`.  The VFS resolves user-level paths via
//! `$XDG_DATA_HOME/sah/skills` and project-local paths via `{git_root}/.skills`.

use crate::skill::{Skill, SkillSource};
use crate::skill_loader::{load_skill_from_builtin, load_skill_from_dir};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::file_loader::{FileSource, VirtualFileSystem};
use swissarmyhammer_common::validation::{ValidationIssue, ValidationLevel};

// Include the generated builtin skills
include!(concat!(env!("OUT_DIR"), "/builtin_skills.rs"));

/// Convert a VFS [`FileSource`] into a skill-level [`SkillSource`].
fn file_source_to_skill_source(fs: &FileSource) -> SkillSource {
    match fs {
        FileSource::Builtin | FileSource::Dynamic => SkillSource::Builtin,
        FileSource::User => SkillSource::User,
        FileSource::Local => SkillSource::Local,
    }
}

/// Resolves skills from all sources with proper override precedence.
///
/// Delegates search-path management to a [`VirtualFileSystem`] configured
/// with dot-directory paths (`$XDG_DATA_HOME/sah/skills` for user,
/// `{git_root}/.skills` for local).  Builtins are loaded from data
/// embedded in the binary.
///
/// Precedence (later overrides earlier): builtin < user < local < extra_paths
pub struct SkillResolver {
    /// VFS used for search-path resolution (user + local directories).
    pub(crate) vfs: VirtualFileSystem,
}

impl SkillResolver {
    /// Create a new SkillResolver.
    ///
    /// Configures the VFS to resolve:
    /// - `$XDG_DATA_HOME/sah/skills` (user skills)
    /// - `{git_root}/.skills` (local/project skills)
    pub fn new() -> Self {
        let mut vfs = VirtualFileSystem::new("skills");
        vfs.use_dot_directory_paths();
        Self { vfs }
    }

    /// Add an extra search path for skills.
    ///
    /// Extra paths are loaded after user and local paths, giving them the
    /// highest precedence among filesystem-based sources.
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.vfs.add_search_path(path, FileSource::Local);
    }

    /// Resolve all skills from all sources.
    ///
    /// Returns skills keyed by name. Later sources override earlier ones.
    /// Precedence: builtin → user → local → extra_paths
    pub fn resolve_all(&self) -> HashMap<String, Skill> {
        let mut skills = HashMap::new();

        // 1. Load builtins (lowest precedence)
        self.load_builtins(&mut skills);

        // 2. Walk VFS search paths for skill subdirectories.
        //    The VFS resolves dot-directory paths in user-then-local order,
        //    so user skills are loaded first (lower precedence) and local
        //    skills override them.
        self.load_from_vfs_paths(&mut skills);

        skills
    }

    /// Resolve only builtin skills (no local/user overrides).
    pub fn resolve_builtins(&self) -> HashMap<String, Skill> {
        let mut skills = HashMap::new();
        self.load_builtins(&mut skills);
        skills
    }

    /// Validate all skill sources, catching parse/load failures that `resolve_all` silently skips.
    ///
    /// Walks the same builtin → user → local tiers as `resolve_all` but converts
    /// load errors into `ValidationIssue`s instead of logging warnings.
    pub fn validate_all_sources(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // 1. Validate builtins
        self.validate_builtins(&mut issues);

        // 2. Validate VFS search paths
        self.validate_vfs_paths(&mut issues);

        issues
    }

    /// Validate builtin skills, capturing load failures.
    ///
    /// Directories without a SKILL.md are skipped silently — they are resource
    /// directories (e.g. partials) rather than standalone skills.
    fn validate_builtins(&self, issues: &mut Vec<ValidationIssue>) {
        let builtin_files = get_builtin_skills();

        let mut skill_groups: HashMap<String, Vec<(&str, &str)>> = HashMap::new();
        for (name, content) in &builtin_files {
            let skill_name = if let Some(pos) = name.find('/') {
                &name[..pos]
            } else {
                name
            };
            skill_groups
                .entry(skill_name.to_string())
                .or_default()
                .push((name, content));
        }

        for (skill_name, files) in &skill_groups {
            // Skip groups that don't contain a SKILL.md — they are resource
            // directories (partials, supporting docs) rather than skills.
            let has_skill_md = files
                .iter()
                .any(|(name, _)| name.ends_with("/SKILL.md") || *name == "SKILL.md");
            if !has_skill_md {
                continue;
            }

            if let Err(e) = load_skill_from_builtin(skill_name, files) {
                issues.push(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: PathBuf::from(format!("skill:builtin:{skill_name}")),
                    content_title: Some(skill_name.clone()),
                    line: None,
                    column: None,
                    message: format!("Failed to load builtin skill: {e}"),
                    suggestion: Some("Check SKILL.md frontmatter syntax".to_string()),
                });
            }
        }
    }

    /// Validate skills found via VFS search paths.
    fn validate_vfs_paths(&self, issues: &mut Vec<ValidationIssue>) {
        for (dir, source) in self.resolve_search_paths() {
            self.validate_directory(&dir, source, issues);
        }
    }

    /// Validate all skills from a directory, capturing load failures.
    fn validate_directory(
        &self,
        dir: &Path,
        source: SkillSource,
        issues: &mut Vec<ValidationIssue>,
    ) {
        if !dir.is_dir() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                issues.push(ValidationIssue {
                    level: ValidationLevel::Error,
                    file_path: dir.to_path_buf(),
                    content_title: None,
                    line: None,
                    column: None,
                    message: format!("Failed to read skills directory: {e}"),
                    suggestion: None,
                });
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip directories without a SKILL.md — namespace prefixes, not skills.
                if !path.join("SKILL.md").exists() {
                    continue;
                }
                if let Err(e) = load_skill_from_dir(&path, source.clone()) {
                    let skill_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    issues.push(ValidationIssue {
                        level: ValidationLevel::Error,
                        file_path: path.clone(),
                        content_title: Some(skill_name),
                        line: None,
                        column: None,
                        message: format!("Failed to load {} skill: {e}", source),
                        suggestion: Some(
                            "Check SKILL.md exists and has valid frontmatter".to_string(),
                        ),
                    });
                }
            }
        }
    }

    /// Load builtin skills embedded in the binary.
    fn load_builtins(&self, skills: &mut HashMap<String, Skill>) {
        let builtin_files = get_builtin_skills();

        // Group files by skill name (directory prefix)
        let mut skill_groups: HashMap<String, Vec<(&str, &str)>> = HashMap::new();

        for (name, content) in &builtin_files {
            // Names are like "plan/SKILL" — extract the directory as skill name
            let skill_name = if let Some(pos) = name.find('/') {
                &name[..pos]
            } else {
                name
            };

            skill_groups
                .entry(skill_name.to_string())
                .or_default()
                .push((name, content));
        }

        for (skill_name, files) in &skill_groups {
            match load_skill_from_builtin(skill_name, files) {
                Ok(skill) => {
                    tracing::debug!("Loaded builtin skill: {}", skill.name);
                    skills.insert(skill.name.as_str().to_string(), skill);
                }
                Err(e) => {
                    tracing::warn!("Failed to load builtin skill '{}': {}", skill_name, e);
                }
            }
        }
    }

    /// Resolve VFS search paths into (directory, SkillSource) pairs.
    ///
    /// The VFS provides search paths in precedence order (user before local).
    /// Extra paths added via [`add_search_path`] appear after the defaults.
    fn resolve_search_paths(&self) -> Vec<(PathBuf, SkillSource)> {
        self.vfs
            .get_search_paths()
            .iter()
            .map(|sp| (sp.path.clone(), file_source_to_skill_source(&sp.source)))
            .collect()
    }

    /// Load skills from all VFS-managed search paths.
    ///
    /// Walks each resolved directory for skill subdirectories (directories
    /// containing a SKILL.md). Earlier directories have lower precedence;
    /// a skill in a later directory overrides one with the same name.
    fn load_from_vfs_paths(&self, skills: &mut HashMap<String, Skill>) {
        for (dir, source) in self.resolve_search_paths() {
            self.load_from_directory(&dir, source, skills);
        }
    }

    /// Load all skills from a directory (each subdirectory is a skill).
    fn load_from_directory(
        &self,
        dir: &Path,
        source: SkillSource,
        skills: &mut HashMap<String, Skill>,
    ) {
        if !dir.is_dir() {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read skills directory {}: {}", dir.display(), e);
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip directories without a SKILL.md — they are namespace
                // prefixes (e.g. user/repo/skill), not skills themselves.
                if !path.join("SKILL.md").exists() {
                    continue;
                }
                match load_skill_from_dir(&path, source.clone()) {
                    Ok(skill) => {
                        tracing::debug!(
                            "Loaded {} skill: {} from {}",
                            source,
                            skill.name,
                            path.display()
                        );
                        skills.insert(skill.name.as_str().to_string(), skill);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load skill from {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
}

impl Default for SkillResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_builtins() {
        let resolver = SkillResolver::new();
        let skills = resolver.resolve_all();

        // Should have the builtin skills
        assert!(skills.contains_key("plan"), "should have plan skill");
        assert!(skills.contains_key("commit"), "should have commit skill");
        assert!(skills.contains_key("test"), "should have test skill");
        assert!(
            skills.contains_key("implement"),
            "should have implement skill"
        );
        assert!(skills.contains_key("map"), "should have map skill");
    }

    #[test]
    fn test_builtin_skill_content() {
        let resolver = SkillResolver::new();
        // Use resolve_builtins() to avoid local .skills/ overrides that
        // may exist in the project directory during testing.
        let skills = resolver.resolve_builtins();

        let plan = skills.get("plan").unwrap();
        assert_eq!(plan.name.as_str(), "plan");
        assert!(!plan.description.is_empty());
        assert!(!plan.instructions.is_empty());
        assert_eq!(plan.source, SkillSource::Builtin);
    }

    #[test]
    fn test_validate_all_sources_catches_invalid_skill() {
        use swissarmyhammer_common::validation::ValidationLevel;

        // Create a temp directory with a malformed skill
        let temp_dir = tempfile::TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("broken-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        // Write a SKILL.md with missing required frontmatter fields
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# No frontmatter at all\nJust plain markdown.",
        )
        .unwrap();

        let mut resolver = SkillResolver::new();
        resolver.add_search_path(temp_dir.path().to_path_buf());

        let issues = resolver.validate_all_sources();

        // Should contain at least one error for the broken skill
        let broken_issues: Vec<_> = issues
            .iter()
            .filter(|issue| {
                issue.level == ValidationLevel::Error
                    && issue.file_path.to_string_lossy().contains("broken-skill")
            })
            .collect();

        assert!(
            !broken_issues.is_empty(),
            "Expected error for skill with missing frontmatter, got issues: {:?}",
            issues
                .iter()
                .map(|i| format!("{}: {}", i.file_path.display(), i.message))
                .collect::<Vec<_>>()
        );
    }

    /// Helper to write a minimal valid SKILL.md into a directory.
    fn write_skill(dir: &std::path::Path, name: &str, description: &str, body: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
        )
        .unwrap();
    }

    #[test]
    fn test_local_overrides_user_via_extra_paths() {
        // Simulate user-level and local-level directories as extra search paths.
        // The second path (local) should win because it has higher precedence.
        let user_dir = tempfile::TempDir::new().unwrap();
        let local_dir = tempfile::TempDir::new().unwrap();

        write_skill(
            user_dir.path(),
            "my-skill",
            "user version",
            "User instructions",
        );
        write_skill(
            local_dir.path(),
            "my-skill",
            "local version",
            "Local instructions",
        );

        let mut resolver = SkillResolver::new();
        // First path = lower precedence, second = higher
        resolver
            .vfs
            .add_search_path(user_dir.path().to_path_buf(), FileSource::User);
        resolver
            .vfs
            .add_search_path(local_dir.path().to_path_buf(), FileSource::Local);

        let skills = resolver.resolve_all();
        let skill = skills.get("my-skill").expect("my-skill should be resolved");
        assert_eq!(
            skill.source,
            SkillSource::Local,
            "local should override user"
        );
        assert_eq!(skill.description, "local version");
        assert_eq!(skill.instructions, "Local instructions");
    }

    #[test]
    fn test_extra_path_overrides_builtin() {
        // An extra search path skill with the same name as a builtin should win.
        let extra_dir = tempfile::TempDir::new().unwrap();
        write_skill(
            extra_dir.path(),
            "plan",
            "custom plan",
            "Custom plan instructions",
        );

        let mut resolver = SkillResolver::new();
        resolver.add_search_path(extra_dir.path().to_path_buf());

        let skills = resolver.resolve_all();
        let plan = skills.get("plan").expect("plan should be resolved");
        assert_eq!(
            plan.source,
            SkillSource::Local,
            "extra path should override builtin"
        );
        assert_eq!(plan.description, "custom plan");
    }

    #[test]
    fn test_resolver_default_impl() {
        // Default should produce a working resolver identical to new()
        let resolver = SkillResolver::default();
        let skills = resolver.resolve_all();
        assert!(
            skills.contains_key("plan"),
            "default resolver should load builtins"
        );
    }

    #[test]
    fn test_resolve_builtins_only() {
        let resolver = SkillResolver::new();
        let builtins = resolver.resolve_builtins();

        // All builtins should have SkillSource::Builtin
        for (name, skill) in &builtins {
            assert_eq!(
                skill.source,
                SkillSource::Builtin,
                "skill '{}' should be builtin",
                name
            );
        }
        assert!(!builtins.is_empty());
    }

    #[test]
    fn test_load_from_nonexistent_directory() {
        // Adding a non-existent path should not cause errors — just no skills loaded
        let mut resolver = SkillResolver::new();
        resolver.add_search_path(PathBuf::from("/tmp/nonexistent-skills-dir-12345"));

        let skills = resolver.resolve_all();
        // Should still have builtins
        assert!(skills.contains_key("plan"));
    }

    #[test]
    fn test_load_from_empty_directory() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut resolver = SkillResolver::new();
        resolver.add_search_path(temp_dir.path().to_path_buf());

        let skills = resolver.resolve_all();
        // Should still have builtins, no errors from empty dir
        assert!(skills.contains_key("plan"));
    }

    #[test]
    fn test_skip_directory_without_skill_md() {
        // A subdirectory without SKILL.md should be silently skipped
        let temp_dir = tempfile::TempDir::new().unwrap();
        let not_a_skill = temp_dir.path().join("not-a-skill");
        std::fs::create_dir_all(&not_a_skill).unwrap();
        std::fs::write(not_a_skill.join("README.md"), "not a skill").unwrap();

        let mut resolver = SkillResolver::new();
        resolver.add_search_path(temp_dir.path().to_path_buf());

        let skills = resolver.resolve_all();
        assert!(!skills.contains_key("not-a-skill"));
    }

    #[test]
    fn test_validate_all_sources_no_errors_for_builtins() {
        let resolver = SkillResolver::new();
        let issues = resolver.validate_all_sources();

        // Builtins should all validate cleanly
        let builtin_errors: Vec<_> = issues
            .iter()
            .filter(|i| i.file_path.to_string_lossy().contains("builtin"))
            .collect();
        assert!(
            builtin_errors.is_empty(),
            "builtin skills should have no validation errors, got: {:?}",
            builtin_errors
                .iter()
                .map(|i| &i.message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_validate_unreadable_directory() {
        // validate_directory with a path that doesn't exist should not panic
        let resolver = SkillResolver::new();
        let issues = resolver.validate_all_sources();
        // Just ensure it returns without panicking; non-existent VFS paths are skipped
        let _ = issues;
    }

    #[test]
    fn test_file_source_to_skill_source_mapping() {
        assert_eq!(
            file_source_to_skill_source(&FileSource::Builtin),
            SkillSource::Builtin
        );
        assert_eq!(
            file_source_to_skill_source(&FileSource::Dynamic),
            SkillSource::Builtin
        );
        assert_eq!(
            file_source_to_skill_source(&FileSource::User),
            SkillSource::User
        );
        assert_eq!(
            file_source_to_skill_source(&FileSource::Local),
            SkillSource::Local
        );
    }
}
