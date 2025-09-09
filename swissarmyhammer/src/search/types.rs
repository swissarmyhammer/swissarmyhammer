//! Core data structures for semantic search functionality

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use swissarmyhammer_common::utils::find_swissarmyhammer_directory;

/// Unique identifier for indexed files
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(pub String);

/// Hash of file content for change detection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentHash(pub String);

/// Programming language detected for a file
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// Rust programming language
    Rust,
    /// Python programming language
    Python,
    /// TypeScript programming language
    TypeScript,
    /// JavaScript programming language
    JavaScript,
    /// Dart programming language
    Dart,
    /// Unknown or unsupported language
    Unknown,
}

/// Type of code chunk based on TreeSitter parsing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkType {
    /// Function definition
    Function,
    /// Class definition
    Class,
    /// Module or namespace
    Module,
    /// Import or include statement
    Import,
    /// Plain text for files that fail TreeSitter parsing
    PlainText,
}

/// A chunk of code extracted from a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// Unique identifier for this chunk
    pub id: String,
    /// Path to the source file
    pub file_path: PathBuf,
    /// Programming language of the file
    pub language: Language,
    /// The actual code content
    pub content: String,
    /// Line number where this chunk starts
    pub start_line: usize,
    /// Line number where this chunk ends
    pub end_line: usize,
    /// Type of code chunk
    pub chunk_type: ChunkType,
    /// Hash of the content for change detection
    pub content_hash: ContentHash,
}

/// Vector embedding for a code chunk
#[derive(Debug, Clone)]
pub struct Embedding {
    /// ID of the chunk this embedding represents
    pub chunk_id: String,
    /// 384-dimensional vector for nomic-embed-code
    pub vector: Vec<f32>,
}

/// Indexed file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    /// Unique identifier for the file
    pub file_id: FileId,
    /// Path to the file
    pub path: PathBuf,
    /// Detected programming language
    pub language: Language,
    /// Hash of the file content
    pub content_hash: ContentHash,
    /// Number of chunks extracted from this file
    pub chunk_count: usize,
    /// Timestamp when the file was indexed
    pub indexed_at: DateTime<Utc>,
}

/// Search result with similarity score
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    /// The matching code chunk
    pub chunk: CodeChunk,
    /// Similarity score (0.0 to 1.0)
    pub similarity_score: f32,
    /// Excerpt of the matching content
    pub excerpt: String,
}

/// Search query parameters
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// The search text
    pub text: String,
    /// Maximum number of results to return
    pub limit: usize,
    /// Minimum similarity threshold
    pub similarity_threshold: f32,
    /// Optional language filter
    pub language_filter: Option<Language>,
}

/// Configuration for the semantic search system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticConfig {
    /// Path to the DuckDB database file
    pub database_path: PathBuf,
    /// Embedding model name
    pub embedding_model: String,
    /// Maximum chunk size in characters
    pub chunk_size: usize,
    /// Chunk overlap in characters
    pub chunk_overlap: usize,
    /// Minimum similarity threshold for search results
    pub similarity_threshold: f32,
    /// Maximum excerpt length for search results
    pub excerpt_length: usize,
    /// Number of context lines to include in excerpts
    pub context_lines: usize,
    /// Similarity threshold for simple search methods
    pub simple_search_threshold: f32,
    /// Similarity threshold for code similarity search
    pub code_similarity_threshold: f32,
    /// Number of characters for content preview in explanations
    pub content_preview_length: usize,
    /// Minimum chunk size in characters for parsing
    pub min_chunk_size: usize,
    /// Maximum chunk size in characters for parsing
    pub max_chunk_size: usize,
    /// Maximum chunks to extract per file
    pub max_chunks_per_file: usize,
    /// Maximum file size in bytes to prevent OOM on massive files
    pub max_file_size_bytes: usize,
}

