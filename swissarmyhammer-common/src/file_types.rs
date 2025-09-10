//! File type detection and extension handling utilities
//!
//! This module provides consistent file extension checking and type detection
//! patterns used throughout the codebase.

use std::path::Path;

/// Common prompt file extensions
pub const PROMPT_EXTENSIONS: &[&str] = &["md", "yaml", "yml", "markdown"];

/// Compound prompt file extensions (checked first due to specificity)
pub const COMPOUND_PROMPT_EXTENSIONS: &[&str] =
    &["md.liquid", "markdown.liquid", "yaml.liquid", "yml.liquid"];

/// All supported prompt extensions (compound first, then simple)
pub fn all_prompt_extensions() -> Vec<&'static str> {
    let mut extensions = COMPOUND_PROMPT_EXTENSIONS.to_vec();
    extensions.extend_from_slice(PROMPT_EXTENSIONS);
    extensions
}

/// Check if a file has a prompt extension
pub fn is_prompt_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        PROMPT_EXTENSIONS.contains(&ext_str.as_str())
    } else {
        false
    }
}

/// Check if a file has a compound prompt extension (e.g., .md.liquid)
pub fn has_compound_extension<P: AsRef<Path>>(path: P) -> bool {
    let path_str = path.as_ref().to_string_lossy().to_lowercase();
    COMPOUND_PROMPT_EXTENSIONS
        .iter()
        .any(|ext| path_str.ends_with(&format!(".{ext}")))
}

/// Check if a file is any kind of prompt file (simple or compound extension)
pub fn is_any_prompt_file<P: AsRef<Path>>(path: P) -> bool {
    has_compound_extension(&path) || is_prompt_file(path)
}

/// Extract the base name from a path, removing all supported extensions
pub fn extract_base_name<P: AsRef<Path>>(path: P) -> String {
    let path = path.as_ref();
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    // Try compound extensions first (they're more specific)
    for ext in COMPOUND_PROMPT_EXTENSIONS {
        let extension = format!(".{ext}");
        if filename.ends_with(&extension) {
            return filename[..filename.len() - extension.len()].to_string();
        }
    }

    // Try simple extensions
    for ext in PROMPT_EXTENSIONS {
        let extension = format!(".{ext}");
        if filename.ends_with(&extension) {
            return filename[..filename.len() - extension.len()].to_string();
        }
    }

    // No recognized extension, return full filename
    filename.to_string()
}

/// Get the extension (compound or simple) of a prompt file
pub fn get_prompt_extension<P: AsRef<Path>>(path: P) -> Option<String> {
    let path = path.as_ref();
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())?
        .to_lowercase();

    // Check compound extensions first
    for ext in COMPOUND_PROMPT_EXTENSIONS {
        let extension = format!(".{ext}");
        if filename.ends_with(&extension) {
            return Some(ext.to_string());
        }
    }

    // Check simple extensions
    for ext in PROMPT_EXTENSIONS {
        let extension = format!(".{ext}");
        if filename.ends_with(&extension) {
            return Some(ext.to_string());
        }
    }

    None
}

/// File extension matcher using pattern matching for efficiency
pub struct ExtensionMatcher {
    extensions: Vec<String>,
}

impl ExtensionMatcher {
    /// Create a new matcher for the given extensions
    pub fn new(extensions: &[&str]) -> Self {
        Self {
            extensions: extensions.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    /// Create a matcher for prompt files
    pub fn for_prompts() -> Self {
        Self::new(&all_prompt_extensions())
    }

    /// Check if a path matches any of the configured extensions
    pub fn matches<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_str = path.as_ref().to_string_lossy().to_lowercase();

        // Check compound extensions first (more specific)
        for ext in &self.extensions {
            if ext.contains('.') && path_str.ends_with(&format!(".{ext}")) {
                return true;
            }
        }

        // Check simple extensions
        if let Some(ext) = path.as_ref().extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            return self
                .extensions
                .iter()
                .any(|e| !e.contains('.') && e == &ext_str);
        }

        false
    }

