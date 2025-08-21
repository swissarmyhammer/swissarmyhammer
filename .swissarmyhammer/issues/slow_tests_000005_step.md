# Step 5: Optimize Database and Search Tests

Refer to /Users/wballard/sah-slow_tests/ideas/slow_tests.md

## Objective
Optimize tests involving DuckDB operations, semantic search functionality, and Tantivy search indexing to reduce execution time while maintaining comprehensive test coverage.

## Background
The SwissArmyHammer codebase includes extensive search functionality:
- Semantic search using DuckDB for vector storage (`swissarmyhammer/src/search/`)
- Tantivy-based text search and indexing  
- Search tool integration tests (`swissarmyhammer-tools/src/mcp/tools/search/`)
- Large-scale indexing and embedding operations
- Search performance and regression tests

## Tasks

### 1. Audit Database and Search Test Performance
- Identify tests involving DuckDB database operations  
- Document tests with expensive indexing operations
- Map tests using embedding model operations
- Identify tests with large dataset processing

### 2. Optimize Database Test Patterns
- **In-Memory Databases**: Use `:memory:` DuckDB connections for tests
- **Minimal Schema Setup**: Create only necessary tables and indexes
- **Batch Operations**: Group database operations to reduce connection overhead
- **Transaction Isolation**: Use transactions for test data isolation
- **Connection Reuse**: Share database connections within test modules where safe

### 3. Optimize Search Index Operations
- **Small Test Corpora**: Use minimal document sets that validate functionality
- **Mock Embeddings**: Replace expensive embedding generation with mock vectors
- **Index Reuse**: Cache and reuse search indexes for related tests where appropriate
- **Focused Indexing**: Index only necessary fields for specific tests

### 4. Split Large Search Integration Tests
Break down complex search tests:
- **Unit Tests**: Test search logic without database/index overhead  
- **Component Tests**: Test search operations with mock backends
- **Integration Tests**: Test complete search workflows with optimized data
- **Performance Tests**: Separate performance validation from functional testing

### 5. Implement Database Test Optimizations

#### In-Memory Database Pattern
```rust
use duckdb::Connection;

#[test] 
fn test_search_storage() {
    // Use in-memory database for fast test execution
    let conn = Connection::open_in_memory().unwrap();
    
    // Minimal schema setup
    conn.execute_batch("
        CREATE TABLE test_embeddings (
            id INTEGER PRIMARY KEY,
            content TEXT,
            embedding FLOAT[]
        );
    ").unwrap();
    
    // Test operations on isolated in-memory database
}
```

#### Mock Embedding Pattern
```rust
// Instead of expensive embedding generation
fn generate_test_embedding() -> Vec<f32> {
    // Return mock embedding vector instead of calling model
    vec![0.1, 0.2, 0.3, 0.4, 0.5]
}

#[test]
fn test_semantic_search() {
    let mock_embedding = generate_test_embedding();
    // Test search logic without expensive embedding generation
}
```

#### Minimal Search Index Pattern
```rust
use tantivy::{Index, schema::*};

fn create_test_index() -> Index {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("content", TEXT);
    let schema = schema_builder.build();
    
    // Create minimal in-memory index
    Index::create_in_ram(schema)
}
```

## Acceptance Criteria
- [ ] All database and search tests identified and performance-profiled
- [ ] Database tests optimized to use in-memory connections
- [ ] Search tests use minimal test corpora and mock embeddings
- [ ] Large search integration tests split into focused components  
- [ ] Tests can run in parallel with isolated database instances
- [ ] Database/search test execution time reduced by >60%
- [ ] All search functionality test coverage maintained
- [ ] No shared database state between tests

## Implementation Strategy

### Test Categories to Optimize
1. **Semantic Search Tests** - DuckDB vector storage and retrieval
2. **Text Search Tests** - Tantivy indexing and querying operations
3. **Search Integration Tests** - End-to-end search workflow tests
4. **Index Performance Tests** - Large-scale indexing operations  
5. **Embedding Tests** - Model loading and vector generation tests

### Database Optimization Techniques
- Use DuckDB in-memory mode (`:memory:`) for all tests
- Pre-populate test databases with minimal datasets
- Use database transactions for test isolation
- Implement database connection pooling for test suites
- Cache expensive index builds across related tests

### Search Optimization Techniques  
- Generate mock embeddings instead of using actual models
- Create minimal Tantivy indexes with small document sets
- Use deterministic test data for reproducible search results
- Implement search result mocking for unit tests
- Optimize schema definitions for test-specific needs

## Estimated Effort
Large (6-7 focused work sessions)

## Dependencies  
- Step 2 (serial test fixes for parallel database operations)
- Step 4 (file system optimizations for index storage)

## Follow-up Steps
- Step 6: Optimize Integration and E2E Tests  
- Database optimizations will significantly improve search-related test performance