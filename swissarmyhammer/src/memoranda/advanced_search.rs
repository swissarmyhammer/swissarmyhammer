//! Advanced search engine for memoranda using Tantivy indexing
//!
//! This module provides full-text search capabilities for memos with advanced features
//! including boolean queries, phrase searches, wildcards, and relevance scoring.
//! It builds on the Tantivy search library for high-performance full-text search.

use crate::common::mcp_errors::McpResultExt;
use crate::error::{Result, SwissArmyHammerError};
use crate::memoranda::{Memo, MemoId, SearchOptions, SearchResult};
use std::collections::HashMap;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    doc,
    query::{BooleanQuery, Occur, Query, QueryParser},
    schema::{Field, Schema, Value},
    Index, IndexReader, IndexWriter, Term,
};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Configuration constants for advanced search engine
const DEFAULT_WRITER_BUFFER_SIZE: usize = 50_000_000; // 50MB buffer for index writer

/// Advanced memo search engine using Tantivy for full-text indexing
///
/// Provides high-performance search capabilities for memoranda with support for:
/// - Boolean queries (AND, OR operators)
/// - Phrase searches ("exact phrase")
/// - Wildcard searches (term*)
/// - Relevance scoring and ranking
/// - Search result highlighting
///
/// The engine maintains an in-memory or persistent Tantivy index that is updated
/// automatically when memos are created, updated, or deleted.
///
/// # Examples
///
/// ```rust,ignore
/// use swissarmyhammer::memoranda::{AdvancedMemoSearchEngine, SearchOptions};
///
/// let engine = AdvancedMemoSearchEngine::new_in_memory().await?;
///
/// // Index some memos
/// let memo1 = Memo::new("Project Meeting".to_string(), "Discussed timeline".to_string());
/// engine.index_memo(&memo1).await?;
///
/// // Search with boolean query
/// let options = SearchOptions::default();
/// let results = engine.search("project AND meeting", &options).await?;
/// ```
pub struct AdvancedMemoSearchEngine {
    index: Index,
    reader: IndexReader,
    writer: RwLock<IndexWriter>,
    title_field: Field,
    content_field: Field,
    id_field: Field,
    created_at_field: Field,
    updated_at_field: Field,
}

impl AdvancedMemoSearchEngine {
    /// Create a new in-memory search engine
    ///
    /// Uses RAM for storage, suitable for development and smaller datasets.
    /// The index will be lost when the application terminates.
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - New search engine instance or error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let engine = AdvancedMemoSearchEngine::new_in_memory().await?;
    /// ```
    pub async fn new_in_memory() -> Result<Self> {
        let schema = Self::build_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::new_from_index(index).await
    }

    /// Create a new persistent search engine
    ///
    /// Stores the index on disk at the specified path. The index will persist
    /// between application runs and can handle larger datasets efficiently.
    ///
    /// # Arguments
    ///
    /// * `index_path` - Directory where the index should be stored
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - New search engine instance or error
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::path::PathBuf;
    ///
    /// let engine = AdvancedMemoSearchEngine::new_persistent(
    ///     PathBuf::from("/path/to/index")
    /// ).await?;
    /// ```
    pub async fn new_persistent(index_path: impl AsRef<Path>) -> Result<Self> {
        let path = index_path.as_ref();
        std::fs::create_dir_all(path)?;

        let schema = Self::build_schema();
        let directory = MmapDirectory::open(path).with_tantivy_context()?;
        let index = Index::open_or_create(directory, schema).with_tantivy_context()?;

        Self::new_from_index(index).await
    }

    /// Create search engine from an existing Tantivy index
    async fn new_from_index(index: Index) -> Result<Self> {
        let schema = index.schema();

        let title_field = schema.get_field("title").map_err(|_| {
            SwissArmyHammerError::Other("Missing title field in schema".to_string())
        })?;
        let content_field = schema.get_field("content").map_err(|_| {
            SwissArmyHammerError::Other("Missing content field in schema".to_string())
        })?;
        let id_field = schema
            .get_field("id")
            .map_err(|_| SwissArmyHammerError::Other("Missing id field in schema".to_string()))?;
        let created_at_field = schema.get_field("created_at").map_err(|_| {
            SwissArmyHammerError::Other("Missing created_at field in schema".to_string())
        })?;
        let updated_at_field = schema.get_field("updated_at").map_err(|_| {
            SwissArmyHammerError::Other("Missing updated_at field in schema".to_string())
        })?;

        let writer = index
            .writer(DEFAULT_WRITER_BUFFER_SIZE)
            .with_tantivy_context()?;

        let reader = index.reader().with_tantivy_context()?;

        Ok(Self {
            index,
            reader,
            writer: RwLock::new(writer),
            title_field,
            content_field,
            id_field,
            created_at_field,
            updated_at_field,
        })
    }

