//! File discovery functionality for outline generation

use crate::outline::{
    types::{DiscoveredFile, FileDiscoveryConfig, FileDiscoveryReport},
    utils::{
        get_relative_path, is_likely_generated_file, matches_glob_pattern, parse_glob_pattern,
    },
    OutlineError, Result,
};
use swissarmyhammer_search::{Language, LanguageRegistry};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// File discovery engine for outline generation
#[derive(Debug)]
pub struct FileDiscovery {
    /// Glob patterns to match files against
    patterns: Vec<String>,
    /// Configuration for file discovery
    config: FileDiscoveryConfig,
    /// Language registry for file type detection
    language_registry: LanguageRegistry,
}

impl FileDiscovery {
    /// Create a new FileDiscovery instance with default configuration
    pub fn new(patterns: Vec<String>) -> Result<Self> {
        Self::with_config(patterns, FileDiscoveryConfig::new())
    }

    /// Create a new FileDiscovery instance with custom configuration
    pub fn with_config(patterns: Vec<String>, config: FileDiscoveryConfig) -> Result<Self> {
        if patterns.is_empty() {
            return Err(OutlineError::FileDiscovery(
                "At least one pattern must be provided".to_string(),
            ));
        }

        // Validate all patterns
        for pattern in &patterns {
            Self::validate_glob_pattern(pattern)?;
        }

        Ok(Self {
            patterns,
            config,
            language_registry: LanguageRegistry::with_defaults(),
        })
    }

    /// Validate a single glob pattern
    fn validate_glob_pattern(pattern: &str) -> Result<()> {
        if pattern.is_empty() {
            return Err(OutlineError::InvalidGlobPattern {
                pattern: pattern.to_string(),
                message: "Pattern cannot be empty".to_string(),
            });
        }

        // Try to compile the pattern to check if it's valid
        if let Err(e) = glob::Pattern::new(pattern) {
            return Err(OutlineError::InvalidGlobPattern {
                pattern: pattern.to_string(),
                message: e.to_string(),
            });
        }

        Ok(())
    }

    /// Discover files matching the configured patterns
    pub fn discover_files(&self) -> Result<(Vec<DiscoveredFile>, FileDiscoveryReport)> {
        let start_time = Instant::now();
        let mut discovered_files = Vec::new();
        let mut report = FileDiscoveryReport::new();

        tracing::info!("Starting file discovery with patterns: {:?}", self.patterns);

        for pattern in &self.patterns {
            let pattern_files = self.discover_files_for_pattern(pattern, &mut report)?;
            discovered_files.extend(pattern_files);
        }

        // Remove duplicates (same file might match multiple patterns)
        discovered_files.sort_by(|a, b| a.path.cmp(&b.path));
        discovered_files.dedup_by(|a, b| a.path == b.path);

        report.duration = start_time.elapsed();
        tracing::info!("File discovery completed: {}", report.summary());

        Ok((discovered_files, report))
    }

    /// Discover files for a single glob pattern
    fn discover_files_for_pattern(
        &self,
        pattern: &str,
        report: &mut FileDiscoveryReport,
    ) -> Result<Vec<DiscoveredFile>> {
        let mut files = Vec::new();

        // Parse the pattern to extract base directory and file pattern
        let (base_dir, file_pattern) = parse_glob_pattern(pattern);

        tracing::debug!(
            "Processing pattern '{}' -> base: '{}', file_pattern: '{}'",
            pattern,
            base_dir.display(),
            file_pattern
        );

        // Check if base directory exists
        if !base_dir.exists() {
            tracing::warn!("Base directory does not exist: {}", base_dir.display());
            return Ok(files);
        }

        // Create a walker that respects configuration
        let mut walker_builder = WalkBuilder::new(&base_dir);
        walker_builder
            .git_ignore(self.config.respect_gitignore)
            .git_global(self.config.respect_gitignore)
            .git_exclude(self.config.respect_gitignore)
            .hidden(!self.config.include_hidden)
            .parents(true);

        if let Some(max_depth) = self.config.max_depth {
            walker_builder.max_depth(Some(max_depth));
        }

        let walker = walker_builder.build();

        // Walk the directory structure and collect matching files
        for entry in walker {
            match entry {
                Ok(dir_entry) => {
                    let path = dir_entry.path();
                    if path.is_file() {
                        self.process_file(path, &base_dir, &file_pattern, &mut files, report)?;
                    }
                }
                Err(e) => {
                    tracing::warn!("Error processing directory entry: {}", e);
                    report.add_error(PathBuf::new(), e.to_string());
                }
            }
        }

        Ok(files)
    }