    /// Get all matching files from a directory
    pub fn filter_files<P: AsRef<Path>>(&self, paths: Vec<P>) -> Vec<P> {
        paths.into_iter().filter(|p| self.matches(p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_prompt_file() {
        assert!(is_prompt_file(Path::new("test.md")));
        assert!(is_prompt_file(Path::new("test.yaml")));
        assert!(is_prompt_file(Path::new("test.yml")));
        assert!(is_prompt_file(Path::new("test.markdown")));
        assert!(!is_prompt_file(Path::new("test.txt")));
        assert!(!is_prompt_file(Path::new("test")));
        assert!(!is_prompt_file(Path::new("test.md.liquid"))); // This should be compound
    }

    #[test]
    fn test_has_compound_extension() {
        assert!(has_compound_extension(Path::new("test.md.liquid")));
        assert!(has_compound_extension(Path::new("test.markdown.liquid")));
        assert!(has_compound_extension(Path::new("test.yaml.liquid")));
        assert!(has_compound_extension(Path::new("test.yml.liquid")));
        assert!(!has_compound_extension(Path::new("test.md")));
        assert!(!has_compound_extension(Path::new("test.txt")));
    }

    #[test]
    fn test_is_any_prompt_file() {
        assert!(is_any_prompt_file(Path::new("test.md")));
        assert!(is_any_prompt_file(Path::new("test.md.liquid")));
        assert!(is_any_prompt_file(Path::new("test.yaml")));
        assert!(is_any_prompt_file(Path::new("test.yaml.liquid")));
        assert!(!is_any_prompt_file(Path::new("test.txt")));
        assert!(!is_any_prompt_file(Path::new("test")));
    }

    #[test]
    fn test_extract_base_name() {
        assert_eq!(extract_base_name(Path::new("test.md")), "test");
        assert_eq!(extract_base_name(Path::new("test.md.liquid")), "test");
        assert_eq!(
            extract_base_name(Path::new("complex-name.yaml.liquid")),
            "complex-name"
        );
        assert_eq!(
            extract_base_name(Path::new("config.prod.yml")),
            "config.prod"
        );
        assert_eq!(extract_base_name(Path::new("README")), "README"); // No extension
    }

    #[test]
    fn test_get_prompt_extension() {
        assert_eq!(
            get_prompt_extension(Path::new("test.md")),
            Some("md".to_string())
        );
        assert_eq!(
            get_prompt_extension(Path::new("test.md.liquid")),
            Some("md.liquid".to_string())
        );
        assert_eq!(
            get_prompt_extension(Path::new("test.yaml.liquid")),
            Some("yaml.liquid".to_string())
        );
        assert_eq!(get_prompt_extension(Path::new("test.txt")), None);
        assert_eq!(get_prompt_extension(Path::new("README")), None);
    }

    #[test]
    fn test_extension_matcher() {
        let matcher = ExtensionMatcher::new(&["md", "yaml", "txt"]);

        assert!(matcher.matches(Path::new("test.md")));
        assert!(matcher.matches(Path::new("test.yaml")));
        assert!(matcher.matches(Path::new("test.txt")));
        assert!(!matcher.matches(Path::new("test.rs")));
    }

    #[test]
    fn test_extension_matcher_for_prompts() {
        let matcher = ExtensionMatcher::for_prompts();

        assert!(matcher.matches(Path::new("test.md")));
        assert!(matcher.matches(Path::new("test.md.liquid")));
        assert!(matcher.matches(Path::new("test.yaml")));
        assert!(matcher.matches(Path::new("test.yaml.liquid")));
        assert!(!matcher.matches(Path::new("test.txt")));
        assert!(!matcher.matches(Path::new("test.rs")));
    }

    #[test]
    fn test_filter_files() {
        let matcher = ExtensionMatcher::new(&["md", "yaml"]);
        let paths = vec![
            PathBuf::from("test.md"),
            PathBuf::from("test.yaml"),
            PathBuf::from("test.txt"),
            PathBuf::from("README"),
        ];

        let filtered = matcher.filter_files(paths);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|p| p.file_name().unwrap() == "test.md"));
        assert!(filtered
            .iter()
            .any(|p| p.file_name().unwrap() == "test.yaml"));
    }

    #[test]
    fn test_case_sensitivity() {
        // Extensions should be case-insensitive when matching (our current implementation)
        assert!(is_prompt_file(Path::new("file.MD")));
        assert!(is_prompt_file(Path::new("file.YAML")));

        // The matcher should also handle case insensitivity
        let matcher = ExtensionMatcher::new(&["md", "yaml"]);
        assert!(matcher.matches(Path::new("file.MD")));
        assert!(matcher.matches(Path::new("file.YAML")));
    }

    #[test]
    fn test_hidden_files() {
        assert!(is_prompt_file(Path::new(".test.md")));
        assert!(has_compound_extension(Path::new(".config.yaml.liquid")));
        assert!(is_any_prompt_file(Path::new(".hidden.yml")));
    }

    #[test]
    fn test_multiple_dots_in_filename() {
        assert_eq!(extract_base_name(Path::new("file.test.md")), "file.test");
        assert_eq!(
            extract_base_name(Path::new("config.prod.yaml.liquid")),
            "config.prod"
        );
        assert!(is_any_prompt_file(Path::new("my.config.file.yml")));
    }
}
