# Semantic Search

SwissArmyHammer's semantic search system provides intelligent code search capabilities using vector embeddings and AI-powered similarity matching. Unlike traditional text-based search, semantic search understands the meaning and context of code, enabling more accurate and relevant results.

## Overview

The semantic search system offers:
- **Vector-based search**: Uses embeddings to understand code semantics
- **Multi-language support**: Rust, Python, TypeScript, JavaScript, Dart
- **Code-aware parsing**: TreeSitter integration for structured code analysis
- **Local processing**: All embeddings computed locally with no external API calls
- **Performance optimization**: Efficient indexing and caching
- **Incremental updates**: Only re-index changed files

## Core Concepts

### Semantic Understanding

Traditional text search finds exact matches:
```bash
grep "function login" *.js  # Finds only exact phrase
```

Semantic search understands meaning:
```bash
sah search query "user authentication"  # Finds related concepts:
# - login functions
# - auth middleware  
# - session management
# - password validation
```

### Vector Embeddings

Code is converted to high-dimensional vectors that capture semantic meaning:
- Similar code produces similar vectors
- Related concepts cluster together in vector space
- Similarity measured by vector distance
- AI model trained specifically for code understanding

### Code Structure Awareness

TreeSitter parsing provides structured understanding:
- Function definitions and implementations
- Class hierarchies and relationships
- Module dependencies and imports
- Documentation and comments
- Type information and signatures

## Getting Started

### Indexing Files

Before searching, index your codebase:

```bash
# Index all Rust files
sah search index "**/*.rs"

# Index multiple file types
sah search index "**/*.rs" "**/*.py" "**/*.ts"

# Index specific directories
sah search index "src/**/*.rs" "lib/**/*.rs"

# Force re-index all files
sah search index "**/*.rs" --force
```

### Basic Search

Search indexed code:
```bash
# Basic search
sah search query "error handling"

# Limit results
sah search query "async function implementation" --limit 5

# Search specific concepts
sah search query "database connection pooling"
```

### Search Results

Results include:
```json
{
  "results": [
    {
      "file_path": "src/auth.rs",
      "chunk_text": "fn handle_auth_error(e: AuthError) -> Result<Response> { ... }",
      "line_start": 42,
      "line_end": 48,
      "similarity_score": 0.87,
      "language": "rust", 
      "chunk_type": "Function",
      "excerpt": "...fn handle_auth_error(e: AuthError) -> Result<Response> {..."
    }
  ],
  "query": "error handling",
  "total_results": 1,
  "execution_time_ms": 123
}
```

## Supported Languages

### Rust (.rs)
- Functions and methods
- Structs and enums
- Traits and implementations
- Modules and use statements
- Type definitions
- Documentation comments

### Python (.py)
- Functions and methods
- Classes and inheritance
- Decorators and properties
- Import statements
- Type hints
- Docstrings

### TypeScript (.ts)
- Functions and arrow functions
- Classes and interfaces
- Type definitions
- Import/export statements
- Generics and constraints
- JSDoc comments

### JavaScript (.js)
- Functions (regular and arrow)
- Classes and prototypes
- Module imports/exports
- Object methods
- Closure patterns
- Comments

### Dart (.dart)
- Functions and methods
- Classes and mixins
- Constructors
- Type definitions
- Library imports
- Documentation comments

### Plain Text Fallback

Files that cannot be parsed with TreeSitter are indexed as plain text with basic symbol extraction.

## Advanced Usage

### Indexing Strategies

**Project-wide indexing**:
```bash
# Index entire codebase
sah search index "**/*.{rs,py,ts,js,dart}"
```

**Selective indexing**:
```bash
# Index only source directories
sah search index "src/**/*" "lib/**/*" "crates/**/*"

# Exclude test files
sah search index "**/*.rs" --exclude "**/*test*.rs" "**/*spec*.rs"
```

**Incremental updates**:
```bash
# Only re-index changed files (default behavior)
sah search index "**/*.rs"

# Force complete re-indexing
sah search index "**/*.rs" --force
```

### Search Query Optimization

