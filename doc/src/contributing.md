# Contributing

Welcome to SwissArmyHammer! We appreciate your interest in contributing to this project. This guide will help you get started.

## Code of Conduct

SwissArmyHammer follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive in all interactions.

## Getting Started

### Development Environment

1. **Install Rust**: Ensure you have Rust 1.70 or later installed
2. **Clone the repository**:
   ```bash
   git clone https://github.com/swissarmyhammer/swissarmyhammer.git
   cd swissarmyhammer
   ```

3. **Install dependencies**:
   ```bash
   # Install development dependencies
   cargo install cargo-watch cargo-tarpaulin cargo-audit
   
   # Install pre-commit hooks (optional but recommended)
   pip install pre-commit
   pre-commit install
   ```

4. **Run tests** to verify setup:
   ```bash
   cargo test
   cargo clippy
   cargo fmt --check
   ```

### Project Structure

```
swissarmyhammer/
├── swissarmyhammer/          # Core library
├── swissarmyhammer-cli/      # Command-line interface
├── swissarmyhammer-tools/    # MCP tools and server
│   └── src/mcp/tools/        # Individual MCP tool implementations
├── builtin/                  # Built-in prompts and workflows
├── doc/                      # Documentation (mdBook)
├── tests/                    # Integration tests
└── benches/                  # Benchmarks
```

### MCP Tools Architecture

SwissArmyHammer uses a **dynamic CLI architecture** where CLI commands are automatically generated from MCP tool definitions. This eliminates code duplication and ensures perfect consistency between MCP and CLI interfaces.

Key components:
- **Tool Registry** - Central registry of all MCP tools
- **Dynamic CLI Builder** - Automatically generates CLI commands from tool schemas
- **Schema Converter** - Converts between JSON Schema and Clap arguments
- **Dynamic Execution** - Routes CLI commands to appropriate MCP tools

## How to Contribute

### Reporting Issues

Before creating an issue, please:
1. Search existing issues to avoid duplicates
2. Use the issue templates when available
3. Provide detailed information including:
   - SwissArmyHammer version (`sah --version`)
   - Operating system and version
   - Steps to reproduce
   - Expected vs actual behavior
   - Relevant configuration files

### Proposing Features

For new features:
1. Open an issue with the "feature request" label
2. Describe the problem you're solving
3. Provide examples of how it would work
4. Consider implementation complexity
5. Wait for maintainer feedback before starting work

### Developing MCP Tools

MCP tools are the primary extension mechanism for SwissArmyHammer. Each tool automatically becomes available in both the MCP interface (for Claude Code) and the CLI.

#### Creating a New MCP Tool

1. **Create tool module** in `swissarmyhammer-tools/src/mcp/tools/`:
```bash
mkdir -p swissarmyhammer-tools/src/mcp/tools/yourcategory/youraction
```

2. **Implement the tool**:
```rust
// swissarmyhammer-tools/src/mcp/tools/yourcategory/youraction/mod.rs
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde_json;
use std::collections::HashMap;
use crate::mcp::{McpError, McpTool, ToolContext};

pub struct YourActionTool;

#[async_trait]
impl McpTool for YourActionTool {
    fn name(&self) -> &'static str {
        "yourcategory_youraction"  // MCP tool name
    }
    
    fn description(&self) -> &'static str {
        "Brief description for MCP interface"
    }
    
    // CLI Integration Methods
    fn cli_category(&self) -> Option<&'static str> {
        Some("yourcategory")  // CLI category
    }
    
    fn cli_name(&self) -> &'static str {
        "youraction"  // CLI command name
    }
    
    fn cli_about(&self) -> Option<&'static str> {
        Some("Detailed description for CLI help")
    }
    
    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the item"
                },
                "content": {
                    "type": "string",
                    "description": "Content of the item"
                },
                "priority": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "default": 3,
                    "description": "Priority level (1-5)"
                }
            },
            "required": ["title", "content"]
        })
    }
    
    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> Result<CallToolResult, McpError> {
        // Extract arguments
        let title = arguments.get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing title".to_string()))?;
        
        let content = arguments.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidArguments("Missing content".to_string()))?;
        
        let priority = arguments.get("priority")
            .and_then(|v| v.as_i64())
            .unwrap_or(3) as i32;
        
        // Implement your logic here
        let result = format!("Created item '{}' with priority {}", title, priority);
        
        Ok(CallToolResult::success(&result))
    }
}
```

