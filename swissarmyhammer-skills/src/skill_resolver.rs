//! Skill resolver — discovers skills from builtin → local → user sources
//!
//! Precedence: builtin < local < user (later sources override earlier ones)

use crate::skill::{Skill, SkillSource};
use crate::skill_loader::{load_skill_from_builtin, load_skill_from_dir};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::validation::{ValidationIssue, ValidationLevel};

// Include the generated builtin skills
include!(concat!(env!("OUT_DIR"), "/builtin_skills.rs"));

/// Resolves skills from all sources with proper override precedence
pub struct SkillResolver {
    /// Additional search paths beyond defaults
    extra_paths: Vec<PathBuf>,
}

impl SkillResolver {
    pub fn new() -> Self {
        Self {
            extra_paths: Vec::new(),
        }
    }

    /// Add an extra search path for skills
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.extra_paths.push(path);
    }

    /// Resolve all skills from all sources
    ///
    /// Returns skills keyed by name. Later sources override earlier ones.
    /// Precedence: builtin → local → user
    pub fn resolve_all(&self) -> HashMap<String, Skill> {
        let mut skills = HashMap::new();

        // 1. Load builtins (lowest precedence)
        self.load_builtins(&mut skills);

        // 2. Load from local project paths
        self.load_from_local_paths(&mut skills);

        // 3. Load from user-level paths
        self.load_from_user_paths(&mut skills);

        // 4. Load from extra paths
        for path in &self.extra_paths {
            self.load_from_directory(path, SkillSource::Local, &mut skills);
        }

        skills
    }

    /// Resolve only builtin skills (no local/user overrides)
    pub fn resolve_builtins(&self) -> HashMap<String, Skill> {
        let mut skills = HashMap::new();
        self.load_builtins(&mut skills);
        skills
    }

    /// Validate all skill sources, catching parse/load failures that `resolve_all` silently skips.
    ///
    /// Walks the same builtin → local → user tiers as `resolve_all` but converts
    /// load errors into `ValidationIssue`s instead of logging warnings.
    pub fn validate_all_sources(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // 1. Validate builtins
        self.validate_builtins(&mut issues);

        // 2. Validate local project paths
        self.validate_local_paths(&mut issues);

        // 3. Validate user-level paths
        self.validate_user_paths(&mut issues);

        // 4. Validate extra paths
        for path in &self.extra_paths {
            self.validate_directory(path, SkillSource::Local, &mut issues);
        }

        issues
    }

    /// Validate builtin skills, capturing load failures
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

    /// Validate skills from project-local paths
    fn validate_local_paths(&self, issues: &mut Vec<ValidationIssue>) {
        let cwd = std::env::current_dir().unwrap_or_default();

        let dot_skills = cwd.join(".skills");
        self.validate_directory(&dot_skills, SkillSource::Local, issues);

        let sah_skills = cwd.join(".swissarmyhammer").join("skills");
        self.validate_directory(&sah_skills, SkillSource::Local, issues);
    }

    /// Validate skills from user-level paths
    fn validate_user_paths(&self, issues: &mut Vec<ValidationIssue>) {
        if let Some(home) = dirs::home_dir() {
            let user_skills = home.join(".skills");
            self.validate_directory(&user_skills, SkillSource::User, issues);

            let user_sah_skills = home.join(".swissarmyhammer").join("skills");
            self.validate_directory(&user_sah_skills, SkillSource::User, issues);
        }
    }

    /// Validate all skills from a directory, capturing load failures
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
                if let Err(e) = load_skill_from_dir(&path, source.clone()) {
                    let skill_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    let source_label = match source {
                        SkillSource::Builtin => "builtin",
                        SkillSource::Local => "local",
                        SkillSource::User => "user",
                    };

                    issues.push(ValidationIssue {
                        level: ValidationLevel::Error,
                        file_path: path.clone(),
                        content_title: Some(skill_name),
                        line: None,
                        column: None,
                        message: format!("Failed to load {source_label} skill: {e}"),
                        suggestion: Some(
                            "Check SKILL.md exists and has valid frontmatter".to_string(),
                        ),
                    });
                }
            }
        }
    }

    /// Load builtin skills embedded in the binary
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

    /// Load skills from project-local paths
    fn load_from_local_paths(&self, skills: &mut HashMap<String, Skill>) {
        let cwd = std::env::current_dir().unwrap_or_default();

        // Try .skills/ in project root
        let dot_skills = cwd.join(".skills");
        self.load_from_directory(&dot_skills, SkillSource::Local, skills);

        // Try .swissarmyhammer/skills/ in project root
        let sah_skills = cwd.join(".swissarmyhammer").join("skills");
        self.load_from_directory(&sah_skills, SkillSource::Local, skills);
    }

    /// Load skills from user-level paths
    fn load_from_user_paths(&self, skills: &mut HashMap<String, Skill>) {
        if let Some(home) = dirs::home_dir() {
            // Try ~/.skills/
            let user_skills = home.join(".skills");
            self.load_from_directory(&user_skills, SkillSource::User, skills);

            // Try ~/.swissarmyhammer/skills/
            let user_sah_skills = home.join(".swissarmyhammer").join("skills");
            self.load_from_directory(&user_sah_skills, SkillSource::User, skills);
        }
    }

    /// Load all skills from a directory (each subdirectory is a skill)
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
        assert!(skills.contains_key("kanban"), "should have kanban skill");
        assert!(skills.contains_key("commit"), "should have commit skill");
        assert!(skills.contains_key("test"), "should have test skill");
        assert!(
            skills.contains_key("implement"),
            "should have implement skill"
        );
    }

    #[test]
    fn test_builtin_skill_content() {
        let resolver = SkillResolver::new();
        let skills = resolver.resolve_all();

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
}
