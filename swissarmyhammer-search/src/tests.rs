//! Integration tests for semantic search module

#[cfg(test)]
mod integration_tests {
    use crate::*;
    use crate::storage::VectorStorage;
    use crate::indexer::IndexingOptions;
    use tempfile::TempDir;

    /// Test isolation guard for semantic tests
    struct SemanticTestGuard {
        _temp_dir: TempDir,
        original_db_path: Option<String>,
        _lock_guard: std::sync::MutexGuard<'static, ()>,
    }

    impl SemanticTestGuard {
        fn new() -> Self {
            // Acquire the global semantic DB environment lock to prevent race conditions
            let lock_guard = crate::test_utils::acquire_semantic_db_lock();

            let temp_dir = TempDir::new().expect("Failed to create temp dir for semantic test");
            let original_db_path = std::env::var("SWISSARMYHAMMER_SEMANTIC_DB_PATH").ok();

            let test_db_path = temp_dir.path().join("semantic_test.db");
            std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", &test_db_path);

            Self {
                _temp_dir: temp_dir,
                original_db_path,
                _lock_guard: lock_guard,
            }
        }
    }

    impl Drop for SemanticTestGuard {
        fn drop(&mut self) {
            match &self.original_db_path {
                Some(path) => std::env::set_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH", path),
                None => std::env::remove_var("SWISSARMYHAMMER_SEMANTIC_DB_PATH"),
            }
        }
    }

    #[test]
    fn test_semantic_module_imports() {
        let _guard = SemanticTestGuard::new();

        // Test that all module exports are accessible
        let _config = SemanticConfig::default();
        let _language = Language::Rust;

        // Verify we can create core components
        let parser_config = ParserConfig::default();
        assert!(CodeParser::new(parser_config).is_ok());

        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(EmbeddingEngine::new_for_testing()).is_ok());

        let config = SemanticConfig::default();
        assert!(VectorStorage::new(config).is_ok());
    }

    #[test]
    fn test_language_enum() {
        // Test Language enum serialization/deserialization
        let languages = vec![
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::JavaScript,
            Language::Dart,
        ];

        for lang in languages {
            let serialized = serde_json::to_string(&lang).unwrap();
            let deserialized: Language = serde_json::from_str(&serialized).unwrap();
            assert_eq!(lang, deserialized);
        }
    }

    #[test]
    fn test_semantic_config_default() {
        let config = SemanticConfig::default();
        assert!(config
            .database_path
            .to_string_lossy()
            .contains("semantic.db"));
        assert_eq!(config.embedding_model, "nomic-ai/nomic-embed-code");
        assert_eq!(config.chunk_size, 512);
        assert_eq!(config.chunk_overlap, 64);
        assert_eq!(config.similarity_threshold, 0.7);
    }

    #[test]
    fn test_parser_config_default() {
        let config = ParserConfig::default();
        assert_eq!(config.min_chunk_size, 10);
        assert_eq!(config.max_chunk_size, 2000);
        assert_eq!(config.max_chunks_per_file, 100);
    }

    #[test]
    fn test_indexing_options_default() {
        let options = IndexingOptions::default();
        assert!(!options.force);
        assert!(options.glob_pattern.is_none());
        assert!(options.max_files.is_none());
    }

    #[test]
    fn test_search_query_creation() {
        let query = SearchQuery {
            text: "function test".to_string(),
            limit: 10,
            similarity_threshold: 0.8,
            language_filter: Some(Language::Rust),
        };
        assert_eq!(query.text, "function test");
        assert_eq!(query.limit, 10);
        assert_eq!(query.similarity_threshold, 0.8);
        assert_eq!(query.language_filter, Some(Language::Rust));
    }

    #[test]
    fn test_semantic_utils() {
        // Test text normalization
        let input = "  fn main() {  \n\n  println!(\"hello\");  \n  }  \n\n";
        let normalized = SemanticUtils::normalize_text(input);
        assert!(!normalized.contains("  "));
        assert!(normalized.contains("fn main()"));
        assert!(normalized.contains("println!"));

        // Test cosine similarity
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = SemanticUtils::cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 1e-6);

        // Test chunk ID generation
        let path = std::path::Path::new("test.rs");
        let id = SemanticUtils::generate_chunk_id(path, 1, 5);
        assert_eq!(id, "test.rs:1:5");
    }

    #[test]
    fn test_file_extensions_for_languages() {
        let rust_exts = SemanticUtils::get_file_extensions_for_language(&Language::Rust);
        assert!(rust_exts.contains(&"rs"));

        let python_exts = SemanticUtils::get_file_extensions_for_language(&Language::Python);
        assert!(python_exts.contains(&"py"));

        let ts_exts = SemanticUtils::get_file_extensions_for_language(&Language::TypeScript);
        assert!(ts_exts.contains(&"ts"));
    }

    #[test]
    fn test_should_index_file() {
        // Should index normal source files
        assert!(SemanticUtils::should_index_file(std::path::Path::new(
            "src/main.rs"
        )));
        assert!(SemanticUtils::should_index_file(std::path::Path::new(
            "lib/utils.py"
        )));

        // Should not index hidden files or build directories
        assert!(!SemanticUtils::should_index_file(std::path::Path::new(
            ".hidden/file.rs"
        )));
        assert!(!SemanticUtils::should_index_file(std::path::Path::new(
            "target/debug/main"
        )));
        assert!(!SemanticUtils::should_index_file(std::path::Path::new(
            "node_modules/package/index.js"
        )));
    }

    #[test]
    fn test_code_chunk_creation() {
        use std::path::PathBuf;

        let chunk = CodeChunk {
            id: "test-1".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() {}".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("abc123".to_string()),
        };

        assert_eq!(chunk.id, "test-1");
        assert_eq!(chunk.language, Language::Rust);
        assert_eq!(chunk.chunk_type, ChunkType::Function);
    }

    #[test]
    fn test_search_result_creation() {
        use std::path::PathBuf;

        let chunk = CodeChunk {
            id: "test-1".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() {}".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("abc123".to_string()),
        };

        let result = SemanticSearchResult {
            chunk,
            similarity_score: 0.95,
            excerpt: "fn main() {}".to_string(),
        };

        assert_eq!(result.similarity_score, 0.95);
        assert_eq!(result.chunk.id, "test-1");
    }

    #[test]
    fn test_indexing_stats_default() {
        let stats = IndexStats::default();
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.chunk_count, 0);
        assert_eq!(stats.embedding_count, 0);
    }
}
