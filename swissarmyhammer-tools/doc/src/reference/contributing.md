# Contributing

Thank you for your interest in contributing to SwissArmyHammer Tools! This guide will help you get started.

## Development Setup

### Prerequisites

- Rust 1.70 or later
- Git
- Optional: mdBook for documentation

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/swissarmyhammer/swissarmyhammer
cd swissarmyhammer/swissarmyhammer-tools

# Build the project
cargo build

# Run tests
cargo nextest run

# Run specific tests
cargo nextest run --test test_name
```

### Project Structure

```
swissarmyhammer-tools/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── mcp/               # MCP server implementation
│   │   ├── server.rs      # Core server
│   │   ├── tool_registry.rs  # Tool management
│   │   └── tools/        # Tool implementations
│   │       ├── files/    # File operations
│   │       ├── search/   # Semantic search
│   │       └── ...       # Other categories
│   └── test_utils/       # Testing utilities
├── tests/                # Integration tests
├── doc/                  # Documentation source
└── Cargo.toml           # Dependencies
```

## Adding a New Tool

### 1. Create Tool Module

Create a new module in `src/mcp/tools/<category>/`:

```rust,ignore
// src/mcp/tools/files/my_tool/mod.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::mcp::{McpTool, ToolContext};

#[derive(Debug, Deserialize)]
struct MyToolParams {
    required_param: String,
    #[serde(default)]
    optional_param: Option<String>,
}

#[derive(Debug, Serialize)]
struct MyToolResult {
    output: String,
    status: String,
}

pub struct MyTool;

#[async_trait]
impl McpTool for MyTool {
    fn name(&self) -> &str {
        "files_my_tool"
    }

    fn description(&self) -> &str {
        "Brief description of what this tool does"
    }

    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "required_param": {
                    "type": "string",
                    "description": "Description of parameter"
                },
                "optional_param": {
                    "type": "string",
                    "description": "Optional parameter description"
                }
            },
            "required": ["required_param"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        context: Arc<ToolContext>,
    ) -> Result<serde_json::Value> {
        let params: MyToolParams = serde_json::from_value(params)?;

        // Tool implementation here
        let result = MyToolResult {
            output: "result".to_string(),
            status: "success".to_string(),
        };

        Ok(serde_json::to_value(result)?)
    }
}
```

### 2. Register Tool

Add to category registration in `src/mcp/tools/<category>/mod.rs`:

```rust,ignore
pub fn register_file_tools(registry: &mut ToolRegistry) {
    // ... existing tools
    registry.register(Box::new(MyTool));
}
```

### 3. Write Tests

Create tests in `src/mcp/tools/<category>/my_tool/mod.rs`:

```rust,ignore
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_my_tool_success() {
        let tool = MyTool;
        let context = create_test_context().await;

        let params = json!({
            "required_param": "test"
        });

        let result = tool.execute(params, context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_my_tool_missing_required_param() {
        let tool = MyTool;
        let context = create_test_context().await;

        let params = json!({});

        let result = tool.execute(params, context).await;
        assert!(result.is_err());
    }
}
```

### 4. Document Tool

Add documentation in `doc/src/features/` and update `SUMMARY.md`.

## Testing Guidelines

### Unit Tests

Test individual functions and components:

```rust,ignore
#[tokio::test]
async fn test_function() {
    let result = my_function().await;
    assert_eq!(result, expected);
}
```

### Integration Tests

Test complete tool execution:

```rust,ignore
#[tokio::test]
async fn test_tool_integration() {
    let server = McpServer::new(...).await?;
    let result = server.execute_tool("tool_name", params).await?;
    // Verify result
}
```

### Property-Based Tests

Use proptest for property testing:

```rust,ignore
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_property(input in any::<String>()) {
        // Test property holds for any input
    }
}
```

## Code Style

### Formatting

Use rustfmt:

```bash
cargo fmt
cargo fmt -- --check  # CI check
```

### Linting

Use clippy:

```bash
cargo clippy
cargo clippy -- -D warnings  # CI check
```

### Documentation

- Document all public items
- Use examples in doc comments
- Keep comments concise and clear

Example:

```rust,ignore
/// Read file contents with optional offset and limit.
///
/// # Parameters
///
/// - `path`: Path to file (relative to working directory)
/// - `offset`: Starting line number (optional)
/// - `limit`: Maximum lines to read (optional)
///
/// # Returns
///
/// File contents with metadata including encoding and line counts.
///
/// # Examples
///
/// ```rust
/// let params = json!({
///     "path": "Cargo.toml"
/// });
/// let result = tool.execute(params, context).await?;
/// ```
pub async fn execute(...) -> Result<...> {
    // Implementation
}
```

## Pull Request Process

### 1. Fork and Branch

```bash
# Fork the repository on GitHub
# Clone your fork
git clone https://github.com/YOUR_USERNAME/swissarmyhammer

# Create a branch
git checkout -b feature/my-new-tool
```

### 2. Make Changes

- Follow code style guidelines
- Write tests for new functionality
- Update documentation

### 3. Test

```bash
# Run all tests
cargo nextest run

# Run clippy
cargo clippy

# Format code
cargo fmt

# Build documentation
cd doc && mdbook build
```

### 4. Commit

Use conventional commit messages:

```bash
git commit -m "feat: add new tool for X"
git commit -m "fix: resolve issue with Y"
git commit -m "docs: update tool documentation"
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `test`: Test additions or changes
- `refactor`: Code refactoring
- `chore`: Maintenance tasks

### 5. Push and Create PR

```bash
git push origin feature/my-new-tool
```

Then create a pull request on GitHub with:
- Clear description of changes
- Link to any related issues
- Screenshots if applicable

### 6. Review Process

- Maintainers will review your PR
- Address any feedback
- Once approved, maintainers will merge

## Documentation

### Building Documentation

```bash
cd doc
mdbook build
mdbook serve  # Preview locally
```

### Documentation Structure

- `introduction.md`: Overview
- `getting-started.md`: Installation and setup
- `architecture/`: System design
- `features/`: Tool documentation
- `troubleshooting.md`: Common issues
- `reference/`: API and tool reference

## Release Process

Releases are managed by maintainers:

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Create git tag
4. Publish to crates.io
5. Create GitHub release

## Community

- **GitHub Issues**: Bug reports and feature requests
- **Discussions**: Questions and general discussion
- **Pull Requests**: Code contributions

## Code of Conduct

Be respectful and professional in all interactions. We're here to build great software together.

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.

## Questions?

If you have questions about contributing:
- Open an issue on GitHub
- Check existing documentation
- Ask in GitHub Discussions

Thank you for contributing to SwissArmyHammer Tools!