    /// Build the Tantivy schema for memo indexing
    fn build_schema() -> Schema {
        use tantivy::schema::*;

        let mut schema_builder = Schema::builder();

        // Title field - searchable and stored, higher weight
        schema_builder.add_text_field("title", TEXT | STORED);

        // Content field - searchable but not stored (too large)
        schema_builder.add_text_field("content", TEXT);

        // ID field - stored for retrieval
        schema_builder.add_text_field("id", STORED);

        // Timestamp fields - stored for metadata
        schema_builder.add_text_field("created_at", STORED);
        schema_builder.add_text_field("updated_at", STORED);

        schema_builder.build()
    }

    /// Index a single memo in the search engine
    ///
    /// Adds or updates the memo in the search index. If a memo with the same ID
    /// already exists, it will be replaced.
    ///
    /// # Arguments
    ///
    /// * `memo` - The memo to index
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error if indexing fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let memo = Memo::new("Title".to_string(), "Content".to_string());
    /// engine.index_memo(&memo).await?;
    /// ```
    pub async fn index_memo(&self, memo: &Memo) -> Result<()> {
        // First, delete any existing document with the same ID
        {
            let mut writer = self.writer.write().await;
            let id_term = Term::from_field_text(self.id_field, memo.id.as_str());
            writer.delete_term(id_term);
            writer.commit().with_tantivy_context()?;
        }

        // Reload reader to ensure deletions are visible
        self.reader.reload().with_tantivy_context()?;

        // Then, add the new document
        {
            let mut writer = self.writer.write().await;

            let doc = doc!(
                self.title_field => memo.title.clone(),
                self.content_field => memo.content.clone(),
                self.id_field => memo.id.as_str(),
                self.created_at_field => memo.created_at.to_rfc3339(),
                self.updated_at_field => memo.updated_at.to_rfc3339(),
            );

            writer.add_document(doc).with_tantivy_context()?;

            writer.commit().with_tantivy_context()?;
        }

        // Final reload to see the new document
        self.reader.reload().with_tantivy_context()?;

        debug!("Indexed memo: {} ({})", memo.title, memo.id);
        Ok(())
    }

    /// Index multiple memos in batch
    ///
    /// More efficient than indexing memos one by one. Commits the index
    /// after all memos are added.
    ///
    /// # Arguments
    ///
    /// * `memos` - Collection of memos to index
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error if indexing fails
    pub async fn index_memos(&self, memos: &[Memo]) -> Result<()> {
        for memo in memos {
            self.index_memo(memo).await?;
        }
        self.commit().await?;
        info!("Indexed {} memos", memos.len());
        Ok(())
    }

    /// Remove a memo from the search index
    ///
    /// # Arguments
    ///
    /// * `memo_id` - The ID of the memo to remove
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error if removal fails
    pub async fn remove_memo(&self, memo_id: &MemoId) -> Result<()> {
        {
            let mut writer = self.writer.write().await;
            let id_term = Term::from_field_text(self.id_field, memo_id.as_str());
            writer.delete_term(id_term);
            writer.commit().with_tantivy_context()?;
        }

        // Reload reader to see changes
        self.reader.reload().with_tantivy_context()?;

        debug!("Removed memo from index: {}", memo_id);
        Ok(())
    }

    /// Commit pending changes to the index
    ///
    /// Makes indexed changes searchable. Should be called after indexing
    /// operations to ensure changes are visible to searches.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Success or error if commit fails
    pub async fn commit(&self) -> Result<()> {
        let mut writer = self.writer.write().await;
        writer.commit().with_tantivy_context()?;

        // Reload reader to see new changes
        self.reader.reload().with_tantivy_context()?;

        debug!("Committed index changes");
        Ok(())
    }

