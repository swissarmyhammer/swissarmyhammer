# Rust API Reference

SwissArmyHammer provides a comprehensive Rust API for building custom tools, integrations, and extensions. The library is designed with modularity and flexibility in mind, offering both async and sync interfaces for different use cases.

## Overview

The SwissArmyHammer crate provides:
- **Prompt Management**: Load, store, and organize prompts from various sources
- **Template Engine**: Powerful Liquid-based template processing with custom filters
- **System Prompt**: Automatic Claude Code integration with centralized coding standards
- **Semantic Search**: Vector-based code search with TreeSitter parsing
- **Issue Tracking**: Git-integrated issue management system
- **Memoranda**: Note-taking and knowledge management
- **Workflow System**: State-based execution engine
- **Plugin Architecture**: Extensible filter and processing system

## Quick Start

Add SwissArmyHammer to your `Cargo.toml`:

```toml
[dependencies]
swissarmyhammer = "0.1.0"
```

Basic usage example:

```rust
use swissarmyhammer::PromptLibrary;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new prompt library
    let mut library = PromptLibrary::new();
    
    // Add prompts from a directory
    library.add_directory("./.swissarmyhammer/prompts")?;
    
    // Get and render a prompt
    let prompt = library.get("code-review")?;
    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    let rendered = prompt.render(&args)?;
    
    println!("{}", rendered);
    Ok(())
}
```

## Core Modules

### Prompt Management (`prompts`)

The prompts module provides the core functionality for managing and organizing prompts.

#### Key Types

**`PromptLibrary`**: Main interface for prompt management
```rust
use swissarmyhammer::prompts::PromptLibrary;

let mut library = PromptLibrary::new();
library.add_directory("./prompts")?;
library.add_file("./custom_prompt.md")?;

// Access prompts
let prompt = library.get("my-prompt")?;
let all_prompts = library.list();
```

**`Prompt`**: Represents a single prompt with metadata
```rust
use swissarmyhammer::prompts::Prompt;

let prompt = Prompt {
    name: "code-review".to_string(),
    content: "Review this code: {{ code }}".to_string(),
    metadata: HashMap::new(),
};

let rendered = prompt.render(&context)?;
```

**`PromptMetadata`**: Metadata associated with prompts
```rust
use swissarmyhammer::prompts::PromptMetadata;

let metadata = PromptMetadata {
    description: Some("Code review prompt".to_string()),
    tags: vec!["review".to_string(), "code".to_string()],
    author: Some("team@example.com".to_string()),
    version: Some("1.0.0".to_string()),
    ..Default::default()
};
```

#### Prompt Loading

```rust
use swissarmyhammer::prompt_resolver::PromptResolver;

let resolver = PromptResolver::new();

// Load from directory
resolver.load_directory("./prompts")?;

// Load single file
resolver.load_file("./prompt.md")?;

// Load from memory
resolver.load_from_content("name", "content", metadata)?;
```

### Template Engine (`template`)

Liquid-based template engine with custom filters and extensions.

#### Basic Templating

```rust
use swissarmyhammer::template::Template;
use std::collections::HashMap;

let template = Template::from_string("Hello {{ name }}!")?;
let mut context = HashMap::new();
context.insert("name".to_string(), "World".to_string());

let rendered = template.render(&context)?;
assert_eq!(rendered, "Hello World!");
```

#### Advanced Features

```rust
use swissarmyhammer::template::{Template, TemplateEngine};

let engine = TemplateEngine::new()
    .with_custom_filters()
    .with_security_limits();

let template = engine.parse("{{ code | highlight: 'rust' | trim }}")?;
let result = template.render(&context)?;
```

### System Prompt (`system_prompt`)

The system prompt module provides automatic integration with Claude Code through centralized coding standards and guidelines.

#### System Prompt Rendering

```rust
use swissarmyhammer::system_prompt::{render_system_prompt, clear_cache, SystemPromptError};

// Render the system prompt with caching
let rendered_prompt = render_system_prompt()?;
println!("System prompt ({} characters)", rendered_prompt.len());

// Clear the cache to force re-render
clear_cache();
let fresh_prompt = render_system_prompt()?;
```

#### Error Handling

