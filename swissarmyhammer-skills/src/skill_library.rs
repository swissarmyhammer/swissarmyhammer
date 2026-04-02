//! SkillLibrary — stores and retrieves resolved skills

use crate::skill::Skill;
use crate::skill_resolver::SkillResolver;
use std::collections::HashMap;

/// In-memory skill library populated by the resolver
pub struct SkillLibrary {
    skills: HashMap<String, Skill>,
}

impl SkillLibrary {
    /// Create a new empty library
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Load all skills using the default resolver
    pub fn load_defaults(&mut self) {
        let resolver = SkillResolver::new();
        self.skills = resolver.resolve_all();
        tracing::debug!("SkillLibrary loaded {} skills", self.skills.len());
    }

    /// Load skills using a custom resolver
    pub fn load_with_resolver(&mut self, resolver: &SkillResolver) {
        self.skills = resolver.resolve_all();
    }

    /// Get a skill by name
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// List all available skills
    pub fn list(&self) -> Vec<&Skill> {
        let mut skills: Vec<_> = self.skills.values().collect();
        skills.sort_by_key(|s| s.name.as_str());
        skills
    }

    /// Get the number of loaded skills
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if the library is empty
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Get all skill names
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.skills.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

impl Default for SkillLibrary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_load_defaults() {
        let mut library = SkillLibrary::new();
        library.load_defaults();

        assert!(!library.is_empty());
        assert!(library.get("plan").is_some());
        assert!(library.get("nonexistent").is_none());
    }

    #[test]
    fn test_library_list() {
        let mut library = SkillLibrary::new();
        library.load_defaults();

        let skills = library.list();
        assert!(!skills.is_empty());

        // Should be sorted
        let names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);
    }

    #[test]
    fn test_library_load_with_resolver() {
        let resolver = SkillResolver::new();
        let mut library = SkillLibrary::new();
        assert!(library.is_empty());

        library.load_with_resolver(&resolver);
        assert!(!library.is_empty());
        // Should have the same skills as load_defaults
        assert!(library.get("plan").is_some());
    }

    #[test]
    fn test_library_len() {
        let mut library = SkillLibrary::new();
        assert_eq!(library.len(), 0);

        library.load_defaults();
        assert!(library.len() > 0);
    }

    #[test]
    fn test_library_names() {
        let mut library = SkillLibrary::new();
        library.load_defaults();

        let names = library.names();
        assert!(!names.is_empty());

        // Should be sorted
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);

        // Should contain known builtins
        assert!(names.contains(&"plan"));
        assert!(names.contains(&"commit"));
    }

    #[test]
    fn test_library_default_is_empty() {
        let library = SkillLibrary::default();
        assert!(library.is_empty());
        assert_eq!(library.len(), 0);
        assert!(library.names().is_empty());
    }

    #[test]
    fn test_library_new_is_empty() {
        let library = SkillLibrary::new();
        assert!(library.is_empty());
        assert!(library.list().is_empty());
        assert!(library.get("anything").is_none());
    }
}
