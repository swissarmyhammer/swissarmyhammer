//! Utilities and helpers for semantic search

use crate::error::SearchResult;
use crate::storage::VectorStorage;
use crate::types::{ContentHash, FileChangeReport, FileChangeStatus, Language};
use std::path::{Path, PathBuf};

/// Utility functions for semantic search operations
pub struct SemanticUtils;

impl SemanticUtils {
    /// Normalize text content for better embedding quality
    pub fn normalize_text(content: &str) -> String {
        // Remove excessive whitespace
        let normalized = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        // Remove comments for code content (basic implementation)
        Self::remove_basic_comments(&normalized)
    }

    /// Remove basic single-line and multi-line comments
    fn remove_basic_comments(content: &str) -> String {
        let mut result = String::new();
        let mut in_multiline_comment = false;
        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let mut processed_line = line.to_string();

            // Handle multi-line comments (/* */)
            if in_multiline_comment {
                if let Some(end_pos) = processed_line.find("*/") {
                    processed_line = processed_line[end_pos + 2..].to_string();
                    in_multiline_comment = false;
                } else {
                    continue; // Skip entire line if still in comment
                }
            }

            // Check for start of multi-line comment
            if let Some(start_pos) = processed_line.find("/*") {
                if let Some(end_pos) = processed_line[start_pos..].find("*/") {
                    // Comment starts and ends on same line
                    let before = &processed_line[..start_pos];
                    let after = &processed_line[start_pos + end_pos + 2..];
                    processed_line = format!("{before}{after}");
                } else {
                    // Comment starts but doesn't end on this line
                    processed_line = processed_line[..start_pos].to_string();
                    in_multiline_comment = true;
                }
            }

            // Remove single-line comments (//)
            if let Some(comment_pos) = processed_line.find("//") {
                processed_line = processed_line[..comment_pos].to_string();
            }

            let trimmed = processed_line.trim();
            if !trimmed.is_empty() {
                result.push_str(trimmed);
                result.push('\n');
            }
        }

        result.trim_end().to_string()
    }

    /// Calculate cosine similarity between two embedding vectors
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Generate a unique ID for a code chunk
    pub fn generate_chunk_id(file_path: &Path, start_line: usize, end_line: usize) -> String {
        format!("{}:{}:{}", file_path.display(), start_line, end_line)
    }

    /// Get the semantic search database directory
    pub fn get_database_dir() -> SearchResult<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            crate::error::SearchError::Config("Could not determine home directory".to_string())
        })?;

        Ok(home_dir.join(".swissarmyhammer"))
    }

    /// Ensure the database directory exists
    pub fn ensure_database_dir() -> SearchResult<PathBuf> {
        let db_dir = Self::get_database_dir()?;
        std::fs::create_dir_all(&db_dir).map_err(crate::error::SearchError::Io)?;
        Ok(db_dir)
    }

    /// Get file extension for language detection
    pub fn get_file_extensions_for_language(language: &Language) -> Vec<&'static str> {
        match language {
            Language::Rust => vec!["rs"],
            Language::Python => vec!["py", "pyw"],
            Language::TypeScript => vec!["ts", "tsx"],
            Language::JavaScript => vec!["js", "jsx"],
            Language::Dart => vec!["dart"],
            Language::Unknown => vec![], // No specific extensions for unknown languages
        }
    }

    /// Check if a file should be indexed based on its path
    pub fn should_index_file(file_path: &Path) -> bool {
        // Skip hidden files and directories
        for component in file_path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if name.starts_with('.') {
                    return false;
                }
            }
        }

        // Skip common build/dependency directories
        let path_str = file_path.to_string_lossy();
        let skip_patterns = [
            "target/",
            "node_modules/",
            ".git/",
            "build/",
            "dist/",
            "__pycache__/",
            ".pyc",
        ];

        for pattern in &skip_patterns {
            if path_str.contains(pattern) {
                return false;
            }
        }

        true
    }
}

/// Utility for MD5-based file content hashing
pub struct FileHasher;

impl FileHasher {
    /// Calculate MD5 hash of file content
    pub fn hash_file(path: impl AsRef<Path>) -> SearchResult<ContentHash> {
        let path = path.as_ref();
        let content = std::fs::read(path).map_err(crate::error::SearchError::Io)?;
        let hash = Self::hash_content(&content);
        Ok(hash)
    }

    /// Calculate MD5 hash of content bytes
    pub fn hash_content(content: &[u8]) -> ContentHash {
        let digest = md5::compute(content);
        ContentHash(format!("{digest:x}"))
    }

    /// Calculate hash of string content (for testing)
    pub fn hash_string(content: &str) -> ContentHash {
        Self::hash_content(content.as_bytes())
    }
}

/// Tracks file changes for smart re-indexing
pub struct FileChangeTracker {
    storage: VectorStorage,
}

impl FileChangeTracker {
    /// Create a new file change tracker
    pub fn new(storage: VectorStorage) -> Self {
        Self { storage }
    }

    /// Check multiple files for changes and return those that need re-indexing
    pub fn check_files_for_changes<P: AsRef<Path>>(
        &self,
        file_paths: impl IntoIterator<Item = P>,
    ) -> SearchResult<FileChangeReport> {
        let mut report = FileChangeReport::new();

        for path in file_paths {
            let path = path.as_ref();
            match self.check_single_file(path) {
                Ok(status) => {
                    report.add_file_status(path.to_path_buf(), status);
                }
                Err(e) => {
                    tracing::warn!("Failed to check file {}: {}", path.display(), e);
                    report.add_error(path.to_path_buf(), e);
                }
            }
        }

        Ok(report)
    }