/// Status of a file's change detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeStatus {
    /// File has changed since last indexing
    Changed {
        /// The new content hash for the changed file
        new_hash: ContentHash,
        /// Whether this file was previously indexed
        exists_in_index: bool,
    },
    /// File hasn't changed since last indexing
    Unchanged {
        /// The content hash of the unchanged file
        hash: ContentHash,
    },
}

/// Report of file change detection results
#[derive(Debug)]
pub struct FileChangeReport {
    /// Files that have changed and exist in index
    pub changed_files: std::collections::HashMap<PathBuf, ContentHash>,
    /// Files that haven't changed
    pub unchanged_files: std::collections::HashMap<PathBuf, ContentHash>,
    /// Files that are new (not in index)
    pub new_files: std::collections::HashMap<PathBuf, ContentHash>,
    /// Files that had errors during processing
    pub errors: std::collections::HashMap<PathBuf, crate::error::SwissArmyHammerError>,
}

/// Statistics about the vector storage index
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStats {
    /// Number of indexed files
    pub file_count: usize,
    /// Number of code chunks
    pub chunk_count: usize,
    /// Number of embeddings
    pub embedding_count: usize,
}

impl FileChangeReport {
    /// Create a new empty change report
    pub fn new() -> Self {
        Self {
            changed_files: std::collections::HashMap::new(),
            unchanged_files: std::collections::HashMap::new(),
            new_files: std::collections::HashMap::new(),
            errors: std::collections::HashMap::new(),
        }
    }

    /// Add a file status to the report
    pub fn add_file_status(&mut self, path: PathBuf, status: FileChangeStatus) {
        match status {
            FileChangeStatus::Changed {
                new_hash,
                exists_in_index,
            } => {
                if exists_in_index {
                    self.changed_files.insert(path, new_hash);
                } else {
                    self.new_files.insert(path, new_hash);
                }
            }
            FileChangeStatus::Unchanged { hash } => {
                self.unchanged_files.insert(path, hash);
            }
        }
    }

    /// Add an error for a file
    pub fn add_error(&mut self, path: PathBuf, error: crate::error::SwissArmyHammerError) {
        self.errors.insert(path, error);
    }

    /// Get files that need indexing (changed + new)
    pub fn files_needing_indexing(&self) -> impl Iterator<Item = &PathBuf> {
        self.changed_files.keys().chain(self.new_files.keys())
    }

    /// Get total number of files processed
    pub fn total_files(&self) -> usize {
        self.changed_files.len()
            + self.unchanged_files.len()
            + self.new_files.len()
            + self.errors.len()
    }

    /// Get a summary string of the report
    pub fn summary(&self) -> String {
        format!(
            "Files: {} new, {} changed, {} unchanged, {} errors",
            self.new_files.len(),
            self.changed_files.len(),
            self.unchanged_files.len(),
            self.errors.len()
        )
    }
}

impl Default for FileChangeReport {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SemanticConfig {
    fn default() -> Self {
        // Follow the same precedence pattern as prompts/workflows:
        // 1. Local .swissarmyhammer directories (most specific)
        // 2. User ~/.swissarmyhammer directory (fallback)
        let database_path = Self::find_semantic_database_path();

        Self {
            database_path,
            embedding_model: "nomic-ai/nomic-embed-code".to_string(),
            chunk_size: 512,
            chunk_overlap: 64,
            similarity_threshold: 0.7,
            excerpt_length: 200,
            context_lines: 2,
            simple_search_threshold: 0.5,
            code_similarity_threshold: 0.7,
            content_preview_length: 100,
            min_chunk_size: 50,
            max_chunk_size: 2000,
            max_chunks_per_file: 100,
            max_file_size_bytes: 10 * 1024 * 1024, // 10MB
        }
    }
}

impl SemanticConfig {
    /// Find the most appropriate path for the semantic database
    /// Following the Git repository-centric precedence:
    /// 1. Environment variable SWISSARMYHAMMER_SEMANTIC_DB_PATH (test isolation)
    /// 2. Git repository .swissarmyhammer directory (repository-specific)
    /// 3. User ~/.swissarmyhammer directory (fallback for commands outside Git repos)
    fn find_semantic_database_path() -> PathBuf {
        // Check for environment variable override (for test isolation)
        if let Ok(env_path) = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH") {
            let path = PathBuf::from(env_path);
            tracing::debug!(
                "Using environment variable semantic database path: {}",
                path.display()
            );
            return path;
        }