**Effective queries**:
```bash
# Specific concepts
sah search query "error handling patterns"
sah search query "async database operations" 
sah search query "HTTP request middleware"

# Implementation details
sah search query "trait implementation for serialization"
sah search query "React component lifecycle hooks"
sah search query "memory management and cleanup"
```

**Query strategies**:
- Use domain-specific terminology
- Combine related concepts
- Include both high-level and specific terms
- Search for patterns and implementations

### Result Filtering

**Limit results**:
```bash
sah search query "authentication" --limit 10
```

**Similarity thresholds**:
Results are automatically filtered by similarity score (typically > 0.5).

**File type filtering**:
Search specific languages by indexing only those files:
```bash
sah search index "**/*.rs"  # Index only Rust
sah search query "memory safety"  # Will only search Rust files
```

## Performance Optimization

### Indexing Performance

**First-time setup**:
- Initial embedding model download (~100MB)
- TreeSitter parser compilation
- Complete codebase analysis
- Can take several minutes for large projects

**Subsequent runs**:
- Model cached locally
- Only changed files re-indexed  
- Incremental updates are fast
- Vector database optimized for queries

**Optimization tips**:
```bash
# Index incrementally
sah search index "src/**/*.rs"  # Start with core source
sah search index "lib/**/*.rs"  # Add libraries
sah search index "tests/**/*.rs"  # Add tests last

# Use specific patterns
sah search index "src/main.rs" "src/lib.rs"  # Critical files first
```

### Query Performance

**Fast queries**:
- Embedding model loaded once
- Vector similarity computed efficiently
- Results cached for repeated queries
- Logarithmic scaling with index size

**Performance characteristics**:
- First query: ~1-2 seconds (model loading)
- Subsequent queries: ~100-300ms
- Scales well to large codebases (10k+ files)
- Memory usage scales with index size

### Storage

**Index location**:
- Stored in `.swissarmyhammer/search.db`
- DuckDB database for efficient storage
- Automatically added to `.gitignore`
- Portable across machines

**Storage size**:
- ~1-5MB per 1000 source files
- Compressed vector representations
- Metadata and text chunks
- Grows linearly with codebase size

## Integration with Development Workflow

### Code Exploration

**Understanding new codebases**:
```bash
# Find authentication systems
sah search query "user authentication login"

# Locate error handling patterns  
sah search query "error handling Result Option"

# Find database interactions
sah search query "database query connection"

# Discover API endpoints
sah search query "HTTP route handler endpoint"
```

**Architecture analysis**:
```bash
# Find design patterns
sah search query "factory pattern builder"

# Locate configuration management
sah search query "config settings environment"

# Find testing utilities
sah search query "test helper mock fixture"
```

### Refactoring Support

**Before refactoring**:
```bash
# Find all usages of a concept
sah search query "user session management"

# Locate similar implementations
sah search query "validation input sanitization" 

# Find related error types
sah search query "CustomError DatabaseError"
```

**Impact analysis**:
```bash
# Find dependencies
sah search query "imports {module_name}"

# Locate similar patterns
sah search query "{old_pattern}" --limit 20
```

### Code Review

**Review preparation**:
```bash
# Understand changed areas
sah search query "{feature_area} implementation"

# Find related code
sah search query "{component} {functionality}"

# Check for similar patterns
sah search query "{new_pattern} {approach}"
```

### Documentation and Learning

**Knowledge discovery**:
```bash
# Learn from existing code
sah search query "async streaming data processing"

# Find implementation examples
sah search query "trait object dynamic dispatch"

# Discover best practices
sah search query "error propagation handling"
```

## Use Cases

### Code Discovery

**Finding functionality**:
- "How is logging implemented?"
- "Where are HTTP requests handled?"
- "How is database connection managed?"
- "What validation logic exists?"

**Pattern recognition**:
- "Find all factory patterns"
- "Locate builder implementations"
- "Show async processing examples"
- "Find error handling approaches"

### Maintenance and Debugging

**Issue investigation**:
- "Find error handling for network timeouts"
- "Locate memory leak prevention code"
- "Show panic handling strategies"
- "Find resource cleanup patterns"

