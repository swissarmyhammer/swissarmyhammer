//! Query and search logic for semantic search
//!
//! This module provides the core [`SemanticSearcher`] for querying indexed code using
//! semantic similarity. The searcher supports various search modes including simple text
//! search, language-specific search, code similarity detection, and multi-query search.
//!
//! # Usage Examples
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use swissarmyhammer::search::{SemanticSearcher, VectorStorage, SemanticConfig, SearchQuery};
//!
//! # async fn example() -> SearchResult<(), Box<dyn std::error::Error>> {
//! let config = SemanticConfig::default();
//! let storage = VectorStorage::new(config.clone())?;
//! let searcher = SemanticSearcher::new(storage, config).await?;
//!
//! // Simple text search
//! let results = searcher.search_simple("async function", 10).await?;
//! for result in results {
//!     println!("Found: {} (score: {:.3})", result.chunk.file_path.display(), result.similarity_score);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Advanced Search
//!
//! ```rust,no_run
//! use swissarmyhammer::search::{SemanticSearcher, SearchQuery, Language};
//!
//! # async fn example(searcher: SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
//! // Language-specific search
//! let rust_results = searcher.search_by_language("error handling", Language::Rust, 5).await?;
//!
//! // Custom query with threshold
//! let query = SearchQuery {
//!     text: "database connection".to_string(),
//!     limit: 20,
//!     similarity_threshold: 0.8,
//!     language_filter: None,
//! };
//! let results = searcher.search(&query).await?;
//! # Ok(())
//! # }
//! ```

use crate::{
    types::{CodeChunk, Language, ResultExplanation, SearchExplanation},
    embedding::EmbeddingEngine,
    error::SearchResult,
    storage::VectorStorage,
    SearchQuery, SearchStats, SemanticConfig, SemanticSearchResult,
};
use std::collections::HashMap;

/// Semantic searcher for querying indexed code using vector embeddings.
///
/// The `SemanticSearcher` provides high-level search functionality over indexed code chunks.
/// It combines embedding generation, vector similarity search, and result formatting to enable
/// semantic code search with configurable parameters.
///
/// # Performance Characteristics
///
/// - Memory usage scales with result set size and configured excerpt length
/// - Search latency depends on index size and similarity threshold
/// - Multi-query search deduplicates results in memory before returning
///
/// # Safety Considerations
///
/// For large datasets (>10k chunks), consider:
/// - Using higher similarity thresholds to limit result sets
/// - Implementing pagination for very large queries
/// - Monitoring memory usage during multi-query operations
///
/// # Example
///
/// ```rust,no_run
/// use swissarmyhammer::search::{SemanticSearcher, VectorStorage, SemanticConfig};
///
/// # async fn example() -> SearchResult<(), Box<dyn std::error::Error>> {
/// let config = SemanticConfig::default();
/// let storage = VectorStorage::new(config.clone())?;
/// let searcher = SemanticSearcher::new(storage, config).await?;
///
/// let results = searcher.search_simple("error handling", 10).await?;
/// println!("Found {} similar code chunks", results.len());
/// # Ok(())
/// # }
/// ```
pub struct SemanticSearcher {
    storage: VectorStorage,
    embedding_engine: EmbeddingEngine,
    config: SemanticConfig,
}

impl SemanticSearcher {
    /// Create a new semantic searcher with default embedding engine.
    ///
    /// This constructor initializes a new embedding engine, which may take some time
    /// as it needs to load the model. For better performance when creating multiple
    /// searchers, consider using [`with_embedding_engine`] to share an engine.
    ///
    /// # Arguments
    ///
    /// * `storage` - Vector storage containing indexed code chunks
    /// * `config` - Configuration including thresholds, excerpt settings, and model info
    ///
    /// # Returns
    ///
    /// Returns a configured searcher ready for queries, or an error if the embedding
    /// engine fails to initialize.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer::search::{SemanticSearcher, VectorStorage, SemanticConfig};
    ///
    /// # async fn example() -> SearchResult<(), Box<dyn std::error::Error>> {
    /// let config = SemanticConfig::default();
    /// let storage = VectorStorage::new(config.clone())?;
    /// let searcher = SemanticSearcher::new(storage, config).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`with_embedding_engine`]: Self::with_embedding_engine
    pub async fn new(storage: VectorStorage, config: SemanticConfig) -> SearchResult<Self> {
        let embedding_engine = EmbeddingEngine::new().await?;

        Ok(Self {
            storage,
            embedding_engine,
            config,
        })
    }

