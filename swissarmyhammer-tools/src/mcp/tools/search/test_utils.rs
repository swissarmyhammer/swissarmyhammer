//! Test utilities for search tools
//!
//! This module provides shared test utilities for search-related tools,
//! including configuration helpers for test execution.

#[cfg(test)]
use swissarmyhammer_search::SemanticConfig;

/// Create a unique test configuration with an isolated database
///
/// This function creates a `SemanticConfig` with a unique temporary database path
/// to ensure test isolation. Each invocation generates a fresh database in a unique
/// directory based on the current thread ID and timestamp.
///
/// # Returns
///
/// A `SemanticConfig` instance with:
/// - Unique database path in system temp directory
/// - Test embedding model from swissarmyhammer_config
/// - Standard test-appropriate chunk sizes and thresholds
///
/// # Panics
///
/// Panics if the temporary directory cannot be created.
#[cfg(test)]
pub(crate) fn create_test_semantic_config() -> SemanticConfig {
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    let thread_id = format!("{:?}", thread::current().id());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let unique_id = format!(
        "{}_{}",
        thread_id.replace("ThreadId(", "").replace(")", ""),
        timestamp
    );

    let persistent_path = std::env::temp_dir().join(format!("swissarmyhammer_test_{unique_id}"));
    std::fs::create_dir_all(&persistent_path).expect("Failed to create persistent test dir");
    let db_path = persistent_path.join("semantic.db");

    SemanticConfig {
        database_path: db_path,
        embedding_model: swissarmyhammer_config::DEFAULT_TEST_EMBEDDING_MODEL.to_string(),
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
        max_file_size_bytes: 10 * 1024 * 1024,
    }
}