```rust
use swissarmyhammer::system_prompt::{render_system_prompt, SystemPromptError};

match render_system_prompt() {
    Ok(content) => println!("System prompt loaded successfully"),
    Err(SystemPromptError::FileNotFound(path)) => {
        println!("System prompt not found at: {}", path);
        // Continue without system prompt
    }
    Err(SystemPromptError::TemplateError(e)) => {
        eprintln!("Template rendering failed: {}", e);
        return Err(e.into());
    }
    Err(e) => {
        eprintln!("System prompt error: {}", e);
        return Err(e.into());
    }
}
```

#### Claude Code Integration

```rust
use swissarmyhammer::claude_code_integration::{
    execute_claude_code_with_system_prompt, 
    ClaudeCodeConfig, 
    ClaudeCodeError
};

// Configure system prompt integration (always enabled)
let config = ClaudeCodeConfig {
    system_prompt_debug: false,
};

// Execute Claude Code with automatic system prompt injection
let args = vec!["prompt".to_string(), "render".to_string(), "my-prompt".to_string()];
let result = execute_claude_code_with_system_prompt(&args, None, config, false).await?;

println!("Claude Code output: {}", String::from_utf8_lossy(&result.stdout));
```

#### Custom System Prompt Implementation

```rust
use swissarmyhammer::system_prompt::SystemPromptRenderer;
use std::path::PathBuf;

// Create a custom renderer with specific paths
let renderer = SystemPromptRenderer::new();

// Render with custom template context
let custom_context = std::collections::HashMap::new();
let rendered = renderer.render_with_context(&custom_context)?;

// Check cache validity manually
let system_prompt_path = PathBuf::from("builtin/prompts/.system.md");
if renderer.is_cache_valid(&cached_entry, &system_prompt_path) {
    println!("Cache is valid, using cached content");
} else {
    println!("Cache is stale, will re-render");
}
```

#### Custom Filters

```rust
use swissarmyhammer::prompt_filter::PromptFilter;

// Built-in filters
let filters = vec![
    PromptFilter::Trim,
    PromptFilter::Uppercase,
    PromptFilter::CodeHighlight { language: "rust".to_string() },
    PromptFilter::FileRead { path: "./example.rs".to_string() },
];

// Apply filters to content
let processed = filters.apply("content")?;
```

### Semantic Search (`search`)

Vector-based semantic search with TreeSitter integration for code understanding.

#### Indexing

```rust
use swissarmyhammer::search::{SearchEngine, IndexConfig};

let config = IndexConfig {
    model_path: "./models/code-embeddings".to_string(),
    index_path: "./search_index".to_string(),
    ..Default::default()
};

let engine = SearchEngine::new(config)?;

// Index files
engine.index_files(&["**/*.rs", "**/*.py"]).await?;

// Index specific content
engine.index_content("file.rs", content, language).await?;
```

#### Querying

```rust
use swissarmyhammer::search::SearchQuery;

let query = SearchQuery {
    text: "error handling patterns".to_string(),
    limit: 10,
    similarity_threshold: 0.5,
    ..Default::default()
};

let results = engine.search(&query).await?;

for result in results {
    println!("File: {} (score: {:.2})", result.file_path, result.similarity_score);
    println!("Content: {}", result.excerpt);
}
```

### Issue Management (`issues`)

Git-integrated issue tracking system.

#### Core Types

```rust
use swissarmyhammer_issues::{Issue, IssueName, FileSystemIssueStorage};

// Create issue
let issue = Issue {
    name: IssueName::new("FEATURE_001_user-auth")?,
    content: "# User Authentication\n\nImplement login system".to_string(),
    status: IssueStatus::Active,
    created_at: chrono::Utc::now(),
    ..Default::default()
};

// Storage operations
let storage = FileSystemIssueStorage::new("./issues")?;
storage.create(&issue).await?;
storage.complete(&issue.name).await?;
```

#### Git Integration

```rust
use swissarmyhammer_issues::{IssueManager, GitIntegration};

let manager = IssueManager::new("./issues")?
    .with_git_integration();

// Start work (creates branch)
manager.start_work(&issue_name).await?;

// Complete work (merges branch) 
manager.complete_work(&issue_name).await?;

// Get current issue from branch
let current = manager.current_issue().await?;
```

### Memoranda (`memoranda`)

Note-taking and knowledge management system.

