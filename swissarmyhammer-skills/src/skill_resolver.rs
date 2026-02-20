//! Skill resolver — discovers skills from builtin → local → user sources
//!
//! Precedence: builtin < local < user (later sources override earlier ones)

use crate::skill::{Skill, SkillSource};
use crate::skill_loader::{load_skill_from_builtin, load_skill_from_dir};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
                        tracing::warn!(
                            "Failed to load skill from {}: {}",
                            path.display(),
                            e
                        );
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
        assert!(!plan.allowed_tools.is_empty());
        assert_eq!(plan.source, SkillSource::Builtin);
    }
}
