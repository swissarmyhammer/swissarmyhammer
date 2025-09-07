//! High-level search operations providing a unified API

use crate::{
    error::{SearchError, SearchResult},
    types::{IndexStats, *},
    storage::VectorStorage,

    indexer::{FileIndexer, IndexingOptions},
    searcher::SemanticSearcher,
};
use async_trait::async_trait;


/// Main trait for search operations
#[async_trait]
pub trait SearchOperationsTrait {
    /// Index files from the given patterns
    async fn index_files(&mut self, patterns: Vec<String>, force: bool) -> SearchResult<IndexStats>;
    
    /// Perform a semantic search query
    async fn search(&self, query: &str, limit: usize) -> SearchResult<Vec<SemanticSearchResult>>;
    
    /// Search with advanced options
    async fn search_advanced(&self, query: &SearchQuery) -> SearchResult<Vec<SearchResultWithExplanation>>;
    
    /// Get index statistics
    async fn get_index_stats(&self) -> SearchResult<IndexStats>;
    
    /// Clear the search index
    async fn clear_index(&self) -> SearchResult<()>;
}

/// Main search operations implementation
pub struct SearchOperations {
    storage: VectorStorage,
    indexer: FileIndexer,
    searcher: SemanticSearcher,
}

impl SearchOperations {
    /// Create a new SearchOperations instance
    pub async fn new() -> SearchResult<Self> {
        let config = SemanticConfig::default();
        let storage = VectorStorage::new(config.clone())
            .map_err(|e| SearchError::Database(format!("Failed to initialize storage: {}", e)))?;
        
        let indexer = FileIndexer::new(storage.clone()).await?;
        let searcher = SemanticSearcher::new(storage.clone(), config).await?;
        
        Ok(Self {
            storage,
            indexer,
            searcher,
        })
    }
    
    /// Create a SearchOperations instance with custom configuration
    pub async fn with_config(config: SemanticConfig) -> SearchResult<Self> {
        let storage = VectorStorage::new(config.clone())
            .map_err(|e| SearchError::Database(format!("Failed to initialize storage: {}", e)))?;
        
        let indexer = FileIndexer::new(storage.clone()).await?;
        let searcher = SemanticSearcher::new(storage.clone(), config).await?;
        
        Ok(Self {
            storage,
            indexer,
            searcher,
        })
    }
}

#[async_trait]
impl SearchOperationsTrait for SearchOperations {
    async fn index_files(&mut self, patterns: Vec<String>, force: bool) -> SearchResult<IndexStats> {
        let options = IndexingOptions {
            force,
            glob_pattern: None,
            max_files: None,
        };
        
        self.indexer.index_patterns(patterns, options).await
    }
    
    async fn search(&self, query: &str, limit: usize) -> SearchResult<Vec<SemanticSearchResult>> {
        self.searcher.search_simple(query, limit).await
    }
    
    async fn search_advanced(&self, query: &SearchQuery) -> SearchResult<Vec<SearchResultWithExplanation>> {
        self.searcher.search_with_explanation(query).await
    }
    
    async fn get_index_stats(&self) -> SearchResult<IndexStats> {
        let storage_stats = self.storage.get_stats()?;
        Ok(IndexStats {
            file_count: storage_stats.total_files,
            chunk_count: storage_stats.total_chunks,
            embedding_count: storage_stats.total_embeddings,
        })
    }
    
    async fn clear_index(&self) -> SearchResult<()> {
        self.storage.clear_all().await
    }
}



/// Search result with additional context
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResultWithExplanation {
    pub result: SemanticSearchResult,
    pub explanation: SearchExplanation,
}