```rust
use swissarmyhammer_memoranda::{MemoStorage, Memo};

let storage = MemoStorage::new("./memos")?;

// Create memo
let memo = storage.create(
    "Architecture Notes",
    "# System Design\n\nKey decisions and rationale"
).await?;

// Search memos
let results = storage.search("architecture design").await?;

// Get all memos
let all_memos = storage.list().await?;
```

### Workflow System (`workflow`)

State-based execution engine for complex automation.

#### Workflow Definition

```rust
use swissarmyhammer::workflow::{Workflow, WorkflowState, Action};

let workflow = Workflow {
    name: "development-cycle".to_string(),
    initial_state: "research".to_string(),
    states: HashMap::from([
        ("research".to_string(), WorkflowState {
            actions: vec![
                Action::SearchCode { query: "{{ feature }}" },
                Action::CreateMemo { title: "Research: {{ feature }}" },
            ],
            transitions: HashMap::from([
                ("complete".to_string(), "design".to_string()),
            ]),
        }),
        ("design".to_string(), WorkflowState {
            actions: vec![
                Action::CreateIssue { 
                    name: "{{ feature }}",
                    content: "{{ design_spec }}"
                },
            ],
            transitions: HashMap::from([
                ("approved".to_string(), "implement".to_string()),
            ]),
        }),
    ]),
};
```

#### Workflow Execution

```rust
use swissarmyhammer::workflow::{WorkflowEngine, ExecutionContext};

let engine = WorkflowEngine::new();
let context = ExecutionContext::new()
    .with_variable("feature", "user-authentication")
    .with_variable("design_spec", "OAuth 2.0 implementation");

let execution = engine.execute(&workflow, context).await?;

// Check execution status
match execution.status {
    ExecutionStatus::Running => println!("Workflow in progress"),
    ExecutionStatus::Completed => println!("Workflow completed successfully"),
    ExecutionStatus::Failed(error) => println!("Workflow failed: {}", error),
}
```

### Plugin System (`plugins`)

Extensible architecture for custom functionality.

#### Plugin Development

```rust
use swissarmyhammer::plugins::{Plugin, PluginContext, PluginResult};

#[derive(Debug)]
pub struct CustomCodeFormatter;

impl Plugin for CustomCodeFormatter {
    fn name(&self) -> &str {
        "custom-formatter"
    }
    
    fn process(&self, input: &str, context: &PluginContext) -> PluginResult<String> {
        // Custom formatting logic
        let formatted = format_code(input, &context.language)?;
        Ok(formatted)
    }
}

// Register plugin
let mut registry = PluginRegistry::new();
registry.register(Box::new(CustomCodeFormatter))?;
```

#### Using Plugins

```rust
use swissarmyhammer::plugins::{PluginRegistry, PluginContext};

let registry = PluginRegistry::with_builtin_plugins();
let context = PluginContext {
    language: Some("rust".to_string()),
    file_path: Some("./src/main.rs".to_string()),
    ..Default::default()
};

let result = registry.apply("custom-formatter", input, &context)?;
```

## Configuration

### Library Configuration

```rust
use swissarmyhammer::config::{Config, SearchConfig, IssueConfig};

let config = Config {
    search: SearchConfig {
        model_path: "./models".to_string(),
        index_path: "./.sah/search.db".to_string(),
        embedding_dimension: 768,
        ..Default::default()
    },
    issues: IssueConfig {
        storage_path: "./issues".to_string(),
        git_integration: true,
        branch_prefix: "issue/".to_string(),
        ..Default::default()
    },
    ..Default::default()
};

// Initialize with config
let library = PromptLibrary::with_config(config)?;
```

### Environment Integration

```rust
use swissarmyhammer::config::ConfigBuilder;

let config = ConfigBuilder::new()
    .from_env()  // Load from environment variables
    .from_file("./sah.toml")?  // Override with file config
    .from_args(args)?  // Override with CLI args
    .build()?;
```

## Error Handling

SwissArmyHammer uses a comprehensive error system:

```rust
use swissarmyhammer::error::{SwissArmyHammerError, Result};

fn example_function() -> Result<String> {
    match some_operation() {
        Ok(value) => Ok(value),
        Err(e) => Err(SwissArmyHammerError::ProcessingError {
            message: "Operation failed".to_string(),
            source: Some(Box::new(e)),
        }),
    }
}

// Error types
pub enum SwissArmyHammerError {
    IoError(std::io::Error),
    TemplateError(String),
    SearchError(String),
    IssueError(String),
    ValidationError(String),
    ConfigError(String),
    NetworkError(String),
    ProcessingError { message: String, source: Option<Box<dyn std::error::Error + Send + Sync>> },
}
```

## Async and Sync APIs

Most functionality is available in both async and sync variants:

```rust
// Async API (preferred for I/O operations)
use swissarmyhammer::async_api::*;

let results = search_engine.search(&query).await?;
let memo = memo_storage.create(title, content).await?;

// Sync API (for simple cases)
use swissarmyhammer::sync_api::*;

let results = search_engine.search_blocking(&query)?;
let memo = memo_storage.create_blocking(title, content)?;
```

## Testing Utilities

SwissArmyHammer provides testing utilities for integration tests:

```rust
use swissarmyhammer::test_utils::*;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_prompt_rendering() {
        let temp_dir = create_temp_directory()?;
        let library = create_test_library(&temp_dir)?;
        
        let prompt = library.get("test-prompt")?;
        let result = prompt.render(&test_context())?;
        
        assert_eq!(result, "Expected output");
    }
    
    #[test]
    fn test_search_indexing() {
        let temp_index = create_temp_search_index()?;
        let engine = SearchEngine::new(temp_index.config())?;
        
        // Test operations
    }
}
```

## Performance Considerations

### Memory Management

```rust
use swissarmyhammer::config::PerformanceConfig;

let config = PerformanceConfig {
    max_prompt_size: 1024 * 1024,  // 1MB
    max_search_results: 100,
    cache_size: 1000,
    enable_lazy_loading: true,
    ..Default::default()
};
```

### Caching

```rust
use swissarmyhammer::cache::{Cache, CacheConfig};

let cache = Cache::new(CacheConfig {
    max_entries: 1000,
    ttl_seconds: 3600,
    enable_persistence: true,
})?;

// Cached operations
let result = cache.get_or_compute("key", || expensive_operation())?;
```

### Resource Limits

```rust
use swissarmyhammer::security::{ResourceLimits, SecurityContext};

let limits = ResourceLimits {
    max_file_size: 10 * 1024 * 1024,  // 10MB
    max_processing_time: Duration::from_secs(30),
    allowed_directories: vec!["/safe/path".to_string()],
    ..Default::default()
};

let security = SecurityContext::new(limits);
```

## Integration Examples

### Custom CLI Tool

```rust
use swissarmyhammer::prelude::*;
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Search { query: String },
    CreateMemo { title: String, content: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let library = PromptLibrary::new();
    
    match cli.command {
        Commands::Search { query } => {
            let results = library.search(&query).await?;
            for result in results {
                println!("{}: {}", result.name, result.excerpt);
            }
        }
        Commands::CreateMemo { title, content } => {
            let memo = library.create_memo(&title, &content).await?;
            println!("Created memo: {}", memo.id);
        }
    }
    
    Ok(())
}
```

### Web Service Integration

```rust
use swissarmyhammer::prelude::*;
use axum::{Json, extract::Query, routing::get, Router};

#[derive(serde::Deserialize)]
struct SearchParams {
    q: String,
    limit: Option<usize>,
}

async fn search(Query(params): Query<SearchParams>) -> Json<SearchResults> {
    let library = get_library().await;
    let results = library.search(&params.q)
        .limit(params.limit.unwrap_or(10))
        .execute().await
        .unwrap();
    Json(results)
}

fn app() -> Router {
    Router::new()
        .route("/search", get(search))
}
```

## Migration Guide

### From 0.x to 1.0

Key breaking changes and migration strategies:

```rust
// Old API
let library = PromptLibrary::from_directory("./prompts")?;

// New API
let mut library = PromptLibrary::new();
library.add_directory("./prompts")?;

// Old search API
let results = search("query")?;

// New search API  
let engine = SearchEngine::new(config)?;
let results = engine.search(&SearchQuery::new("query")).await?;
```

This API reference provides comprehensive coverage of SwissArmyHammer's Rust API. For additional examples and detailed documentation, see the generated rustdoc documentation and the examples directory in the repository.