3. **Add description file** (optional but recommended):
```markdown
<!-- swissarmyhammer-tools/src/mcp/tools/yourcategory/youraction/description.md -->
# Your Action Tool

Detailed description of what this tool does, including:
- Purpose and use cases
- Parameter explanations
- Example usage
- Expected behavior
```

4. **Register the tool** - Add to build macros or registry (done automatically in most cases)

#### Tool Development Guidelines

**CLI Integration Best Practices**:

1. **Category Organization**: Group related tools under logical categories
   - `memo` - Memoranda/note management
   - `issue` - Issue tracking
   - `files` - File operations
   - `search` - Search functionality

2. **Command Naming**: Use action-oriented names
   - `create`, `list`, `update`, `delete` for CRUD operations
   - `search`, `query`, `index` for search operations
   - `show`, `work`, `merge` for workflow operations

3. **Schema Design**: Create schemas that translate well to CLI
   - Use descriptive property names
   - Include helpful descriptions (become help text)
   - Add appropriate validation (min/max, enum values)
   - Provide sensible defaults

**JSON Schema Best Practices**:

```rust
fn schema(&self) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            // String arguments become --flag value
            "name": {
                "type": "string",
                "description": "Name of the resource",
                "minLength": 1,
                "maxLength": 100
            },
            
            // Boolean flags (use sparingly)
            "force": {
                "type": "boolean", 
                "default": false,
                "description": "Force the operation"
            },
            
            // Enum provides validation and CLI choices
            "format": {
                "type": "string",
                "enum": ["json", "yaml", "table"],
                "default": "table", 
                "description": "Output format"
            },
            
            // Arrays for multiple values
            "tags": {
                "type": "array",
                "items": {"type": "string"},
                "description": "List of tags to apply"
            }
        },
        "required": ["name"]
    })
}
```

**Error Handling**:

```rust
async fn execute(&self, arguments: serde_json::Map<String, serde_json::Value>, context: &ToolContext) -> Result<CallToolResult, McpError> {
    // Validate arguments
    let name = arguments.get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidArguments("Missing required field 'name'".to_string()))?;
    
    // Perform operation with proper error handling
    match perform_operation(name).await {
        Ok(result) => Ok(CallToolResult::success(&result)),
        Err(e) => Ok(CallToolResult::error(&format!("Operation failed: {}", e)))
    }
}
```

**Testing Tools**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[tokio::test]
    async fn test_tool_execution() {
        let tool = YourActionTool;
        let context = create_test_context();
        
        let mut args = serde_json::Map::new();
        args.insert("title".to_string(), serde_json::Value::String("Test".to_string()));
        args.insert("content".to_string(), serde_json::Value::String("Content".to_string()));
        
        let result = tool.execute(args, &context).await.unwrap();
        assert!(!result.is_error.unwrap_or(false));
    }

    #[test]
    fn test_schema_validation() {
        let tool = YourActionTool;
        let schema = tool.schema();
        
        // Verify schema structure
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["title"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::Value::String("title".to_string())));
    }
}
```

### Code Contributions

#### Pull Request Process

1. **Fork the repository** and create a feature branch
2. **Make your changes** following our coding standards
3. **Add tests** for new functionality
4. **Update documentation** if needed
5. **Run the full test suite**:
   ```bash
   # Run all tests
   cargo test --workspace
   
   # Run integration tests
   cargo test --test '*'
   
   # Check formatting and lints
   cargo fmt --check
   cargo clippy -- -D warnings
   
   # Run benchmarks (if performance-related)
   cargo bench
   ```

6. **Create a pull request** with:
   - Clear title and description
   - Link to related issues
   - Screenshots/examples if applicable
   - Checklist of completed items

#### Coding Standards

**Rust Code Style**:
- Use `cargo fmt` for formatting
- Pass `cargo clippy` with no warnings
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Write comprehensive doc comments with examples
- Use meaningful variable and function names

**Error Handling**:
- Use the `anyhow` crate for error handling
- Provide contextual error messages
- Use the `Result` type consistently
- Don't panic in library code

**Testing**:
- Write unit tests for all public functions
- Add integration tests for complex workflows
- Use property-based testing where appropriate
- Maintain test coverage above 80%

**Documentation**:
- Write rustdoc comments for all public items
- Include usage examples in documentation
- Update the user guide for new features
- Keep CHANGELOG.md updated

#### Code Review Guidelines

**For Authors**:
- Keep PRs focused and reasonably sized
- Respond to feedback promptly
- Be open to suggestions and changes
- Test edge cases and error conditions

**For Reviewers**:
- Be constructive and specific in feedback
- Test the changes locally when possible
- Check for security implications
- Verify documentation is updated

### Documentation Contributions

Documentation improvements are always welcome:

- **User Guide**: Located in `doc/src/`
- **API Documentation**: Rust doc comments in source code
- **Examples**: Located in `doc/src/examples/`
- **README**: Project overview and quick start

When updating documentation:
1. Use clear, concise language
2. Provide practical examples
3. Test all code examples
4. Check for broken links
5. Follow the existing style and structure

## Development Workflows

### Running Tests

```bash
# Unit tests only
cargo test --lib

