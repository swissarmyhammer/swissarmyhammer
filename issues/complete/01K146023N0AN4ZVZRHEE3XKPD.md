update the documentation, focus on the index and search features

update the documentation, focus on the index and search features

## Proposed Solution

After analyzing the codebase, I've identified comprehensive search and indexing functionality that needs better documentation:

### Current Search Systems
1. **Basic Search Engine** (`search.rs`): Tantivy full-text search with fuzzy matching
2. **Advanced Search** (`search_advanced.rs`): Regex, case sensitivity, excerpt generation
3. **Semantic Search** (`semantic/` directory): Vector embeddings, DuckDB storage, TreeSitter parsing

### Documentation Updates Needed

**1. Enhance Search Guide (`search-guide.md`)**
- Add section on semantic/AI-powered search capabilities
- Document the three search strategies and when to use each
- Add examples of semantic search with code similarity detection
- Include performance comparison between search methods

**2. Create Search Architecture Documentation (`search-architecture.md`)**
- Document the indexing system architecture (Tantivy vs DuckDB)
- Explain how embeddings are generated and stored
- Cover TreeSitter integration for code parsing
- Detail the hybrid search strategy combining fuzzy + full-text + semantic

**3. Update CLI Search Documentation (`cli-search.md`)**
- Add semantic search command options
- Document embedding model configuration
- Include examples of language-specific semantic search

**4. Create Index Management Guide (`index-management.md`)**
- Document index creation, updating, and optimization
- Cover both text and vector index maintenance
- Performance tuning guidelines
- Index storage and persistence patterns

**5. Add Performance Documentation**
- Benchmarking results for different search strategies
- Memory usage patterns and optimization
- Scalability considerations for large codebases

### Implementation Steps
1. Create new documentation files for semantic search and indexing
2. Enhance existing search guide with comprehensive examples
3. Update CLI documentation with new search features
4. Add performance and architecture documentation
5. Cross-reference all search-related documentation

This will provide complete coverage of the sophisticated search and indexing capabilities that are currently implemented but under-documented.