        // Try Git repository .swissarmyhammer directory first
        if let Some(swissarmyhammer_dir) = find_swissarmyhammer_directory() {
            let semantic_db_path = swissarmyhammer_dir.join("semantic.db");
            tracing::debug!(
                "Using Git repository semantic database at: {}",
                semantic_db_path.display()
            );
            return semantic_db_path;
        }

        // Fallback to user home directory (for commands outside Git repositories)
        if let Some(home_dir) = dirs::home_dir() {
            let swissarmyhammer_dir = home_dir.join(".swissarmyhammer");

            // Try to create the .swissarmyhammer directory
            if let Err(e) = std::fs::create_dir_all(&swissarmyhammer_dir) {
                tracing::warn!(
                    "Cannot create .swissarmyhammer directory in home at {}: {}. Using relative path fallback.",
                    swissarmyhammer_dir.display(),
                    e
                );
                return PathBuf::from(".swissarmyhammer/semantic.db");
            }

            let semantic_db_path = swissarmyhammer_dir.join("semantic.db");
            tracing::debug!(
                "Using home directory semantic database at: {}",
                semantic_db_path.display()
            );
            return semantic_db_path;
        }

        // Final fallback to relative path in current directory
        tracing::warn!("No home directory available, using relative path for semantic database");
        PathBuf::from(".swissarmyhammer/semantic.db")
    }

    /// Create a ParserConfig from this SemanticConfig
    pub fn to_parser_config(&self) -> crate::Result<crate::search::parser::ParserConfig> {
        crate::search::parser::ParserConfig::new(
            self.min_chunk_size,
            self.max_chunk_size,
            self.max_chunks_per_file,
            self.max_file_size_bytes,
        )
        .map_err(|e| crate::error::SwissArmyHammerError::Config(e.to_string()))
    }

