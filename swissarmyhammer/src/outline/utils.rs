//! Utility functions for outline generation

use std::path::{Path, PathBuf};

/// Parse a glob pattern to extract base directory and file pattern
pub fn parse_glob_pattern(pattern: &str) -> (PathBuf, String) {
    let path = Path::new(pattern);

    // Find the first component with glob characters
    let mut base_components = Vec::new();
    let mut pattern_components = Vec::new();
    let mut found_glob = false;

    for component in path.components() {
        let component_str = component.as_os_str().to_string_lossy();
        if !found_glob
            && !component_str.contains('*')
            && !component_str.contains('?')
            && !component_str.contains('[')
        {
            base_components.push(component);
        } else {
            found_glob = true;
            pattern_components.push(component_str.to_string());
        }
    }

    let base_dir = if base_components.is_empty() {
        PathBuf::from(".")
    } else {
        base_components.iter().collect()
    };

    let file_pattern = if pattern_components.is_empty() {
        "*".to_string()
    } else {
        pattern_components.join("/")
    };

    (base_dir, file_pattern)
}

/// Check if a file path matches a glob pattern
pub fn matches_glob_pattern(path: &Path, pattern: &str) -> Result<bool, glob::PatternError> {
    let glob_pattern = glob::Pattern::new(pattern)?;
    let path_str = path.to_string_lossy();
    Ok(glob_pattern.matches(&path_str))
}

/// Get the relative path from a base directory
pub fn get_relative_path(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

/// Check if a path is likely a temporary or generated file
pub fn is_likely_generated_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // Common generated file patterns
    path_str.contains("target/")
        || path_str.contains("node_modules/")
        || path_str.contains("dist/")
        || path_str.contains("build/")
        || path_str.contains(".git/")
        || path_str.contains("coverage/")
        || path_str.ends_with(".tmp")
        || path_str.ends_with(".temp")
        || path_str.ends_with(".bak")
        || path_str.ends_with(".swp")
        || path_str.ends_with("~")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_glob_pattern() {
        // Test simple patterns
        let (base, pattern) = parse_glob_pattern("*.rs");
        assert_eq!(base, PathBuf::from("."));
        assert_eq!(pattern, "*.rs");

        // Test directory with pattern
        let (base, pattern) = parse_glob_pattern("src/*.rs");
        assert_eq!(base, PathBuf::from("src"));
        assert_eq!(pattern, "*.rs");

        // Test nested directory with pattern
        let (base, pattern) = parse_glob_pattern("src/main/**/*.rs");
        assert_eq!(base, PathBuf::from("src/main"));
        assert_eq!(pattern, "**/*.rs");

        // Test absolute path
        let (base, pattern) = parse_glob_pattern("/usr/local/*.rs");
        assert_eq!(base, PathBuf::from("/usr/local"));
        assert_eq!(pattern, "*.rs");
    }

    #[test]
    fn test_matches_glob_pattern() {
        // Test simple patterns
        assert!(matches_glob_pattern(Path::new("test.rs"), "*.rs").unwrap());
        assert!(!matches_glob_pattern(Path::new("test.py"), "*.rs").unwrap());

        // Test directory patterns
        assert!(matches_glob_pattern(Path::new("src/main.rs"), "src/*.rs").unwrap());
        assert!(!matches_glob_pattern(Path::new("lib/main.rs"), "src/*.rs").unwrap());

        // Test recursive patterns
        assert!(matches_glob_pattern(Path::new("src/deep/main.rs"), "**/*.rs").unwrap());
        assert!(matches_glob_pattern(Path::new("main.rs"), "**/*.rs").unwrap());
    }

    #[test]
    fn test_get_relative_path() {
        let base = Path::new("/home/user/project");
        let file = Path::new("/home/user/project/src/main.rs");
        assert_eq!(get_relative_path(file, base), "src/main.rs");

        // Test when path is not under base
        let other = Path::new("/other/file.rs");
        assert_eq!(get_relative_path(other, base), "/other/file.rs");
    }

    #[test]  
    fn test_is_likely_generated_file() {
        assert!(is_likely_generated_file(Path::new("target/debug/main")));
        assert!(is_likely_generated_file(Path::new("node_modules/lib/index.js")));
        assert!(is_likely_generated_file(Path::new("dist/bundle.js")));
        assert!(is_likely_generated_file(Path::new("build/output.o")));
        assert!(is_likely_generated_file(Path::new(".git/config")));
        assert!(is_likely_generated_file(Path::new("temp.tmp")));
        assert!(is_likely_generated_file(Path::new("backup.bak")));
        assert!(is_likely_generated_file(Path::new("file.swp")));
        assert!(is_likely_generated_file(Path::new("file~")));

        assert!(!is_likely_generated_file(Path::new("src/main.rs")));
        assert!(!is_likely_generated_file(Path::new("README.md")));
        assert!(!is_likely_generated_file(Path::new("Cargo.toml")));
    }
}