    /// Process a single file for inclusion in the discovery results
    fn process_file(
        &self,
        path: &Path,
        base_dir: &Path,
        file_pattern: &str,
        files: &mut Vec<DiscoveredFile>,
        report: &mut FileDiscoveryReport,
    ) -> Result<()> {
        // Check if the file matches the glob pattern
        let relative_path = get_relative_path(path, base_dir);
        if !matches_glob_pattern(Path::new(&relative_path), file_pattern).map_err(|e| {
            OutlineError::InvalidGlobPattern {
                pattern: file_pattern.to_string(),
                message: e.to_string(),
            }
        })? {
            return Ok(());
        }

        // Skip likely generated files even if not in gitignore
        if is_likely_generated_file(path) {
            tracing::debug!("Skipping likely generated file: {}", path.display());
            report.add_skipped_ignored(path);
            return Ok(());
        }

        // Get file metadata
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(e) => {
                tracing::warn!("Failed to read metadata for {}: {}", path.display(), e);
                report.add_error(path.to_path_buf(), e.to_string());
                return Ok(());
            }
        };

        let file_size = metadata.len();

        // Check file size limits
        if let Some(max_size) = self.config.max_file_size {
            if file_size > max_size {
                tracing::debug!(
                    "Skipping large file: {} ({} bytes > {} bytes)",
                    path.display(),
                    file_size,
                    max_size
                );
                report.add_skipped_size(path, file_size);
                return Ok(());
            }
        }

        // Detect language
        let language = self.language_registry.detect_language(path);

        // Only include supported languages unless we're including all files
        if matches!(language, Language::Unknown) {
            tracing::debug!("Skipping unsupported file type: {}", path.display());
        }

        // Create discovered file
        let discovered_file = DiscoveredFile::new(
            path.to_path_buf(),
            language.clone(),
            relative_path,
            file_size,
        );

        report.add_file(&discovered_file);
        files.push(discovered_file);

        tracing::debug!(
            "Discovered file: {} ({:?}, {} bytes)",
            path.display(),
            language,
            file_size
        );

