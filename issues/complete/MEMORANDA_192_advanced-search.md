# Implement Advanced Memoranda Search Functionality

## Overview
Enhance the basic search functionality with advanced full-text search capabilities, following the original memoranda's context-aware search patterns.

## Tasks

### 1. Full-Text Search Implementation
- Upgrade basic string matching to full-text search
- Search across both title and content fields
- Case-insensitive search with optional case-sensitive mode
- Support for partial word matching

### 2. Search Indexing
- Create in-memory search index for performance
- Update index on memo create/update/delete operations
- Consider using existing crates like `tantivy` or simple custom indexing
- Lazy loading of search index on first search

### 3. Search Query Enhancements
- Support multiple search terms (AND logic)
- Support phrase searches with quotes: `"exact phrase"`
- Support basic boolean operators: `term1 AND term2`, `term1 OR term2`
- Wildcard search support: `term*`

### 4. Search Result Ranking
- Relevance scoring based on:
  - Title matches weighted higher than content matches
  - Exact matches weighted higher than partial matches  
  - Multiple term matches weighted higher
- Sort results by relevance score
- Include match highlights in results

### 5. Search Performance Optimization
- Benchmark search performance with large memo collections
- Implement search result caching where appropriate
- Optimize for the common case of recent memo access

### 6. Context-Aware Features
- Implement `get_all_context` for AI consumption:
  - Concatenate all memo content with clear delimiters
  - Include metadata (title, created/updated dates)
  - Optimize for token efficiency in AI contexts

## Search API Enhancements
```rust
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub exact_phrase: bool,
    pub max_results: Option<usize>,
    pub include_highlights: bool,
}

pub struct SearchResult {
    pub memo: Memo,
    pub relevance_score: f32,
    pub highlights: Vec<String>,
    pub match_count: usize,
}
```

## Implementation Notes
- Build on existing search implementation from storage layer
- Consider memory usage vs. performance tradeoffs
- Ensure search remains fast even with hundreds of memos
- Make search configurable for different use cases

## Proposed Solution

After analyzing the existing memoranda implementation, I found that there's already a basic search system with simple case-insensitive substring matching. The current implementation in `FileSystemMemoStorage::search_memos()` loads all memos and filters them in memory.

### Implementation Plan

#### 1. Enhanced Search Structures
- Add `SearchOptions` struct with configurable search behavior (case sensitivity, max results, highlighting)
- Add `SearchResult` struct with relevance scoring and match highlights
- Extend existing `MemoStorage` trait with advanced search methods
- Keep existing `search_memos` for backward compatibility

#### 2. Search Indexing System
- Create `AdvancedMemoSearchEngine` that wraps existing storage with search indexing
- Use Tantivy (already used in existing search.rs) for full-text indexing
- Lazy-load index on first search operation
- Update index automatically on memo create/update/delete operations
- In-memory indexing by default, with optional persistent indexing

#### 3. Query Parser
- Implement query parsing for boolean operators: `term1 AND term2`, `term1 OR term2`  
- Support phrase searches with quotes: `"exact phrase"`
- Support wildcard searches: `term*`
- Default behavior: multiple terms treated as AND operation
- Fall back to simple search for malformed queries

#### 4. Relevance Scoring & Ranking
- Title matches weighted higher than content matches (3x weight)
- Exact matches weighted higher than partial matches
- Multiple term matches increase score
- Phrase matches get highest weight
- Sort results by relevance score descending

#### 5. Context-Aware Features
- Implement `get_all_context()` method that concatenates all memo content
- Include metadata (title, dates) with clear delimiters
- Optimize for AI consumption with token-efficient formatting
- Add configurable limits to prevent excessive token usage

#### 6. Performance Optimizations
- Benchmark with collections of 1000+ memos
- Implement result caching for repeated queries
- Optimize common case of recent memo access
- Configurable index update strategies (immediate vs. batched)

### File Structure Changes
```
src/memoranda/
  ├── mod.rs (existing - add new types)
  ├── storage.rs (existing - extend trait)
  └── advanced_search.rs (new - main implementation)
```

### API Extensions
```rust
// New advanced search method on MemoStorage trait
async fn search_memos_advanced(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResult>>;

// New context method
async fn get_all_context(&self, options: &ContextOptions) -> Result<String>;
```

This approach builds on existing infrastructure while providing comprehensive advanced search capabilities.

## Acceptance Criteria
- [ ] Full-text search working across title and content
- [ ] Search query parsing supports basic boolean operations
- [ ] Results properly ranked by relevance
- [ ] Search performance acceptable for large collections (>1000 memos)
- [ ] `get_all_context` optimized for AI consumption
- [ ] Search highlighting working in CLI output
- [ ] Comprehensive tests for search functionality