    fn check_single_file(&self, path: &Path) -> SearchResult<FileChangeStatus> {
        // Calculate current hash
        let current_hash = FileHasher::hash_file(path)?;

        // Check if file needs re-indexing
        let needs_reindexing = self.storage.needs_reindexing(path, &current_hash)?;

        Ok(match needs_reindexing {
            true => FileChangeStatus::Changed {
                new_hash: current_hash,
                exists_in_index: self.storage.file_exists(path)?,
            },
            false => FileChangeStatus::Unchanged { hash: current_hash },
        })
    }
}

/// Utility for processing files in batches to avoid memory issues
pub struct BatchProcessor {
    batch_size: usize,
}

impl BatchProcessor {
    /// Create a new batch processor with the specified batch size
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    /// Process files in batches to avoid memory issues
    pub fn process_files_in_batches<F, T>(
        &self,
        files: Vec<T>,
        mut processor: F,
    ) -> SearchResult<()>
    where
        F: FnMut(&[T]) -> SearchResult<()>,
        T: Clone,
    {
        for chunk in files.chunks(self.batch_size) {
            processor(chunk)?;

            // Optional: add progress logging
            tracing::debug!("Processed batch of {} files", chunk.len());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_text() {
        let input = "  fn main() {  \n\n  println!(\"hello\");  \n  }  \n\n";
        let expected = "fn main() {\nprintln!(\"hello\");\n}";
        assert_eq!(SemanticUtils::normalize_text(input), expected);
    }

    #[test]
    fn test_remove_basic_comments() {
        let input =
            "fn main() { // This is a comment\n    println!(\"hello\"); // Another comment\n}";
        let expected = "fn main() {\nprintln!(\"hello\");\n}";
        assert_eq!(SemanticUtils::normalize_text(input), expected);
    }

    #[test]
    fn test_remove_multiline_comments() {
        let input = "fn main() {\n    /* This is a\n       multiline comment */\n    println!(\"hello\");\n}";
        let expected = "fn main() {\nprintln!(\"hello\");\n}";
        assert_eq!(SemanticUtils::normalize_text(input), expected);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((SemanticUtils::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((SemanticUtils::cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);

        let a = vec![1.0, 1.0, 0.0];
        let b = vec![1.0, 1.0, 0.0];
        assert!((SemanticUtils::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_generate_chunk_id() {
        let path = Path::new("src/main.rs");
        let id = SemanticUtils::generate_chunk_id(path, 10, 20);
        assert_eq!(id, "src/main.rs:10:20");
    }

    #[test]
    fn test_get_file_extensions_for_language() {
        assert_eq!(
            SemanticUtils::get_file_extensions_for_language(&Language::Rust),
            vec!["rs"]
        );
        assert_eq!(
            SemanticUtils::get_file_extensions_for_language(&Language::Python),
            vec!["py", "pyw"]
        );
        assert!(
            SemanticUtils::get_file_extensions_for_language(&Language::TypeScript).contains(&"ts")
        );
    }

    #[test]
    fn test_should_index_file() {
        assert!(SemanticUtils::should_index_file(Path::new("src/main.rs")));
        assert!(SemanticUtils::should_index_file(Path::new("lib/utils.py")));

        assert!(!SemanticUtils::should_index_file(Path::new(
            ".hidden/file.rs"
        )));
        assert!(!SemanticUtils::should_index_file(Path::new(
            "target/debug/main"
        )));
        assert!(!SemanticUtils::should_index_file(Path::new(
            "node_modules/package/index.js"
        )));
        assert!(!SemanticUtils::should_index_file(Path::new(
            "src/__pycache__/module.pyc"
        )));
    }

    #[test]
    fn test_file_hasher_hash_string() {
        let content = "Hello, world!";
        let hash1 = FileHasher::hash_string(content);
        let hash2 = FileHasher::hash_string(content);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Different content should produce different hash
        let different_hash = FileHasher::hash_string("Different content");
        assert_ne!(hash1, different_hash);
    }

    #[test]
    fn test_file_hasher_hash_content() {
        let content = b"Hello, world!";
        let hash1 = FileHasher::hash_content(content);
        let hash2 = FileHasher::hash_content(content);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);

        // Different content should produce different hash
        let different_hash = FileHasher::hash_content(b"Different content");
        assert_ne!(hash1, different_hash);
    }

    #[test]
    fn test_batch_processor_new() {
        let processor = BatchProcessor::new(10);
        assert_eq!(processor.batch_size, 10);
    }

    #[test]
    fn test_batch_processor_process_files() {
        let processor = BatchProcessor::new(2);
        let files = vec![1, 2, 3, 4, 5];
        let mut processed_batches = Vec::new();

        let result = processor.process_files_in_batches(files, |chunk| {
            processed_batches.push(chunk.to_vec());
            Ok(())
        });

        assert!(result.is_ok());
        assert_eq!(processed_batches.len(), 3); // 5 files with batch size 2 = 3 batches
        assert_eq!(processed_batches[0], vec![1, 2]);
        assert_eq!(processed_batches[1], vec![3, 4]);
        assert_eq!(processed_batches[2], vec![5]);
    }

    #[test]
    fn test_batch_processor_error_handling() {
        let processor = BatchProcessor::new(2);
        let files = vec![1, 2, 3, 4];

        let result = processor.process_files_in_batches(files, |chunk| {
            if chunk.contains(&3) {
                Err(crate::error::SearchError::Config("Test error".to_string()))
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
    }
}
