//! File discovery functionality for outline generation

use crate::{
    types::{DiscoveredFile, FileDiscoveryConfig, FileDiscoveryReport, Language},
    utils::{
        get_relative_path, is_likely_generated_file, matches_glob_pattern, parse_glob_pattern,
    },
    OutlineError, Result,
};
use ignore::WalkBuilder;
use std::path::Path;
use std::time::Instant;

/// File discovery engine for outline generation
#[derive(Debug)]
pub struct FileDiscovery {
    /// Glob patterns to match files against
    patterns: Vec<String>,
    /// Configuration for file discovery
    config: FileDiscoveryConfig,
}

impl FileDiscovery {
    /// Create a new FileDiscovery instance with default configuration
    pub fn new(patterns: Vec<String>) -> Result<Self> {
        Self::with_config(patterns, FileDiscoveryConfig::default())
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

        Ok(Self { patterns, config })
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
        let mut total_files = 0;
        let mut filtered_files = 0;

        tracing::info!("Starting file discovery with patterns: {:?}", self.patterns);

        for pattern in &self.patterns {
            let (pattern_files, pattern_total, pattern_filtered) =
                self.discover_files_for_pattern(pattern)?;
            discovered_files.extend(pattern_files);
            total_files += pattern_total;
            filtered_files += pattern_filtered;
        }

        // Remove duplicates (same file might match multiple patterns)
        discovered_files.sort_by(|a, b| a.path.cmp(&b.path));
        discovered_files.dedup_by(|a, b| a.path == b.path);

        let supported_files = discovered_files.len();
        let discovery_time = start_time.elapsed();

        let report = FileDiscoveryReport {
            total_files,
            filtered_files,
            supported_files,
            discovery_time,
            patterns: self.patterns.clone(),
        };

        tracing::info!("File discovery completed: {}", report.summary());

        Ok((discovered_files, report))
    }

    /// Discover files for a single glob pattern
    fn discover_files_for_pattern(
        &self,
        pattern: &str,
    ) -> Result<(Vec<DiscoveredFile>, usize, usize)> {
        let mut files = Vec::new();
        let mut total_count = 0;
        let mut filtered_count = 0;

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
            return Ok((files, 0, 0));
        }

        // Create a walker that respects configuration
        let mut walker_builder = WalkBuilder::new(&base_dir);
        walker_builder
            .git_ignore(self.config.respect_gitignore)
            .git_global(self.config.respect_gitignore)
            .git_exclude(self.config.respect_gitignore)
            .hidden(!self.config.include_hidden)
            .parents(true);

        let walker = walker_builder.build();

        // Walk the directory structure and collect matching files
        for entry in walker {
            match entry {
                Ok(dir_entry) => {
                    let path = dir_entry.path();
                    if path.is_file() {
                        total_count += 1;
                        match self.process_file(path, &base_dir, &file_pattern) {
                            Ok(Some(discovered_file)) => {
                                files.push(discovered_file);
                            }
                            Ok(None) => {
                                filtered_count += 1;
                            }
                            Err(e) => {
                                tracing::warn!("Error processing file {}: {}", path.display(), e);
                                filtered_count += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Error processing directory entry: {}", e);
                }
            }
        }

        Ok((files, total_count, filtered_count))
    }

    /// Process a single file for inclusion in the discovery results
    fn process_file(
        &self,
        path: &Path,
        base_dir: &Path,
        file_pattern: &str,
    ) -> Result<Option<DiscoveredFile>> {
        // Check if the file matches the glob pattern
        let relative_path = get_relative_path(path, base_dir);
        if !matches_glob_pattern(Path::new(&relative_path), file_pattern).map_err(|e| {
            OutlineError::InvalidGlobPattern {
                pattern: file_pattern.to_string(),
                message: e.to_string(),
            }
        })? {
            return Ok(None);
        }

        // Skip likely generated files even if not in gitignore
        if is_likely_generated_file(path) {
            tracing::debug!("Skipping likely generated file: {}", path.display());
            return Ok(None);
        }

        // Get file metadata
        let metadata = std::fs::metadata(path).map_err(OutlineError::FileSystem)?;
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
                return Ok(None);
            }
        }

        // Detect language from file extension
        let language = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            Language::from_extension(ext)
        } else {
            Language::Unknown
        };

        // Create discovered file (include all files, filtering happens later if needed)
        let discovered_file = DiscoveredFile::new(
            path.to_path_buf(),
            language.clone(),
            relative_path,
            file_size,
        );

        tracing::debug!(
            "Discovered file: {} ({:?}, {} bytes)",
            path.display(),
            language,
            file_size
        );

        Ok(Some(discovered_file))
    }

    /// Get supported file patterns based on supported languages
    pub fn supported_patterns() -> Vec<String> {
        vec![
            "**/*.rs".to_string(),   // Rust
            "**/*.py".to_string(),   // Python
            "**/*.ts".to_string(),   // TypeScript
            "**/*.tsx".to_string(),  // TypeScript JSX
            "**/*.js".to_string(),   // JavaScript
            "**/*.jsx".to_string(),  // JavaScript JSX
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
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[allow(dead_code)]
    fn create_test_files(temp_dir: &TempDir) -> Result<()> {
        // Create various test files
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}")
            .map_err(OutlineError::FileSystem)?;
        fs::write(temp_dir.path().join("lib.py"), "def hello(): pass")
            .map_err(OutlineError::FileSystem)?;
        fs::write(temp_dir.path().join("app.ts"), "console.log('hello');")
            .map_err(OutlineError::FileSystem)?;
        fs::write(temp_dir.path().join("script.js"), "alert('test');")
            .map_err(OutlineError::FileSystem)?;
        fs::write(temp_dir.path().join("widget.dart"), "void main() {}")
            .map_err(OutlineError::FileSystem)?;
        fs::write(temp_dir.path().join("README.md"), "# Test").map_err(OutlineError::FileSystem)?;

        // Create subdirectory with files
        let subdir = temp_dir.path().join("src");
        fs::create_dir_all(&subdir).map_err(OutlineError::FileSystem)?;
        fs::write(subdir.join("module.rs"), "pub fn test() {}")
            .map_err(OutlineError::FileSystem)?;
        fs::write(subdir.join("utils.py"), "import os").map_err(OutlineError::FileSystem)?;

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