**Code quality analysis**:
- "Find duplicate logic patterns"
- "Locate complex functions"
- "Show outdated API usage"
- "Find security-sensitive code"

### Learning and Onboarding

**New team members**:
- "Show authentication flow"
- "Find configuration examples"
- "Locate test patterns"
- "Show deployment procedures"

**Technology adoption**:
- "Find async/await usage"
- "Show generic implementations"
- "Locate macro usage"
- "Find trait implementations"

## Best Practices

### Indexing Strategy

**Comprehensive coverage**:
```bash
# Include all source languages
sah search index "**/*.{rs,py,ts,js,dart,go,java,cpp,h}"

# Exclude generated and vendor code
sah search index "src/**/*" "lib/**/*" --exclude "target/**/*" "node_modules/**/*"
```

**Regular maintenance**:
```bash
# Re-index after major changes
git pull && sah search index "**/*.rs" --force

# Update index with new files
sah search index "**/*.rs"  # Incremental by default
```

### Query Techniques

**Start broad, then narrow**:
```bash
sah search query "authentication"         # Broad overview
sah search query "JWT token validation"   # Specific implementation
sah search query "auth middleware setup"  # Particular aspect
```

**Use domain terminology**:
```bash
# Good: specific terms
sah search query "HTTP request serialization"
sah search query "database transaction rollback"
sah search query "async stream processing"

# Less effective: generic terms  
sah search query "data processing"
sah search query "network code"
```

### Result Analysis

**Evaluate relevance**:
- Higher similarity scores (>0.8) indicate close matches
- Review context around matching code chunks
- Consider file paths and locations
- Examine related functions and types

**Follow-up searches**:
- Use findings to refine queries
- Search for related patterns
- Explore connected functionality
- Verify implementations across codebase

## Troubleshooting

### Indexing Issues

**Model download fails**:
- Check internet connectivity
- Verify disk space (need ~200MB)
- Try again - downloads resume automatically

**Parsing errors**:
- Most files will parse successfully
- Unparseable files indexed as plain text
- Check TreeSitter language support

**Performance problems**:
```bash
# Check index size
ls -la .swissarmyhammer/search.db

# Re-index with smaller scope
sah search index "src/**/*.rs"  # Just source code
```

### Search Issues

**No results found**:
- Verify files are indexed: check `.swissarmyhammer/search.db` exists
- Try broader query terms
- Check if search terms match code language/style
- Re-index if codebase has changed significantly

**Irrelevant results**:
- Use more specific terminology
- Combine multiple concepts in query
- Consider different phrasing
- Try exact technical terms from your domain

**Slow queries**:
- First query loads model (normal delay)
- Large result sets take longer to return
- Reduce result limit for faster response
- Check available memory for large indices

### Common Errors

**"Index not found"**: Run `sah search index` first
**"Model initialization failed"**: Check disk space and permissions
**"No matching files"**: Verify glob patterns and file paths
**"Query too short"**: Use queries with at least 2-3 meaningful words

## Integration with Other Tools

### With Issue Management

Link search results to issues:
```bash
# Find related code for issue
sah search query "user authentication session" > issue_research.md
sah issue update FEATURE_001_auth --file issue_research.md --append
```

### With Workflows

Incorporate search into development workflows:
```markdown
# Research Workflow

1. Search for existing implementations:
   `sah search query "{feature_concept}"`

2. Analyze patterns and approaches:
   Review results for design patterns

3. Document findings:
   `sah memo create --title "[RESEARCH] {feature}"`

4. Plan implementation:
   Use findings to inform architecture decisions
```

### With External Tools

**IDE Integration**:
- Export search results to files for IDE viewing
- Use results to navigate to specific code locations
- Integrate with editor plugins for seamless workflow

**Documentation Generation**:
- Use search results to find code examples
- Extract patterns for documentation
- Generate API usage examples from search results

The semantic search system transforms how you explore, understand, and work with code, providing intelligent discovery capabilities that go far beyond traditional text matching.