# Integration tests only  
cargo test --test '*'

# All tests with verbose output
cargo test --workspace --verbose

# Test with coverage
cargo tarpaulin --out html

# Test specific module
cargo test --package swissarmyhammer search::tests
```

### Development Server

For MCP development:
```bash
# Run MCP server in development mode
cargo run --bin swissarmyhammer-cli serve --stdio

# Or with debug logging
SAH_LOG_LEVEL=debug cargo run --bin swissarmyhammer-cli serve --stdio
```

### Benchmarking

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench search

# Generate benchmark reports
cargo bench -- --save-baseline main
```

### Debugging

```bash
# Run with debug logging
SAH_LOG_LEVEL=debug cargo run --bin swissarmyhammer-cli prompt list

# Use debugger
RUST_LOG=debug cargo run --bin swissarmyhammer-cli -- --help

# Memory debugging with valgrind
valgrind --tool=memcheck cargo run --bin swissarmyhammer-cli
```

## Contribution Areas

### High-Impact Areas

1. **Performance Optimizations**
   - Search indexing speed
   - Template rendering performance  
   - Memory usage reduction
   - Startup time optimization

2. **New Language Support**
   - Add TreeSitter parsers
   - Language-specific prompt templates
   - Build tool integrations

3. **MCP Tool Enhancements**
   - New tool implementations
   - Better error reporting
   - Request/response validation

4. **Documentation**
   - More examples and tutorials
   - Video guides
   - Translation to other languages

5. **Testing**
   - Edge case coverage
   - Performance regression tests
   - Cross-platform testing

### Good First Issues

Look for issues labeled `good-first-issue`:
- Documentation improvements
- Small bug fixes
- Adding new built-in prompts
- Test coverage improvements
- Error message enhancements

## Release Process

### Versioning

SwissArmyHammer uses [Semantic Versioning](https://semver.org/):
- **MAJOR**: Incompatible API changes
- **MINOR**: New functionality (backwards compatible)
- **PATCH**: Bug fixes (backwards compatible)

### Release Checklist

1. Update version numbers in `Cargo.toml` files
2. Update CHANGELOG.md with release notes
3. Run full test suite on multiple platforms
4. Update documentation if needed
5. Create release PR for review
6. Tag release after merge
7. Build and publish binaries
8. Update package registries (crates.io)
9. Announce release

## Community Guidelines

### Communication

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General questions and ideas
- **Discord**: Real-time chat (if available)
- **Matrix**: Alternative chat platform (if available)

### Getting Help

If you need help:
1. Check the documentation first
2. Search existing issues
3. Ask in GitHub Discussions
4. Tag maintainers if urgent

### Recognition

Contributors are recognized:
- In CONTRIBUTORS.md file
- In release notes for significant contributions
- Through GitHub's contribution tracking
- In project documentation when appropriate

## Legal

### License

By contributing to SwissArmyHammer, you agree that your contributions will be licensed under the same license as the project (MIT or Apache-2.0).

### Copyright

- You retain copyright of your contributions
- You grant the project permission to use your contributions
- You confirm you have the right to make the contribution
- You agree your contribution does not violate any third-party rights

### Contributor License Agreement

Currently, no formal CLA is required, but this may change as the project grows. Contributors will be notified if a CLA becomes necessary.

## Resources

### Useful Links

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [mdBook Guide](https://rust-lang.github.io/mdBook/)
- [Model Context Protocol](https://github.com/anthropics/model-context-protocol)

### Tools and Services

- **CI/CD**: GitHub Actions
- **Code Coverage**: Codecov
- **Documentation**: GitHub Pages with mdBook
- **Package Registry**: crates.io
- **Binary Releases**: GitHub Releases

Thank you for contributing to SwissArmyHammer! Your efforts help make AI-powered development tools more accessible and powerful for everyone.