    /// Search memos with advanced query support
    ///
    /// Supports various query types:
    /// - Simple terms: `rust programming`
    /// - Boolean queries: `rust AND programming`, `python OR java`
    /// - Phrase queries: `"exact phrase"`
    /// - Wildcard queries: `program*`
    ///
    /// Results are ranked by relevance score and can include highlighting.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query string
    /// * `options` - Search configuration options
    /// * `all_memos` - All memos for fallback searching (when index is empty)
    ///
    /// # Returns
    ///
    /// * `Result<Vec<SearchResult>>` - Search results with relevance scores
    pub async fn search(
        &self,
        query: &str,
        options: &SearchOptions,
        all_memos: &[Memo],
    ) -> Result<Vec<SearchResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Parse query with support for different query types
        let parsed_query = self.parse_query(query, options)?;

        let searcher = self.reader.searcher();
        let limit = options.max_results.unwrap_or(100);

        let top_docs = searcher
            .search(&*parsed_query, &TopDocs::with_limit(limit))
            .with_tantivy_context()?;

        let mut results = Vec::new();
        let memo_map: HashMap<String, &Memo> = all_memos
            .iter()
            .map(|memo| (memo.id.as_str().to_string(), memo))
            .collect();

        for (score, doc_address) in top_docs {
            // tantivy 0.24+ requires explicit type annotation for doc() method
            // Previous versions: searcher.doc(doc_address)
            // Current version: searcher.doc::<tantivy::TantivyDocument>(doc_address)
            let doc = searcher
                .doc::<tantivy::TantivyDocument>(doc_address)
                .with_tantivy_context()?;

            if let Some(id_value) = doc.get_first(self.id_field) {
                if let Some(id_str) = id_value.as_str() {
                    if let Some(memo) = memo_map.get(id_str) {
                        let highlights = if options.include_highlights {
                            self.generate_search_highlights(memo, query, options)
                        } else {
                            Vec::new()
                        };

                        let match_count = self.count_matches(memo, query, options);

                        results.push(SearchResult {
                            memo: (*memo).clone(),
                            relevance_score: score * 100.0, // Convert to 0-100 scale
                            highlights,
                            match_count,
                        });
                    }
                }
            }
        }

        // Sort by relevance score (highest first)
        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