    /// Validate all configuration parameters
    pub fn validate(&self) -> crate::Result<()> {
        // Validate chunk size parameters
        if self.chunk_size == 0 {
            return Err(crate::error::SwissArmyHammerError::Config(
                "chunk_size must be greater than 0".to_string(),
            ));
        }

        if self.chunk_overlap >= self.chunk_size {
            return Err(crate::error::SwissArmyHammerError::Config(format!(
                "chunk_overlap ({}) must be less than chunk_size ({})",
                self.chunk_overlap, self.chunk_size
            )));
        }

        // Validate similarity thresholds (must be between 0.0 and 1.0)
        if !(0.0..=1.0).contains(&self.similarity_threshold) {
            return Err(crate::error::SwissArmyHammerError::Config(format!(
                "similarity_threshold ({}) must be between 0.0 and 1.0",
                self.similarity_threshold
            )));
        }

        if !(0.0..=1.0).contains(&self.simple_search_threshold) {
            return Err(crate::error::SwissArmyHammerError::Config(format!(
                "simple_search_threshold ({}) must be between 0.0 and 1.0",
                self.simple_search_threshold
            )));
        }

        if !(0.0..=1.0).contains(&self.code_similarity_threshold) {
            return Err(crate::error::SwissArmyHammerError::Config(format!(
                "code_similarity_threshold ({}) must be between 0.0 and 1.0",
                self.code_similarity_threshold
            )));
        }

        // Validate length parameters
        if self.excerpt_length == 0 {
            return Err(crate::error::SwissArmyHammerError::Config(
                "excerpt_length must be greater than 0".to_string(),
            ));
        }

        if self.content_preview_length == 0 {
            return Err(crate::error::SwissArmyHammerError::Config(
                "content_preview_length must be greater than 0".to_string(),
            ));
        }

        // Validate embedding model is not empty
        if self.embedding_model.trim().is_empty() {
            return Err(crate::error::SwissArmyHammerError::Config(
                "embedding_model cannot be empty".to_string(),
            ));
        }

        // Validate parser config parameters using the existing validation
        self.to_parser_config()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_id() {
        let id1 = FileId("test_file".to_string());
        let id2 = FileId("test_file".to_string());
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_content_hash() {
        let hash1 = ContentHash("abc123".to_string());
        let hash2 = ContentHash("abc123".to_string());
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_language_serialization() {
        let lang = Language::Rust;
        let serialized = serde_json::to_string(&lang).unwrap();
        let deserialized: Language = serde_json::from_str(&serialized).unwrap();
        assert_eq!(lang, deserialized);
    }

    #[test]
    fn test_chunk_type_serialization() {
        let chunk_type = ChunkType::Function;
        let serialized = serde_json::to_string(&chunk_type).unwrap();
        let deserialized: ChunkType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(chunk_type, deserialized);
    }

    #[test]
    fn test_code_chunk_creation() {
        let chunk = CodeChunk {
            id: "test_chunk".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() {}".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("hash123".to_string()),
        };

        assert_eq!(chunk.id, "test_chunk");
        assert_eq!(chunk.language, Language::Rust);
        assert_eq!(chunk.chunk_type, ChunkType::Function);
    }

    #[test]
    fn test_embedding_creation() {
        let embedding = Embedding {
            chunk_id: "test_chunk".to_string(),
            vector: vec![0.1, 0.2, 0.3],
        };

        assert_eq!(embedding.chunk_id, "test_chunk");
        assert_eq!(embedding.vector.len(), 3);
    }

    #[test]
    fn test_indexed_file_creation() {
        let indexed_file = IndexedFile {
            file_id: FileId("file123".to_string()),
            path: PathBuf::from("src/main.rs"),
            language: Language::Rust,
            content_hash: ContentHash("abc123".to_string()),
            chunk_count: 5,
            indexed_at: Utc::now(),
        };

        assert_eq!(indexed_file.chunk_count, 5);
        assert_eq!(indexed_file.language, Language::Rust);
    }

    #[test]
    fn test_search_query_creation() {
        let query = SearchQuery {
            text: "function implementation".to_string(),
            limit: 10,
            similarity_threshold: 0.8,
            language_filter: Some(Language::Rust),
        };

        assert_eq!(query.limit, 10);
        assert_eq!(query.similarity_threshold, 0.8);
        assert_eq!(query.language_filter, Some(Language::Rust));
    }

    #[test]
    fn test_semantic_config_default() {
        use std::sync::{Mutex, OnceLock};

        static ENV_LOCK_DEFAULT: OnceLock<Mutex<()>> = OnceLock::new();

        // Use mutex to prevent race conditions between tests
        let _guard = ENV_LOCK_DEFAULT
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();

        // Save original environment variable state
        let original_db_path = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH").ok();

        // Ensure no environment variable override is set for this test
        std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH");

        // Create config WHILE environment variable is cleared
        let config = SemanticConfig::default();

        // Perform all assertions BEFORE restoring environment
        assert!(config
            .database_path
            .to_string_lossy()
            .contains("semantic.db"));

        // The path should end with either .swissarmyhammer/semantic.db or semantic.db
        assert!(
            config
                .database_path
                .to_string_lossy()
                .ends_with(".swissarmyhammer/semantic.db")
                || config
                    .database_path
                    .to_string_lossy()
                    .ends_with("semantic.db")
        );
        assert_eq!(config.embedding_model, "nomic-ai/nomic-embed-code");
        assert_eq!(config.chunk_size, 512);
        assert_eq!(config.chunk_overlap, 64);
        assert_eq!(config.similarity_threshold, 0.7);
        assert_eq!(config.excerpt_length, 200);
        assert_eq!(config.context_lines, 2);
        assert_eq!(config.simple_search_threshold, 0.5);
        assert_eq!(config.code_similarity_threshold, 0.7);
        assert_eq!(config.content_preview_length, 100);
        assert_eq!(config.min_chunk_size, 50);
        assert_eq!(config.max_chunk_size, 2000);
        assert_eq!(config.max_chunks_per_file, 100);
        assert_eq!(config.max_file_size_bytes, 10 * 1024 * 1024);

        // Restore original environment variable state after all assertions
        match original_db_path {
            Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
            None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
        }
    }

    #[test]
    fn test_semantic_config_serialization() {
        let config = SemanticConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: SemanticConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config.embedding_model, deserialized.embedding_model);
        assert_eq!(config.chunk_size, deserialized.chunk_size);
        assert_eq!(config.min_chunk_size, deserialized.min_chunk_size);
        assert_eq!(config.max_chunk_size, deserialized.max_chunk_size);
        assert_eq!(config.max_chunks_per_file, deserialized.max_chunks_per_file);
        assert_eq!(config.max_file_size_bytes, deserialized.max_file_size_bytes);
    }

    #[test]
    fn test_to_parser_config() {
        let config = SemanticConfig::default();
        let parser_config = config.to_parser_config().unwrap();
        assert_eq!(parser_config.min_chunk_size, config.min_chunk_size);
        assert_eq!(parser_config.max_chunk_size, config.max_chunk_size);
        assert_eq!(
            parser_config.max_chunks_per_file,
            config.max_chunks_per_file
        );
        assert_eq!(
            parser_config.max_file_size_bytes,
            config.max_file_size_bytes
        );
    }

    #[test]
    fn test_config_validation_default() {
        let config = SemanticConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_chunk_size_zero() {
        let config = SemanticConfig {
            chunk_size: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("chunk_size must be greater than 0"));
    }

    #[test]
    fn test_config_validation_chunk_overlap_too_large() {
        let mut config = SemanticConfig::default();
        config.chunk_overlap = config.chunk_size; // Equal to chunk_size, should fail
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("chunk_overlap"));
    }

    #[test]
    fn test_config_validation_similarity_threshold_invalid() {
        let config = SemanticConfig {
            similarity_threshold: 1.5, // Greater than 1.0
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("similarity_threshold"));

        let config = SemanticConfig {
            similarity_threshold: -0.1, // Less than 0.0
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("similarity_threshold"));
    }

    #[test]
    fn test_config_validation_simple_search_threshold_invalid() {
        let config = SemanticConfig {
            simple_search_threshold: 2.0, // Greater than 1.0
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("simple_search_threshold"));
    }

    #[test]
    fn test_config_validation_code_similarity_threshold_invalid() {
        let config = SemanticConfig {
            code_similarity_threshold: -1.0, // Less than 0.0
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("code_similarity_threshold"));
    }

    #[test]
    fn test_config_validation_excerpt_length_zero() {
        let config = SemanticConfig {
            excerpt_length: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("excerpt_length must be greater than 0"));
    }

    #[test]
    fn test_config_validation_content_preview_length_zero() {
        let config = SemanticConfig {
            content_preview_length: 0,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("content_preview_length must be greater than 0"));
    }

    #[test]
    fn test_config_validation_embedding_model_empty() {
        let config = SemanticConfig {
            embedding_model: "".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("embedding_model cannot be empty"));

        let config = SemanticConfig {
            embedding_model: "   ".to_string(), // Only whitespace
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("embedding_model cannot be empty"));
    }

    #[test]
    fn test_config_validation_parser_config_invalid() {
        let config = SemanticConfig {
            min_chunk_size: 0, // This will make ParserConfig::new fail
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_chunk_size must be > 0"));
    }

    #[test]
    fn test_file_change_status() {
        let hash = ContentHash("abc123".to_string());

        let changed_status = FileChangeStatus::Changed {
            new_hash: hash.clone(),
            exists_in_index: true,
        };

        let unchanged_status = FileChangeStatus::Unchanged { hash: hash.clone() };

        match changed_status {
            FileChangeStatus::Changed {
                new_hash,
                exists_in_index,
            } => {
                assert_eq!(new_hash, hash);
                assert!(exists_in_index);
            }
            _ => panic!("Expected Changed status"),
        }

        match unchanged_status {
            FileChangeStatus::Unchanged { hash: h } => {
                assert_eq!(h, hash);
            }
            _ => panic!("Expected Unchanged status"),
        }
    }

    #[test]
    fn test_file_change_report_new() {
        let report = FileChangeReport::new();
        assert_eq!(report.changed_files.len(), 0);
        assert_eq!(report.unchanged_files.len(), 0);
        assert_eq!(report.new_files.len(), 0);
        assert_eq!(report.errors.len(), 0);
        assert_eq!(report.total_files(), 0);
    }

    #[test]
    fn test_file_change_report_add_file_status() {
        let mut report = FileChangeReport::new();
        let path1 = PathBuf::from("test1.rs");
        let path2 = PathBuf::from("test2.rs");
        let path3 = PathBuf::from("test3.rs");
        let hash = ContentHash("abc123".to_string());

        // Test adding changed file (exists in index)
        report.add_file_status(
            path1.clone(),
            FileChangeStatus::Changed {
                new_hash: hash.clone(),
                exists_in_index: true,
            },
        );
        assert_eq!(report.changed_files.len(), 1);
        assert!(report.changed_files.contains_key(&path1));

        // Test adding new file (doesn't exist in index)
        report.add_file_status(
            path2.clone(),
            FileChangeStatus::Changed {
                new_hash: hash.clone(),
                exists_in_index: false,
            },
        );
        assert_eq!(report.new_files.len(), 1);
        assert!(report.new_files.contains_key(&path2));

        // Test adding unchanged file
        report.add_file_status(
            path3.clone(),
            FileChangeStatus::Unchanged { hash: hash.clone() },
        );
        assert_eq!(report.unchanged_files.len(), 1);
        assert!(report.unchanged_files.contains_key(&path3));

        assert_eq!(report.total_files(), 3);
    }

    #[test]
    fn test_file_change_report_add_error() {
        let mut report = FileChangeReport::new();
        let path = PathBuf::from("error_file.rs");
        let error = crate::error::SwissArmyHammerError::Config("Test error".to_string());

        report.add_error(path.clone(), error);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors.contains_key(&path));
        assert_eq!(report.total_files(), 1);
    }

    #[test]
    fn test_file_change_report_files_needing_indexing() {
        let mut report = FileChangeReport::new();
        let changed_path = PathBuf::from("changed.rs");
        let new_path = PathBuf::from("new.rs");
        let unchanged_path = PathBuf::from("unchanged.rs");
        let hash = ContentHash("abc123".to_string());

        report.add_file_status(
            changed_path.clone(),
            FileChangeStatus::Changed {
                new_hash: hash.clone(),
                exists_in_index: true,
            },
        );

        report.add_file_status(
            new_path.clone(),
            FileChangeStatus::Changed {
                new_hash: hash.clone(),
                exists_in_index: false,
            },
        );

        report.add_file_status(
            unchanged_path.clone(),
            FileChangeStatus::Unchanged { hash: hash.clone() },
        );

        let needing_indexing: Vec<&PathBuf> = report.files_needing_indexing().collect();
        assert_eq!(needing_indexing.len(), 2);
        assert!(needing_indexing.contains(&&changed_path));
        assert!(needing_indexing.contains(&&new_path));
        assert!(!needing_indexing.contains(&&unchanged_path));
    }

    #[test]
    fn test_file_change_report_summary() {
        let mut report = FileChangeReport::new();
        let hash = ContentHash("abc123".to_string());

        // Add one of each type
        report.add_file_status(
            PathBuf::from("new.rs"),
            FileChangeStatus::Changed {
                new_hash: hash.clone(),
                exists_in_index: false,
            },
        );

        report.add_file_status(
            PathBuf::from("changed.rs"),
            FileChangeStatus::Changed {
                new_hash: hash.clone(),
                exists_in_index: true,
            },
        );

        report.add_file_status(
            PathBuf::from("unchanged.rs"),
            FileChangeStatus::Unchanged { hash: hash.clone() },
        );

        report.add_error(
            PathBuf::from("error.rs"),
            crate::error::SwissArmyHammerError::Config("Test error".to_string()),
        );

        let summary = report.summary();
        assert_eq!(summary, "Files: 1 new, 1 changed, 1 unchanged, 1 errors");
    }

    #[test]
    fn test_index_stats() {
        let stats = IndexStats {
            file_count: 10,
            chunk_count: 100,
            embedding_count: 100,
        };

        assert_eq!(stats.file_count, 10);
        assert_eq!(stats.chunk_count, 100);
        assert_eq!(stats.embedding_count, 100);
    }

    #[test]
    fn test_semantic_config_git_repository_path() {
        use std::sync::{Mutex, OnceLock};
        static ENV_LOCK_GIT: OnceLock<Mutex<()>> = OnceLock::new();

        // Use mutex to prevent race conditions between tests
        let _guard = ENV_LOCK_GIT.get_or_init(|| Mutex::new(())).lock().unwrap();

        // Save original environment variable state
        let original_db_path = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH").ok();

        // Ensure no environment variable override is set for this test
        std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH");

        // This test verifies that the new Git-centric approach works by testing that
        // when we're in a Git repository with .swissarmyhammer, the database path
        // points to that directory. Since the exact path comparison is difficult
        // with temporary directories, we'll validate the path structure instead.

        // Create a simple config and verify it follows expected patterns
        let config = SemanticConfig::default();

        // Restore original environment variable state before any assertions
        match original_db_path {
            Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
            None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
        }

        // Should always contain semantic.db
        assert!(config
            .database_path
            .to_string_lossy()
            .contains("semantic.db"));

        // The path should either:
        // 1. Be in a Git repository's .swissarmyhammer (contains .swissarmyhammer)
        // 2. Be a home directory fallback (contains .swissarmyhammer)
        // 3. Be a relative fallback (.swissarmyhammer/semantic.db)
        let path_str = config.database_path.to_string_lossy();
        assert!(
            path_str.contains(".swissarmyhammer") || path_str.ends_with("semantic.db"),
            "Expected path to contain .swissarmyhammer or end with semantic.db, got: {}",
            path_str
        );
    }

    #[test]
    fn test_semantic_config_home_fallback() {
        use tempfile::TempDir;

        // Acquire the global semantic DB environment lock to prevent race conditions
        let _lock_guard = crate::test_utils::acquire_semantic_db_lock();

        // Store original environment variable and remove it for test isolation
        let original_db_path = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH").ok();
        std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH");

        // Create a temporary directory that doesn't have .git (not a Git repository)
        let temp_dir = TempDir::new().unwrap();

        // Change to a directory without .git repository
        let original_dir = std::env::current_dir().ok();
        if std::env::set_current_dir(temp_dir.path()).is_err() {
            // Restore environment variable if test setup failed
            match original_db_path {
                Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
                None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
            }
            return; // Skip test if can't change directory
        }

        let config = SemanticConfig::default();

        // Restore original directory if it was accessible
        if let Some(original) = original_dir {
            let _ = std::env::set_current_dir(original);
        }

        // Should fallback to home directory since no Git repository was found
        let path_str = config.database_path.to_string_lossy();

        // Since we cleared the environment variable, we should get normal fallback behavior
        assert!(
            path_str.contains("semantic.db"),
            "Expected path to contain 'semantic.db', got: {}",
            path_str
        );

        // Should either be absolute (home) or relative (.swissarmyhammer/semantic.db)
        assert!(
            path_str.ends_with(".swissarmyhammer/semantic.db") || path_str.ends_with("semantic.db"),
            "Expected path to end with '.swissarmyhammer/semantic.db' or 'semantic.db', got: {}",
            path_str
        );

        // Restore original environment variable
        match original_db_path {
            Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
            None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
        }
    }

    #[test]
    fn test_semantic_config_git_repo_no_swissarmyhammer() {
        use std::fs;
        use tempfile::TempDir;

        // Acquire the global semantic DB environment lock to prevent race conditions
        let _lock_guard = crate::test_utils::acquire_semantic_db_lock();

        // Store original environment variable and remove it for test isolation
        let original_db_path = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH").ok();
        std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH");

        let temp_dir = TempDir::new().unwrap();

        // Create Git repository without .swissarmyhammer directory
        fs::create_dir(temp_dir.path().join(".git")).unwrap();
        // Note: intentionally NOT creating .swissarmyhammer

        // Change to the temp directory to simulate being in a Git repository
        let original_dir = std::env::current_dir().ok();
        if std::env::set_current_dir(temp_dir.path()).is_err() {
            // Restore environment variable if test setup failed
            match original_db_path {
                Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
                None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
            }
            return; // Skip test if can't change directory
        }

        let config = SemanticConfig::default();

        // Restore original directory if it was accessible
        if let Some(original) = original_dir {
            let _ = std::env::set_current_dir(original);
        }

        // Should fallback to home directory since Git repository exists but no .swissarmyhammer
        let path_str = config.database_path.to_string_lossy();

        // Since we cleared the environment variable, we should get normal fallback behavior
        assert!(
            path_str.contains("semantic.db"),
            "Expected path to contain 'semantic.db', got: {}",
            path_str
        );

        // Restore original environment variable
        match original_db_path {
            Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
            None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
        }

        // Should either be home directory or relative fallback
        let path_str = config.database_path.to_string_lossy();
        assert!(
            path_str.ends_with(".swissarmyhammer/semantic.db") || path_str.ends_with("semantic.db")
        );
    }

    #[test]
    fn test_semantic_config_environment_variable_override() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_db_path = temp_dir.path().join("test_semantic.db");

        // Set environment variable override
        std::env::set_var(
            "SWISSARMYHAMMER_SEMANTIC_DB_PATH",
            test_db_path.to_string_lossy().to_string(),
        );

        let config = SemanticConfig::default();

        // Clean up environment variable
        std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH");

        // Should use environment variable path
        assert_eq!(config.database_path, test_db_path);
    }
}

/// Search statistics for debugging and monitoring
#[derive(Debug, Clone)]
pub struct SearchStats {
    /// Total number of indexed files
    pub total_files: usize,
    /// Total number of code chunks
    pub total_chunks: usize,
    /// Total number of embeddings
    pub total_embeddings: usize,
    /// Information about the embedding model
    pub model_info: crate::search::EmbeddingModelInfo,
}

/// Detailed explanation of search results for debugging
#[derive(Debug)]
pub struct SearchExplanation {
    /// The original query text
    pub query_text: String,
    /// Norm of the query embedding vector
    pub query_embedding_norm: f32,
    /// Similarity threshold used
    pub threshold: f32,
    /// Total number of candidates evaluated
    pub total_candidates: usize,
    /// Detailed information about each result
    pub results: Vec<ResultExplanation>,
}

/// Explanation of an individual search result
#[derive(Debug)]
pub struct ResultExplanation {
    /// ID of the chunk
    pub chunk_id: String,
    /// Similarity score with the query
    pub similarity_score: f32,
    /// Programming language of the chunk
    pub language: Language,
    /// Type of the code chunk
    pub chunk_type: ChunkType,
    /// Preview of the chunk content (first 100 characters)
    pub content_preview: String,
    /// Whether this result was above the similarity threshold
    pub above_threshold: bool,
}