        Ok(())
    }

    /// Get supported file patterns based on language registry
    pub fn supported_patterns() -> Vec<String> {
        vec![
            "**/*.rs".to_string(),   // Rust
            "**/*.py".to_string(),   // Python
            "**/*.ts".to_string(),   // TypeScript
            "**/*.js".to_string(),   // JavaScript
            "**/*.dart".to_string(), // Dart
        ]
    }

    /// Filter discovered files to only include supported languages
    pub fn filter_supported_files(files: Vec<DiscoveredFile>) -> Vec<DiscoveredFile> {
        files
            .into_iter()
            .filter(|file| file.is_supported())
            .collect()
    }

    /// Get statistics about discovered files grouped by language
    pub fn get_language_statistics(
        files: &[DiscoveredFile],
    ) -> std::collections::HashMap<Language, usize> {
        let mut stats = std::collections::HashMap::new();
        for file in files {
            *stats.entry(file.language.clone()).or_insert(0) += 1;
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(temp_dir: &TempDir) -> Result<()> {
        // Create various test files
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;
        fs::write(temp_dir.path().join("lib.py"), "def hello(): pass")?;
        fs::write(temp_dir.path().join("app.ts"), "console.log('hello');")?;
        fs::write(temp_dir.path().join("script.js"), "alert('test');")?;
        fs::write(temp_dir.path().join("widget.dart"), "void main() {}")?;
        fs::write(temp_dir.path().join("README.md"), "# Test")?;
        fs::write(temp_dir.path().join("data.json"), "{}")?;

        // Create subdirectory with files
        let subdir = temp_dir.path().join("src");
        fs::create_dir_all(&subdir)?;
        fs::write(subdir.join("module.rs"), "pub fn test() {}")?;
        fs::write(subdir.join("utils.py"), "import os")?;

        Ok(())
    }

    #[test]
    fn test_file_discovery_creation() {
        let patterns = vec!["**/*.rs".to_string()];
        let discovery = FileDiscovery::new(patterns);
        assert!(discovery.is_ok());
    }

    #[test]
    fn test_empty_patterns_error() {
        let patterns = vec![];
        let discovery = FileDiscovery::new(patterns);
        assert!(discovery.is_err());
        assert!(discovery
            .unwrap_err()
            .to_string()
            .contains("At least one pattern"));
    }

    #[test]
    fn test_invalid_pattern_error() {
        let patterns = vec!["[invalid".to_string()];
        let discovery = FileDiscovery::new(patterns);
        assert!(discovery.is_err());
    }

    #[test]
    fn test_pattern_validation() {
        assert!(FileDiscovery::validate_glob_pattern("**/*.rs").is_ok());
        assert!(FileDiscovery::validate_glob_pattern("src/**/*.py").is_ok());
        assert!(FileDiscovery::validate_glob_pattern("*.{ts,js}").is_ok());

        assert!(FileDiscovery::validate_glob_pattern("").is_err());
        assert!(FileDiscovery::validate_glob_pattern("[invalid").is_err());
    }

    #[test]
    fn test_discover_files() -> Result<()> {
        let temp_dir = TempDir::new().map_err(OutlineError::FileSystem)?;
        create_test_files(&temp_dir)?;

        let pattern = format!("{}/**/*.rs", temp_dir.path().display());
        let discovery = FileDiscovery::new(vec![pattern])?;

        let (files, report) = discovery.discover_files()?;

        // Should find main.rs and src/module.rs
        assert_eq!(files.len(), 2);
        assert!(files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "main.rs"));
        assert!(files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "module.rs"));

        // All files should be Rust language
        assert!(files.iter().all(|f| f.language == Language::Rust));

        // Check report
        assert_eq!(report.files_discovered, 2);
        assert_eq!(report.supported_files, 2);
        assert_eq!(report.errors.len(), 0);

        Ok(())
    }

    #[test]
    fn test_multiple_patterns() -> Result<()> {
        let temp_dir = TempDir::new().map_err(OutlineError::FileSystem)?;
        create_test_files(&temp_dir)?;

        let patterns = vec![
            format!("{}/**/*.rs", temp_dir.path().display()),
            format!("{}/**/*.py", temp_dir.path().display()),
        ];
        let discovery = FileDiscovery::new(patterns)?;

        let (files, report) = discovery.discover_files()?;

        // Should find Rust and Python files
        assert!(files.len() >= 4); // main.rs, module.rs, lib.py, utils.py
        assert!(files.iter().any(|f| f.language == Language::Rust));
        assert!(files.iter().any(|f| f.language == Language::Python));

        assert!(report.files_discovered >= 4);
        Ok(())
    }

    #[test]
    fn test_file_size_limits() -> Result<()> {
        let temp_dir = TempDir::new().map_err(OutlineError::FileSystem)?;

        // Create a large file
        let large_content = "x".repeat(1000);
        fs::write(temp_dir.path().join("large.rs"), large_content)?;

        let pattern = format!("{}/*.rs", temp_dir.path().display());
        let config = FileDiscoveryConfig::new().with_max_file_size(500); // 500 bytes limit
        let discovery = FileDiscovery::with_config(vec![pattern], config)?;

        let (files, report) = discovery.discover_files()?;

        // Large file should be skipped
        assert_eq!(files.len(), 0);
        assert_eq!(report.files_skipped_size, 1);

        Ok(())
    }

    #[test]
    fn test_gitignore_integration() -> Result<()> {
        let temp_dir = TempDir::new().map_err(OutlineError::FileSystem)?;

        // Initialize as git repo
        fs::create_dir_all(temp_dir.path().join(".git"))?;

        // Create .gitignore
        fs::write(temp_dir.path().join(".gitignore"), "*.tmp\ntarget/\n")?;

        // Create files
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;
        fs::write(temp_dir.path().join("temp.tmp"), "temporary")?;

        // Create target directory
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir)?;
        fs::write(target_dir.join("build.rs"), "fn main() {}")?;

        let pattern = format!("{}/**/*", temp_dir.path().display());
        let discovery = FileDiscovery::new(vec![pattern])?;

        let (files, _report) = discovery.discover_files()?;

        // Should only find main.rs (temp.tmp and target/build.rs should be ignored)
        let rs_files: Vec<_> = files
            .iter()
            .filter(|f| f.extension() == Some("rs"))
            .collect();
        assert_eq!(rs_files.len(), 1);
        assert!(rs_files[0].path.file_name().unwrap() == "main.rs");

        Ok(())
    }

    #[test]
    fn test_language_statistics() -> Result<()> {
        let temp_dir = TempDir::new().map_err(OutlineError::FileSystem)?;
        create_test_files(&temp_dir)?;

        let pattern = format!("{}/**/*", temp_dir.path().display());
        let discovery = FileDiscovery::new(vec![pattern])?;

        let (files, _report) = discovery.discover_files()?;
        let stats = FileDiscovery::get_language_statistics(&files);

        // Check that we have multiple languages
        assert!(stats.contains_key(&Language::Rust));
        assert!(stats.contains_key(&Language::Python));
        assert!(stats.contains_key(&Language::TypeScript));
        assert!(stats.contains_key(&Language::JavaScript));
        assert!(stats.contains_key(&Language::Dart));

        Ok(())
    }

    #[test]
    fn test_supported_patterns() {
        let patterns = FileDiscovery::supported_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.contains("*.rs")));
        assert!(patterns.iter().any(|p| p.contains("*.py")));
        assert!(patterns.iter().any(|p| p.contains("*.ts")));
        assert!(patterns.iter().any(|p| p.contains("*.js")));
        assert!(patterns.iter().any(|p| p.contains("*.dart")));
    }

    #[test]
    fn test_filter_supported_files() {
        let files = vec![
            DiscoveredFile::new(
                PathBuf::from("test.rs"),
                Language::Rust,
                "test.rs".to_string(),
                100,
            ),
            DiscoveredFile::new(
                PathBuf::from("test.txt"),
                Language::Unknown,
                "test.txt".to_string(),
                50,
            ),
        ];

        let supported = FileDiscovery::filter_supported_files(files);
        assert_eq!(supported.len(), 1);
        assert_eq!(supported[0].language, Language::Rust);
    }
}