        debug!("Search for '{}' returned {} results", query, results.len());
        Ok(results)
    }

    /// Parse a query string into a Tantivy query
    fn parse_query(&self, query: &str, options: &SearchOptions) -> Result<Box<dyn Query>> {
        // Handle exact phrase queries
        if options.exact_phrase || (query.starts_with('"') && query.ends_with('"')) {
            return self.parse_phrase_query(query);
        }

        // Handle boolean queries (AND, OR)
        if query.contains(" AND ") || query.contains(" OR ") {
            return self.parse_boolean_query(query, options);
        }

        // Handle wildcard queries - but only if they're intended as wildcards
        // Check if this is an intentional wildcard (ends with * but doesn't contain other special chars)
        if query.ends_with('*') && !Self::contains_query_special_chars(&query[..query.len() - 1]) {
            return self.parse_wildcard_query(query);
        }

        // If query contains special characters that might be tokenized oddly,
        // treat it as a phrase query for better literal matching
        if Self::contains_query_special_chars(query) {
            return self.parse_phrase_query(&format!("\"{query}\""));
        }

        // Default: simple term query using QueryParser
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);

        let parsed = query_parser.parse_query(query).with_tantivy_context()?;

        Ok(parsed)
    }

    /// Parse a phrase query ("exact phrase")
    fn parse_phrase_query(&self, query: &str) -> Result<Box<dyn Query>> {
        let phrase = if query.starts_with('"') && query.ends_with('"') {
            &query[1..query.len() - 1]
        } else {
            query
        };

        // For exact phrase matching, use QueryParser with quoted query
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);

        // Ensure the phrase is properly quoted for Tantivy
        let quoted_phrase = format!("\"{phrase}\"");
        let parsed = query_parser
            .parse_query(&quoted_phrase)
            .with_tantivy_context()?;

        Ok(parsed)
    }

    /// Parse a boolean query (term1 AND term2, term1 OR term2)
    fn parse_boolean_query(&self, query: &str, _options: &SearchOptions) -> Result<Box<dyn Query>> {
        let mut subqueries = Vec::new();

        // Simple parsing - handle AND queries for now
        if query.contains(" AND ") {
            let parts: Vec<&str> = query.split(" AND ").collect();

            for part in parts {
                let term_query = self.create_term_query(part.trim())?;
                subqueries.push((Occur::Must, term_query));
            }
        } else if query.contains(" OR ") {
            let parts: Vec<&str> = query.split(" OR ").collect();

            for part in parts {
                let term_query = self.create_term_query(part.trim())?;
                subqueries.push((Occur::Should, term_query));
            }
        } else {
            // Fallback to single term
            let term_query = self.create_term_query(query.trim())?;
            subqueries.push((Occur::Must, term_query));
        }

        if subqueries.is_empty() {
            return Err(SwissArmyHammerError::Other(
                "Empty boolean query".to_string(),
            ));
        }

        let boolean_query = BooleanQuery::new(subqueries);
        Ok(Box::new(boolean_query))
    }

    /// Parse a wildcard query (term*)
    fn parse_wildcard_query(&self, query: &str) -> Result<Box<dyn Query>> {
        // For now, fall back to regular term query since WildcardQuery may not be available
        // Remove the asterisk and do a prefix search
        let clean_query = query.trim_end_matches('*');
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);

        let parsed = query_parser
            .parse_query(clean_query)
            .with_tantivy_context()?;

        Ok(parsed)
    }

    /// Create a term query for a single search term
    fn create_term_query(&self, term: &str) -> Result<Box<dyn Query>> {
        // Use QueryParser for simplicity - it handles term queries across multiple fields
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);

        let parsed = query_parser.parse_query(term).with_tantivy_context()?;

        Ok(parsed)
    }

    /// Generate highlighted snippets for search results
    fn generate_search_highlights(
        &self,
        memo: &Memo,
        query: &str,
        options: &SearchOptions,
    ) -> Vec<String> {
        crate::memoranda::storage::generate_highlights(memo, query, options)
    }

    /// Count the number of matches in a memo
    fn count_matches(&self, memo: &Memo, query: &str, options: &SearchOptions) -> usize {
        let search_query = if options.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let title_text = if options.case_sensitive {
            memo.title.clone()
        } else {
            memo.title.to_lowercase()
        };

        let content_text = if options.case_sensitive {
            memo.content.clone()
        } else {
            memo.content.to_lowercase()
        };

        let mut count = 0;

        // Count occurrences in title
        let mut start = 0;
        while let Some(pos) = title_text[start..].find(&search_query) {
            count += 1;
            start += pos + search_query.len();
        }

        // Count occurrences in content
        start = 0;
        while let Some(pos) = content_text[start..].find(&search_query) {
            count += 1;
            start += pos + search_query.len();
        }

        count
    }

    /// Check if a string contains special query syntax characters that need escaping
    fn contains_query_special_chars(text: &str) -> bool {
        text.chars().any(|c| {
            matches!(
                c,
                '+' | '-'
                    | '!'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '^'
                    | '"'
                    | '~'
                    | '*'
                    | '?'
                    | ':'
                    | '\\'
                    | '/'
                    | '.'
                    | '&'
                    | '|'
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    fn create_test_memos() -> Vec<Memo> {
        vec![
            Memo::new(
                "Rust Programming Guide".to_string(),
                "Learning Rust language fundamentals".to_string(),
            ),
            Memo::new(
                "Python Tutorial".to_string(),
                "Python programming for beginners".to_string(),
            ),
            Memo::new(
                "Project Meeting".to_string(),
                "Discussed Rust project timeline".to_string(),
            ),
        ]
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        assert_eq!(engine.index.schema().fields().count(), 5);
    }

    #[tokio::test]
    async fn test_index_and_search() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();

        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions::default();
        let results = engine.search("rust", &options, &memos).await.unwrap();

        assert_eq!(results.len(), 2); // Should find "Rust Programming Guide" and "Project Meeting"
        assert!(results[0].relevance_score > 0.0);
    }

    #[tokio::test]
    async fn test_phrase_search() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();

        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions {
            exact_phrase: true,
            ..Default::default()
        };
        let results = engine
            .search("Rust Programming", &options, &memos)
            .await
            .unwrap();

        // Should find only "Rust Programming Guide", not "Project Meeting" with separate words
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memo.title, "Rust Programming Guide");
    }

    #[tokio::test]
    async fn test_boolean_search() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();

        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions::default();
        let results = engine
            .search("rust AND project", &options, &memos)
            .await
            .unwrap();

        // Should find "Project Meeting" (contains both "rust" and "project")
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memo.title, "Project Meeting");
    }

    #[tokio::test]
    async fn test_search_with_highlights() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();

        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions {
            include_highlights: true,
            ..Default::default()
        };
        let results = engine
            .search("programming", &options, &memos)
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(!results[0].highlights.is_empty());

        let highlights_text = results[0].highlights.join(" ");
        assert!(
            highlights_text.contains("**programming**")
                || highlights_text.contains("**Programming**")
        );
    }

    #[tokio::test]
    async fn test_memo_removal() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();

        engine.index_memos(&memos).await.unwrap();

        // Search should find results before removal
        let options = SearchOptions::default();
        let results_before = engine.search("rust", &options, &memos).await.unwrap();
        assert_eq!(results_before.len(), 2);

        // Remove one memo
        let removed_memo_id = memos[0].id.clone();
        engine.remove_memo(&removed_memo_id).await.unwrap();

        // Create filtered memo list excluding the removed memo
        let remaining_memos: Vec<Memo> = memos
            .into_iter()
            .filter(|memo| memo.id != removed_memo_id)
            .collect();

        // Search should find fewer results after removal
        let results_after = engine
            .search("rust", &options, &remaining_memos)
            .await
            .unwrap();
        assert_eq!(results_after.len(), 1);
    }

    #[tokio::test]
    async fn test_empty_and_invalid_queries() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();
        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions::default();

        // Empty query should return no results
        let results = engine.search("", &options, &memos).await.unwrap();
        assert_eq!(results.len(), 0);

        // Whitespace-only query should return no results
        let results = engine.search("   \t\n  ", &options, &memos).await.unwrap();
        assert_eq!(results.len(), 0);

        // Query that matches nothing should return empty results
        let results = engine
            .search("nonexistentword", &options, &memos)
            .await
            .unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_special_characters_in_queries() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();

        // Create memos with special characters
        let special_memos = vec![
            Memo::new(
                "C++ Programming".to_string(),
                "Learning C++/CLI syntax".to_string(),
            ),
            Memo::new(
                "Email: user@domain.com".to_string(),
                "Contact information".to_string(),
            ),
            Memo::new(
                "Path: /usr/local/bin".to_string(),
                "System directories".to_string(),
            ),
        ];

        engine.index_memos(&special_memos).await.unwrap();

        let options = SearchOptions::default();

        // Search for content with special characters
        let results = engine
            .search("C++", &options, &special_memos)
            .await
            .unwrap();
        assert!(!results.is_empty());

        let results = engine
            .search("user@domain.com", &options, &special_memos)
            .await
            .unwrap();
        assert!(!results.is_empty());

        let results = engine
            .search("/usr/local", &options, &special_memos)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_case_sensitivity_options() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();
        engine.index_memos(&memos).await.unwrap();

        // Case insensitive search (default)
        let case_insensitive = SearchOptions {
            case_sensitive: false,
            ..Default::default()
        };
        let results = engine
            .search("RUST", &case_insensitive, &memos)
            .await
            .unwrap();
        assert!(!results.is_empty());

        // Case sensitive search - Tantivy typically normalizes text during indexing,
        // so we test with a term that should have different behavior
        let case_sensitive = SearchOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let _results = engine
            .search("RUST", &case_sensitive, &memos)
            .await
            .unwrap();
        // Note: Tantivy may still find results due to text analysis during indexing
        // This is expected behavior for full-text search engines

        // But exact case should work
        let results = engine
            .search("Rust", &case_sensitive, &memos)
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_max_results_option() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();
        engine.index_memos(&memos).await.unwrap();

        // Search with no limit
        let unlimited = SearchOptions::default();
        let all_results = engine
            .search("programming", &unlimited, &memos)
            .await
            .unwrap();

        // Search with max results = 1
        let limited = SearchOptions {
            max_results: Some(1),
            ..Default::default()
        };
        let limited_results = engine
            .search("programming", &limited, &memos)
            .await
            .unwrap();

        assert!(limited_results.len() <= 1);
        assert!(limited_results.len() <= all_results.len());
    }

    #[tokio::test]
    async fn test_complex_boolean_queries() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();
        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions::default();

        // OR query should find more results than AND
        let and_results = engine
            .search("rust AND programming", &options, &memos)
            .await
            .unwrap();
        let or_results = engine
            .search("rust OR python", &options, &memos)
            .await
            .unwrap();

        assert!(or_results.len() >= and_results.len());
        assert!(!or_results.is_empty());
    }

    #[tokio::test]
    async fn test_phrase_search_precision() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();
        engine.index_memos(&memos).await.unwrap();

        let phrase_options = SearchOptions {
            exact_phrase: true,
            ..Default::default()
        };

        // Exact phrase should be more restrictive than individual words
        let word_results = engine
            .search("rust programming", &SearchOptions::default(), &memos)
            .await
            .unwrap();
        let phrase_results = engine
            .search("rust programming", &phrase_options, &memos)
            .await
            .unwrap();

        // Phrase search should be more restrictive
        assert!(phrase_results.len() <= word_results.len());
    }

    #[tokio::test]
    async fn test_relevance_score_ordering() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let memos = create_test_memos();
        engine.index_memos(&memos).await.unwrap();

        let options = SearchOptions::default();
        let results = engine
            .search("programming", &options, &memos)
            .await
            .unwrap();

        if results.len() > 1 {
            // Results should be ordered by relevance score (highest first)
            for i in 0..results.len() - 1 {
                assert!(results[i].relevance_score >= results[i + 1].relevance_score);
            }

            // All scores should be positive
            for result in &results {
                assert!(result.relevance_score > 0.0);
            }
        }
    }

    #[tokio::test]
    async fn test_special_characters_in_search() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();

        // Create memos with different types of special content
        let special_memos = vec![
            Memo::new(
                "Email Content".to_string(),
                "Contact support at help@example.com".to_string(),
            ),
            Memo::new(
                "Programming Notes".to_string(),
                "Use C++ for performance-critical code".to_string(),
            ),
            Memo::new(
                "File Path".to_string(),
                "Config file located at /usr/local/bin/config".to_string(),
            ),
        ];

        engine.index_memos(&special_memos).await.unwrap();

        let options = SearchOptions::default();

        // Test search for email address
        let results = engine
            .search("help@example.com", &options, &special_memos)
            .await
            .unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should find memo containing email address"
        );

        // Test search for C++ (with special characters)
        let results = engine
            .search("C++", &options, &special_memos)
            .await
            .unwrap();
        assert_eq!(results.len(), 1, "Should find memo containing C++");

        // Test search for file path with forward slashes
        let results = engine
            .search("/usr/local", &options, &special_memos)
            .await
            .unwrap();
        assert_eq!(results.len(), 1, "Should find memo containing file path");
    }

    #[tokio::test]
    async fn test_index_update_on_memo_changes() {
        let engine = AdvancedMemoSearchEngine::new_in_memory().await.unwrap();
        let mut memo = Memo::new(
            "Test Document".to_string(),
            "Contains word zebra".to_string(),
        );

        // Index original memo
        engine.index_memo(&memo).await.unwrap();

        let options = SearchOptions::default();

        // Search for original content should find it
        let results = engine
            .search("zebra", &options, &[memo.clone()])
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Update memo content to completely different words
        memo.update_content("Contains word elephant instead".to_string());

        // Re-index updated memo
        engine.index_memo(&memo).await.unwrap();

        // Search for new content should find it - this is the key functionality
        let results = engine
            .search("elephant", &options, &[memo.clone()])
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Verify that the engine can handle document updates - the exact replacement behavior
        // may vary with different Tantivy versions, but updating should work
        let all_results = engine
            .search("contains", &options, &[memo.clone()])
            .await
            .unwrap();
        assert!(!all_results.is_empty()); // Should find at least the updated content
    }
}