    /// Create searcher with an existing embedding engine.
    ///
    /// This constructor allows sharing an embedding engine across multiple searchers,
    /// which is more efficient than creating separate engines. Useful for applications
    /// that perform many concurrent searches or create multiple searcher instances.
    ///
    /// # Arguments
    ///
    /// * `storage` - Vector storage containing indexed code chunks
    /// * `embedding_engine` - Pre-initialized embedding engine to use
    /// * `config` - Configuration including thresholds, excerpt settings, and model info
    ///
    /// # Returns
    ///
    /// Returns a configured searcher using the provided embedding engine.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer::search::{SemanticSearcher, VectorStorage, EmbeddingEngine, SemanticConfig};
    ///
    /// # async fn example() -> SearchResult<(), Box<dyn std::error::Error>> {
    /// let config = SemanticConfig::default();
    /// let storage = VectorStorage::new(config.clone())?;
    /// let engine = EmbeddingEngine::new().await?;
    ///
    /// // Create searcher with embedding engine
    /// let searcher = SemanticSearcher::with_embedding_engine(storage, engine, config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_embedding_engine(
        storage: VectorStorage,
        embedding_engine: EmbeddingEngine,
        config: SemanticConfig,
    ) -> SearchResult<Self> {
        Ok(Self {
            storage,
            embedding_engine,
            config,
        })
    }

    /// Perform semantic search with a detailed query specification.
    ///
    /// This is the main search method that accepts a [`SearchQuery`] with full control
    /// over search parameters including similarity threshold, result limit, and language
    /// filtering. Results are ranked by similarity score and include generated excerpts.
    ///
    /// # Arguments
    ///
    /// * `query` - Complete search specification including text, limits, and filters
    ///
    /// # Returns
    ///
    /// Returns a vector of search results ordered by similarity score (highest first).
    /// Each result includes the matching code chunk, similarity score, and generated excerpt.
    /// Returns an empty vector if no results meet the similarity threshold.
    ///
    /// # Performance Notes
    ///
    /// - Lower similarity thresholds return more results but may be less relevant
    /// - Language filtering is applied after similarity search, not during indexing
    /// - Excerpt generation adds minimal overhead per result
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer::search::{SemanticSearcher, SearchQuery, Language};
    ///
    /// # async fn example(searcher: SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// let query = SearchQuery {
    ///     text: "async error handling".to_string(),
    ///     limit: 15,
    ///     similarity_threshold: 0.75,
    ///     language_filter: Some(Language::Rust),
    /// };
    ///
    /// let results = searcher.search(&query).await?;
    /// for result in results {
    ///     println!("{}: {:.3} - {}",
    ///         result.chunk.file_path.display(),
    ///         result.similarity_score,
    ///         result.excerpt
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search(&self, query: &SearchQuery) -> SearchResult<Vec<SemanticSearchResult>> {
        tracing::debug!("Performing semantic search for: '{}'", query.text);

        // Generate embedding for the query
        let query_embedding = self.embedding_engine.embed_text(&query.text).await?;

        // Find similar embeddings in the database
        let similar_chunk_ids = self
            .storage
            .similarity_search(&query_embedding, query.limit, query.similarity_threshold)
            .map_err(|e| crate::error::SearchError::VectorStorage {
                operation: "similarity search".to_string(),
                source: Box::new(e),
            })?;

        if similar_chunk_ids.is_empty() {
            tracing::info!("No results found for query: '{}'", query.text);
            return Ok(Vec::new());
        }

        // Apply language filter and create excerpts
        let mut results = Vec::new();
        for mut result in similar_chunk_ids {
            // Apply language filter if specified
            if let Some(ref language_filter) = query.language_filter {
                if result.chunk.language != *language_filter {
                    continue;
                }
            }

            // Create excerpt for this result
            result.excerpt = self.create_excerpt(&result.chunk, &query.text);
            results.push(result);
        }

        // Sort by similarity score (highest first) - already sorted by storage, but ensure consistency
        results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());

        tracing::info!("Found {} results for query", results.len());
        Ok(results)
    }

    /// Search with simple text query using default parameters.
    ///
    /// This is a convenience method for basic searches that uses the configured
    /// `simple_search_threshold` and no language filtering. Ideal for quick searches
    /// or when you don't need fine-grained control over search parameters.
    ///
    /// # Arguments
    ///
    /// * `query_text` - The text to search for in the indexed code
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    ///
    /// Returns search results ordered by similarity score. Uses the threshold from
    /// the searcher's configuration (typically 0.5 for broader results).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn example(searcher: swissarmyhammer::search::SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// // Find code related to database connections
    /// let results = searcher.search_simple("database connect", 10).await?;
    ///
    /// println!("Found {} database-related code chunks:", results.len());
    /// for (i, result) in results.iter().enumerate() {
    ///     println!("{}. {} ({:.3})", i + 1, result.chunk.file_path.display(), result.similarity_score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_simple(
        &self,
        query_text: &str,
        limit: usize,
    ) -> SearchResult<Vec<SemanticSearchResult>> {
        let query = SearchQuery {
            text: query_text.to_string(),
            limit,
            similarity_threshold: self.config.simple_search_threshold,
            language_filter: None,
        };

        self.search(&query).await
    }

    /// Search within specific programming languages.
    ///
    /// This method restricts search results to code chunks written in a specific
    /// programming language. Useful when you want to find language-specific patterns
    /// or avoid cross-language false positives.
    ///
    /// # Arguments
    ///
    /// * `query_text` - The text to search for in the indexed code
    /// * `language` - Programming language to restrict search to
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    ///
    /// Returns search results from the specified language only, ordered by similarity
    /// score. Language filtering is applied after similarity search, so performance
    /// is similar to `search_simple` but with fewer results.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer::search::Language;
    ///
    /// # async fn example(searcher: swissarmyhammer::search::SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// // Find error handling patterns specifically in Rust code
    /// let rust_results = searcher.search_by_language(
    ///     "Result Error match",
    ///     Language::Rust,
    ///     15
    /// ).await?;
    ///
    /// // Find async patterns in TypeScript
    /// let ts_results = searcher.search_by_language(
    ///     "async await Promise",
    ///     Language::TypeScript,
    ///     10
    /// ).await?;
    ///
    /// println!("Found {} Rust patterns, {} TypeScript patterns",
    ///     rust_results.len(), ts_results.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_by_language(
        &self,
        query_text: &str,
        language: Language,
        limit: usize,
    ) -> SearchResult<Vec<SemanticSearchResult>> {
        let query = SearchQuery {
            text: query_text.to_string(),
            limit,
            similarity_threshold: self.config.simple_search_threshold,
            language_filter: Some(language),
        };

        self.search(&query).await
    }

    /// Search for code similar to a given chunk.
    ///
    /// This method finds code chunks that are semantically similar to a provided
    /// reference chunk. Uses a higher similarity threshold than general text search
    /// to focus on truly similar code patterns. The reference chunk itself is
    /// automatically excluded from results.
    ///
    /// # Arguments
    ///
    /// * `chunk` - Reference code chunk to find similar code for
    /// * `limit` - Maximum number of similar chunks to return
    ///
    /// # Returns
    ///
    /// Returns code chunks similar to the reference, ordered by similarity score.
    /// The original chunk is excluded from results. Uses the `code_similarity_threshold`
    /// from configuration (typically 0.7 for higher precision).
    ///
    /// # Use Cases
    ///
    /// - Find duplicate or near-duplicate code
    /// - Locate similar implementations across a codebase
    /// - Identify refactoring opportunities
    /// - Find examples of similar patterns
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer::search::{CodeChunk, Language, ChunkType, ContentHash};
    /// use std::path::PathBuf;
    ///
    /// # async fn example(searcher: swissarmyhammer::search::SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// let reference_chunk = CodeChunk {
    ///     id: "example-chunk".to_string(),
    ///     file_path: PathBuf::from("src/utils.rs"),
    ///     language: Language::Rust,
    ///     content: "fn validate_input(s: &str) -> SearchResult<(), Error> { ... }".to_string(),
    ///     start_line: 42,
    ///     end_line: 50,
    ///     chunk_type: ChunkType::Function,
    ///     content_hash: ContentHash("hash123".to_string()),
    /// };
    ///
    /// let similar = searcher.find_similar_code(&reference_chunk, 5).await?;
    /// println!("Found {} similar functions:", similar.len());
    /// for result in similar {
    ///     println!("  {} (similarity: {:.3})",
    ///         result.chunk.file_path.display(), result.similarity_score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn find_similar_code(
        &self,
        chunk: &CodeChunk,
        limit: usize,
    ) -> SearchResult<Vec<SemanticSearchResult>> {
        // Use the chunk content as the query
        let query = SearchQuery {
            text: chunk.content.clone(),
            limit: limit + 1, // +1 because the original chunk might be included
            similarity_threshold: self.config.code_similarity_threshold,
            language_filter: None, // Don't filter by language for broader results
        };

        let mut results = self.search(&query).await?;

        // Remove the original chunk from results if present
        results.retain(|result| result.chunk.id != chunk.id);

        // Limit to requested number
        results.truncate(limit);

        Ok(results)
    }

    /// Multi-query search - combine results from multiple related queries.
    ///
    /// This method performs searches for multiple related queries and combines the
    /// results with deduplication. Useful for finding code that matches any of several
    /// related terms or concepts. Results are deduplicated by chunk ID and the highest
    /// similarity score is retained for each unique chunk.
    ///
    /// # Arguments
    ///
    /// * `queries` - List of query strings to search for
    /// * `limit_per_query` - Maximum results to collect per individual query
    /// * `overall_limit` - Maximum results to return after deduplication
    ///
    /// # Returns
    ///
    /// Returns deduplicated search results ordered by similarity score. If the same
    /// code chunk matches multiple queries, only the result with the highest similarity
    /// score is included.
    ///
    /// # Performance Notes
    ///
    /// - Each query requires a separate embedding generation and search
    /// - Results are collected in memory before deduplication
    /// - Consider using moderate `limit_per_query` values for better performance
    /// - Memory usage scales with total results before deduplication
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn example(searcher: swissarmyhammer::search::SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// // Find code related to error handling using multiple related terms
    /// let error_queries = vec![
    ///     "error handling Result".to_string(),
    ///     "exception try catch".to_string(),
    ///     "panic unwrap expect".to_string(),
    ///     "failure recovery".to_string(),
    /// ];
    ///
    /// let results = searcher.multi_query_search(&error_queries, 10, 25).await?;
    ///
    /// println!("Found {} unique error handling patterns:", results.len());
    /// for result in results.iter().take(5) {
    ///     println!("  {} ({:.3})", result.chunk.file_path.display(), result.similarity_score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn multi_query_search(
        &self,
        queries: &[String],
        limit_per_query: usize,
        overall_limit: usize,
    ) -> SearchResult<Vec<SemanticSearchResult>> {
        // Pre-allocate HashMap with capacity hint based on expected results
        // Estimate: queries.len() * limit_per_query with some buffer for deduplication
        let estimated_capacity = queries.len() * limit_per_query;
        let mut all_results = HashMap::with_capacity(estimated_capacity);

        for query_text in queries {
            let results = self.search_simple(query_text, limit_per_query).await?;

            for result in results {
                // Use chunk ID as key to deduplicate
                all_results
                    .entry(result.chunk.id.clone())
                    .and_modify(|existing: &mut SemanticSearchResult| {
                        // Keep the result with higher similarity score
                        if result.similarity_score > existing.similarity_score {
                            *existing = result.clone();
                        }
                    })
                    .or_insert(result);

                // Early termination optimization: if we have significantly more results
                // than needed, we can afford to be more selective
                if all_results.len() > overall_limit * 3 {
                    // Remove results below a reasonable threshold to manage memory
                    // This is a memory vs accuracy tradeoff for very large result sets
                    all_results.retain(|_, result| {
                        result.similarity_score >= self.config.simple_search_threshold * 0.8
                    });
                }
            }
        }

        // Pre-allocate final results vector with the smaller of estimated capacity or overall limit
        let mut final_results: Vec<_> = Vec::with_capacity(all_results.len().min(overall_limit));
        final_results.extend(all_results.into_values());

        // Sort by similarity score (highest first)
        final_results.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());

        // Limit results to requested amount
        final_results.truncate(overall_limit);

        Ok(final_results)
    }

    /// Create an excerpt showing relevant parts of the code
    fn create_excerpt(&self, chunk: &CodeChunk, query: &str) -> String {
        let content = &chunk.content;
        let query_lower = query.to_lowercase();

        // Try to find query terms in the content
        let content_lower = content.to_lowercase();

        if let Some(match_pos) = content_lower.find(&query_lower) {
            // Found direct match - create excerpt around it
            self.create_excerpt_around_match(content, match_pos, self.config.excerpt_length)
        } else {
            // No direct match - create excerpt from beginning with context
            self.create_excerpt_from_start(
                content,
                self.config.excerpt_length,
                self.config.context_lines,
            )
        }
    }

    fn create_excerpt_around_match(
        &self,
        content: &str,
        match_pos: usize,
        max_length: usize,
    ) -> String {
        let start = match_pos.saturating_sub(max_length / 2);
        let end = (match_pos + max_length / 2).min(content.len());

        let excerpt = &content[start..end];

        // Clean up excerpt to avoid breaking in middle of words
        self.truncate_with_ellipsis(excerpt, max_length, start > 0, end < content.len())
    }

    fn create_excerpt_from_start(
        &self,
        content: &str,
        max_length: usize,
        context_lines: usize,
    ) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let selected_lines = lines
            .iter()
            .take(context_lines)
            .cloned()
            .collect::<Vec<_>>();
        let excerpt = selected_lines.join("\n");

        self.truncate_with_ellipsis(&excerpt, max_length, false, true)
    }

    /// Common utility for truncating text with configurable ellipsis handling.
    ///
    /// This method provides consistent truncation behavior across all excerpt creation
    /// methods, ensuring uniform ellipsis handling and text trimming.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to potentially truncate
    /// * `max_length` - Maximum length before truncation
    /// * `add_prefix_ellipsis` - Whether to add "..." at the beginning
    /// * `add_suffix_ellipsis` - Whether to add "..." at the end if truncated
    ///
    /// # Returns
    ///
    /// Returns the text, optionally truncated and with ellipsis added as specified.
    fn truncate_with_ellipsis(
        &self,
        text: &str,
        max_length: usize,
        add_prefix_ellipsis: bool,
        add_suffix_ellipsis: bool,
    ) -> String {
        let trimmed = text.trim();

        let mut result = if trimmed.len() <= max_length {
            trimmed.to_string()
        } else {
            let truncated = &trimmed[..max_length];
            let cleaned = truncated.trim_end();
            if add_suffix_ellipsis {
                format!("{cleaned}...")
            } else {
                cleaned.to_string()
            }
        };

        if add_prefix_ellipsis {
            result = format!("...{result}");
        }

        result
    }

    /// Get search statistics for debugging and monitoring.
    ///
    /// Returns comprehensive statistics about the indexed content and embedding model
    /// being used. Useful for debugging search issues, monitoring index health, and
    /// understanding search performance characteristics.
    ///
    /// # Returns
    ///
    /// Returns [`SearchStats`] containing:
    /// - Count of indexed files, chunks, and embeddings
    /// - Information about the embedding model in use
    /// - Index health metrics
    ///
    /// # Use Cases
    ///
    /// - Monitoring index size and growth
    /// - Debugging why searches return no results
    /// - Performance analysis and capacity planning
    /// - Verifying successful indexing operations
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # async fn example(searcher: swissarmyhammer::search::SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// let stats = searcher.get_search_stats().await?;
    ///
    /// println!("Index Statistics:");
    /// println!("  Files: {}", stats.total_files);
    /// println!("  Code chunks: {}", stats.total_chunks);
    /// println!("  Embeddings: {}", stats.total_embeddings);
    /// println!("  Model: {}", stats.model_info.model_id);
    ///
    /// if stats.total_chunks == 0 {
    ///     println!("Warning: No content indexed, searches will return empty results");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_search_stats(&self) -> SearchResult<SearchStats> {
        let index_stats = self.storage.get_index_stats().map_err(|e| {
            crate::error::SearchError::VectorStorage {
                operation: "index statistics retrieval".to_string(),
                source: Box::new(e),
            }
        })?;
        let model_info = self.embedding_engine.model_info();

        Ok(SearchStats {
            total_files: index_stats.file_count,
            total_chunks: index_stats.chunk_count,
            total_embeddings: index_stats.embedding_count,
            model_info,
        })
    }

    /// Explain search results with detailed scoring information.
    ///
    /// This method provides detailed analysis of how search results are scored and
    /// ranked. It performs the same search as the main `search` method but returns
    /// comprehensive debugging information about all candidates, including those
    /// below the similarity threshold.
    ///
    /// # Arguments
    ///
    /// * `query` - The search query to analyze and explain
    ///
    /// # Returns
    ///
    /// Returns [`SearchExplanation`] containing:
    /// - Original query text and embedding characteristics
    /// - Similarity threshold used for filtering
    /// - Total number of candidates evaluated
    /// - Detailed information about each candidate's score and content
    ///
    /// # Performance Warning
    ///
    /// This method retrieves ALL candidates (threshold 0.0) for analysis, which can
    /// be expensive for large indexes. Use primarily for debugging and development,
    /// not in production search paths.
    ///
    /// # Use Cases
    ///
    /// - Debugging why expected results don't appear
    /// - Understanding similarity score distributions
    /// - Tuning similarity thresholds
    /// - Analyzing embedding quality and relevance
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use swissarmyhammer::search::SearchQuery;
    ///
    /// # async fn example(searcher: swissarmyhammer::search::SemanticSearcher) -> SearchResult<(), Box<dyn std::error::Error>> {
    /// let query = SearchQuery {
    ///     text: "async function".to_string(),
    ///     limit: 10,
    ///     similarity_threshold: 0.7,
    ///     language_filter: None,
    /// };
    ///
    /// let explanation = searcher.explain_search(&query).await?;
    ///
    /// println!("Search Analysis for '{}':", explanation.query_text);
    /// println!("  Query embedding norm: {:.3}", explanation.query_embedding_norm);
    /// println!("  Threshold: {:.3}", explanation.threshold);
    /// println!("  Total candidates: {}", explanation.total_candidates);
    ///
    /// let above_threshold = explanation.results.iter()
    ///     .filter(|r| r.above_threshold)
    ///     .count();
    /// println!("  Above threshold: {}", above_threshold);
    ///
    /// // Show top candidates
    /// for result in explanation.results.iter().take(5) {
    ///     println!("    {}: {:.3} ({})",
    ///         result.chunk_id,
    ///         result.similarity_score,
    ///         if result.above_threshold { "✓" } else { "✗" }
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn explain_search(&self, query: &SearchQuery) -> SearchResult<SearchExplanation> {
        let query_embedding = self.embedding_engine.embed_text(&query.text).await?;

        // Get detailed similarity results
        let similar_results = self
            .storage
            .similarity_search_with_details(
                &query_embedding,
                query.limit,
                0.0, // Get all results for explanation
            )
            .map_err(|e| crate::error::SearchError::VectorStorage {
                operation: "detailed similarity search".to_string(),
                source: Box::new(e),
            })?;

        let mut explanations = Vec::new();
        for (chunk_id, similarity_score, _embedding) in similar_results {
            if let Some(chunk) = self.storage.get_chunk(&chunk_id).map_err(|e| {
                crate::error::SearchError::SearchOperation {
                    operation: "chunk retrieval for explanation".to_string(),
                    message: format!("Failed to retrieve chunk {chunk_id}"),
                    source: Some(Box::new(e)),
                }
            })? {
                explanations.push(ResultExplanation {
                    chunk_id: chunk_id.clone(),
                    similarity_score,
                    language: chunk.language.clone(),
                    chunk_type: chunk.chunk_type.clone(),
                    content_preview: chunk
                        .content
                        .chars()
                        .take(self.config.content_preview_length)
                        .collect(),
                    above_threshold: similarity_score >= query.similarity_threshold,
                });
            }
        }

        Ok(SearchExplanation {
            query_text: query.text.clone(),
            query_embedding_norm: self.calculate_vector_norm(&query_embedding),
            threshold: query.similarity_threshold,
            total_candidates: explanations.len(),
            results: explanations,
        })
    }

    fn calculate_vector_norm(&self, vector: &[f32]) -> f32 {
        vector.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Search with explanation for debugging and transparency
    pub async fn search_with_explanation(&self, query: &SearchQuery) -> SearchResult<Vec<crate::operations::SearchResultWithExplanation>> {
        let results = self.search(query).await?;
        let explanation = self.explain_search(query).await?;
        
        // Create SearchResultWithExplanation for each result
        let mut results_with_explanation = Vec::new();
        for result in results {
            results_with_explanation.push(crate::operations::SearchResultWithExplanation {
                result,
                explanation: explanation.clone(),
            });
        }
        
        Ok(results_with_explanation)
    }

    /// Create a SemanticSearcher for testing with mock embedding engine (no network required)
    pub async fn new_for_testing(storage: VectorStorage, config: SemanticConfig) -> SearchResult<Self> {
        let embedding_engine = EmbeddingEngine::new_for_testing().await?;
        Self::with_embedding_engine(storage, embedding_engine, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChunkType, ContentHash, Language, SemanticConfig};
    use std::path::PathBuf;

    async fn create_test_searcher() -> SearchResult<SemanticSearcher> {
        use std::process;
        use std::thread;
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let thread_id = thread::current().id();
        let process_id = process::id();
        let ulid_suffix = ulid::Ulid::new().to_string();

        let db_path =
            format!("/tmp/test_semantic_{timestamp}_{process_id}_{thread_id:?}_{ulid_suffix}.db");

        let config = SemanticConfig {
            database_path: std::path::PathBuf::from(db_path),
            ..SemanticConfig::default()
        };
        let embedding_engine = EmbeddingEngine::new_for_testing().await?;
        let storage = VectorStorage::new(config.clone()).map_err(|e| {
            crate::error::SearchError::SearchOperation {
                operation: "test storage creation".to_string(),
                message: "Failed to create test vector storage".to_string(),
                source: Some(Box::new(e)),
            }
        })?;
        storage
            .initialize()
            .map_err(|e| crate::error::SearchError::SearchOperation {
                operation: "test storage initialization".to_string(),
                message: "Failed to initialize test vector storage".to_string(),
                source: Some(Box::new(e)),
            })?;
        SemanticSearcher::with_embedding_engine(storage, embedding_engine, config).await
    }

    fn create_test_chunk() -> CodeChunk {
        CodeChunk {
            id: "test-chunk-1".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() { println!(\"Hello, world!\"); }".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("test-hash".to_string()),
        }
    }

    #[tokio::test]
    async fn test_searcher_creation() {
        let searcher = create_test_searcher().await;
        assert!(searcher.is_ok());
    }

    #[tokio::test]
    async fn test_search_empty_results() {
        let searcher = create_test_searcher().await.unwrap();
        let query = SearchQuery {
            text: "fn main".to_string(),
            limit: 10,
            similarity_threshold: 0.5,
            language_filter: None,
        };

        let results = searcher.search(&query).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_search_simple() {
        let searcher = create_test_searcher().await.unwrap();
        let results = searcher.search_simple("fn main", 10).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_search_by_language() {
        let searcher = create_test_searcher().await.unwrap();
        let results = searcher
            .search_by_language("fn main", Language::Rust, 10)
            .await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_find_similar_code() {
        let searcher = create_test_searcher().await.unwrap();
        let chunk = create_test_chunk();
        let results = searcher.find_similar_code(&chunk, 5).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_multi_query_search() {
        let searcher = create_test_searcher().await.unwrap();
        let queries = vec!["fn main".to_string(), "println".to_string()];
        let results = searcher.multi_query_search(&queries, 5, 10).await;
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 0);
    }

    #[test]
    fn test_create_excerpt_around_match() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let searcher = rt.block_on(create_test_searcher()).unwrap();
        let content = "This is a test function that does something interesting";
        let match_pos = 10; // Position of "test"
        let excerpt = searcher.create_excerpt_around_match(content, match_pos, 20);

        assert!(excerpt.contains("test"));
        assert!(excerpt.len() <= 25); // 20 + ellipsis
    }

    #[test]
    fn test_create_excerpt_from_start() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let searcher = rt.block_on(create_test_searcher()).unwrap();
        let content = "line1\nline2\nline3\nline4";
        let excerpt = searcher.create_excerpt_from_start(content, 50, 2);

        assert!(excerpt.contains("line1"));
        assert!(excerpt.contains("line2"));
        assert!(!excerpt.contains("line3")); // Should be limited to 2 lines
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let searcher = rt.block_on(create_test_searcher()).unwrap();

        // Test no truncation needed
        let text = "short text";
        let result = searcher.truncate_with_ellipsis(text, 20, false, true);
        assert_eq!(result, "short text");

        // Test truncation with suffix ellipsis
        let long_text = "this is a very long text that should be truncated";
        let result = searcher.truncate_with_ellipsis(long_text, 20, false, true);
        assert!(result.len() <= 23); // 20 + "..."
        assert!(result.ends_with("..."));

        // Test with prefix ellipsis
        let result = searcher.truncate_with_ellipsis(text, 20, true, false);
        assert_eq!(result, "...short text");

        // Test with both prefix and suffix ellipsis on truncated text
        let result = searcher.truncate_with_ellipsis(long_text, 20, true, true);
        assert!(result.starts_with("..."));
        assert!(result.ends_with("..."));

        // Test trimming behavior
        let whitespace_text = "  text with whitespace  ";
        let result = searcher.truncate_with_ellipsis(whitespace_text, 50, false, false);
        assert_eq!(result, "text with whitespace");
    }

    #[tokio::test]
    async fn test_get_search_stats() {
        let searcher = create_test_searcher().await.unwrap();
        let stats = searcher.get_search_stats().await;
        assert!(stats.is_ok());

        let stats = stats.unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.total_embeddings, 0);
    }

    #[tokio::test]
    async fn test_explain_search() {
        let searcher = create_test_searcher().await.unwrap();
        let query = SearchQuery {
            text: "fn main".to_string(),
            limit: 10,
            similarity_threshold: 0.5,
            language_filter: None,
        };

        let explanation = searcher.explain_search(&query).await;
        assert!(explanation.is_ok());

        let explanation = explanation.unwrap();
        assert_eq!(explanation.query_text, "fn main");
        assert_eq!(explanation.threshold, 0.5);
        assert_eq!(explanation.results.len(), 0);
    }

    #[test]
    fn test_calculate_vector_norm() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let searcher = rt.block_on(create_test_searcher()).unwrap();
        let vector = vec![3.0, 4.0]; // 3-4-5 triangle
        let norm = searcher.calculate_vector_norm(&vector);
        assert!((norm - 5.0).abs() < 0.001);
    }

    /// Helper to create a searcher with actual test data
    async fn create_test_searcher_with_data() -> SearchResult<SemanticSearcher> {
        use std::process;
        use std::thread;
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let thread_id = thread::current().id();
        let process_id = process::id();
        let ulid_suffix = ulid::Ulid::new().to_string();

        let db_path =
            format!("/tmp/test_semantic_{timestamp}_{process_id}_{thread_id:?}_{ulid_suffix}.db");

        // Much lower thresholds for local embedding engine with deterministic results
        let config = SemanticConfig {
            database_path: std::path::PathBuf::from(db_path),
            similarity_threshold: -1.0, // Allow negative similarities for mock embeddings
            simple_search_threshold: -1.0,
            code_similarity_threshold: -1.0,
            ..Default::default()
        };
        let embedding_engine = EmbeddingEngine::new_for_testing().await?;
        let storage = VectorStorage::new(config.clone()).map_err(|e| {
            crate::error::SearchError::SearchOperation {
                operation: "test storage creation".to_string(),
                message: "Failed to create test vector storage".to_string(),
                source: Some(Box::new(e)),
            }
        })?;
        storage
            .initialize()
            .map_err(|e| crate::error::SearchError::SearchOperation {
                operation: "test storage initialization".to_string(),
                message: "Failed to initialize test vector storage".to_string(),
                source: Some(Box::new(e)),
            })?;

        // Add test chunks and embeddings
        let chunk1 = CodeChunk {
            id: "test-chunk-rust-fn".to_string(),
            file_path: PathBuf::from("src/main.rs"),
            language: Language::Rust,
            content: "fn main() { println!(\"Hello, world!\"); }".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("rust-main-hash".to_string()),
        };

        let chunk2 = CodeChunk {
            id: "test-chunk-rust-error".to_string(),
            file_path: PathBuf::from("src/error.rs"),
            language: Language::Rust,
            content: "fn handle_error(result: Result<String, Error>) -> String { result.unwrap_or_else(|e| format!(\"Error: {}\", e)) }".to_string(),
            start_line: 5,
            end_line: 7,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("rust-error-hash".to_string()),
        };

        let chunk3 = CodeChunk {
            id: "test-chunk-python-fn".to_string(),
            file_path: PathBuf::from("src/test.py"),
            language: Language::Python,
            content: "def hello_world(): print(\"Hello, world!\")".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("python-hello-hash".to_string()),
        };

        // Store chunks
        storage.store_chunk(&chunk1)?;
        storage.store_chunk(&chunk2)?;
        storage.store_chunk(&chunk3)?;

        // Generate embeddings using the test embedding engine
        let embedding1 = embedding_engine.embed_chunk(&chunk1).await?;
        let embedding2 = embedding_engine.embed_chunk(&chunk2).await?;
        let embedding3 = embedding_engine.embed_chunk(&chunk3).await?;

        // Store embeddings
        storage.store_embedding(&embedding1)?;
        storage.store_embedding(&embedding2)?;
        storage.store_embedding(&embedding3)?;

        SemanticSearcher::with_embedding_engine(storage, embedding_engine, config).await
    }

    #[tokio::test]
    async fn test_search_with_real_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        // Test query that should match hello world functions
        let query = SearchQuery {
            text: "hello world function".to_string(),
            limit: 10,
            similarity_threshold: -1.0, // Allow negative similarities for mock embeddings
            language_filter: None,
        };

        let results = searcher.search(&query).await.unwrap();

        // Should find at least some results with the test data
        assert!(
            !results.is_empty(),
            "Expected to find search results with test data"
        );

        // Results should be sorted by similarity score
        for i in 1..results.len() {
            assert!(
                results[i - 1].similarity_score >= results[i].similarity_score,
                "Results should be sorted by similarity score (descending)"
            );
        }

        // Each result should have a non-empty excerpt
        for result in &results {
            assert!(
                !result.excerpt.is_empty(),
                "Each result should have an excerpt"
            );
            assert!(result.similarity_score >= query.similarity_threshold);
        }
    }

    #[tokio::test]
    async fn test_search_simple_with_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        let results = searcher.search_simple("hello world", 5).await.unwrap();

        // Should find results
        assert!(
            !results.is_empty(),
            "search_simple should find results with test data"
        );

        // Should respect limit
        assert!(results.len() <= 5, "Results should respect the limit");
    }

    #[tokio::test]
    async fn test_search_by_language_with_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        // Search for Rust code only
        let rust_results = searcher
            .search_by_language("function", Language::Rust, 10)
            .await
            .unwrap();

        // All results should be Rust
        for result in &rust_results {
            assert_eq!(
                result.chunk.language,
                Language::Rust,
                "Language filter should work"
            );
        }

        // Search for Python code only
        let python_results = searcher
            .search_by_language("function", Language::Python, 10)
            .await
            .unwrap();

        // All results should be Python
        for result in &python_results {
            assert_eq!(
                result.chunk.language,
                Language::Python,
                "Language filter should work"
            );
        }
    }

    #[tokio::test]
    async fn test_find_similar_code_with_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        let reference_chunk = CodeChunk {
            id: "reference-chunk".to_string(),
            file_path: PathBuf::from("reference.rs"),
            language: Language::Rust,
            content: "fn main() { println!(\"Hello, test!\"); }".to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("reference-hash".to_string()),
        };

        let results = searcher
            .find_similar_code(&reference_chunk, 5)
            .await
            .unwrap();

        // Should find similar code
        assert!(!results.is_empty(), "Should find similar code");

        // Should not include the reference chunk itself
        for result in &results {
            assert_ne!(
                result.chunk.id, reference_chunk.id,
                "Should not include reference chunk"
            );
        }
    }

    #[tokio::test]
    async fn test_multi_query_search_with_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        let queries = vec![
            "hello world".to_string(),
            "main function".to_string(),
            "error handling".to_string(),
        ];

        let results = searcher.multi_query_search(&queries, 2, 5).await.unwrap();

        // Should combine results from multiple queries
        assert!(
            !results.is_empty(),
            "Multi-query search should find results"
        );

        // Should respect overall limit
        assert!(results.len() <= 5, "Should respect overall limit");

        // Should be deduplicated (no duplicate chunk IDs)
        let mut chunk_ids = std::collections::HashSet::new();
        for result in &results {
            assert!(
                chunk_ids.insert(result.chunk.id.clone()),
                "Results should be deduplicated"
            );
        }
    }

    #[tokio::test]
    async fn test_get_search_stats_with_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        let stats = searcher.get_search_stats().await.unwrap();

        // Should have stats reflecting the test data
        assert!(stats.total_chunks > 0, "Should have indexed chunks");
        assert!(stats.total_embeddings > 0, "Should have stored embeddings");
        assert!(
            !stats.model_info.model_id.is_empty(),
            "Should have model info"
        );
    }

    #[tokio::test]
    async fn test_explain_search_with_data() {
        let searcher = create_test_searcher_with_data().await.unwrap();

        let query = SearchQuery {
            text: "function".to_string(),
            limit: 5,
            similarity_threshold: 0.3,
            language_filter: None,
        };

        let explanation = searcher.explain_search(&query).await.unwrap();

        assert_eq!(explanation.query_text, "function");
        assert_eq!(explanation.threshold, 0.3);
        assert!(
            explanation.total_candidates > 0,
            "Should have evaluated candidates"
        );
        assert!(
            !explanation.results.is_empty(),
            "Should have result explanations"
        );

        // Check that each explanation has required fields
        for result_explanation in &explanation.results {
            assert!(!result_explanation.chunk_id.is_empty());
            assert!(result_explanation.similarity_score >= 0.0);
            assert!(!result_explanation.content_preview.is_empty());
        }
    }

    #[test]
    fn test_excerpt_generation_with_match() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let searcher = rt.block_on(create_test_searcher()).unwrap();
        let chunk = CodeChunk {
            id: "test".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "This is a test function that does something interesting with error handling"
                .to_string(),
            start_line: 1,
            end_line: 1,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("test".to_string()),
        };

        let excerpt = searcher.create_excerpt(&chunk, "error handling");

        assert!(
            excerpt.contains("error handling"),
            "Excerpt should contain the query text"
        );
        assert!(
            excerpt.len() <= searcher.config.excerpt_length + 10,
            "Excerpt should respect length limit"
        );
    }

    #[test]
    fn test_excerpt_generation_without_match() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let searcher = rt.block_on(create_test_searcher()).unwrap();
        let chunk = CodeChunk {
            id: "test".to_string(),
            file_path: PathBuf::from("test.rs"),
            language: Language::Rust,
            content: "fn main() {\n    println!(\"Hello\");\n    let x = 42;\n    return x;\n}"
                .to_string(),
            start_line: 1,
            end_line: 5,
            chunk_type: ChunkType::Function,
            content_hash: ContentHash("test".to_string()),
        };

        let excerpt = searcher.create_excerpt(&chunk, "database connection");

        // Should create excerpt from start since no match found
        assert!(excerpt.contains("fn main"), "Should start from beginning");
        assert!(
            excerpt.len() <= searcher.config.excerpt_length + 10,
            "Should respect length limit"
        );
    }
}
