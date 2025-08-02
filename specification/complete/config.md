# sah.toml Configuration Specification

## Overview

The `sah.toml` file is an optional configuration file that can be placed at the root of a repository to define variables that are always available for rendering in prompts and workflows via Liquid templates. This provides a centralized way to manage project-specific variables, metadata, and configuration values.

## File Location

- **Primary location**: `./sah.toml` (root of repository)
- **Alternative locations**: Not supported (keeps configuration simple and predictable)

## File Format

The configuration file uses TOML format for its simplicity, readability, and wide support across programming languages.

## Basic Structure

```toml
# Project metadata
name = "MyProject"
version = "1.0.0"
description = "A sample project configuration"

# Simple key-value pairs
author = "John Doe"
license = "MIT"
repository = "https://github.com/user/repo"

# Arrays
keywords = ["cli", "tool", "automation"]
maintainers = ["alice@example.com", "bob@example.com"]

# Nested configuration sections
[project]
type = "library"
language = "rust"
framework = "tokio"

[build]
target = "x86_64-unknown-linux-gnu"
features = ["async", "json"]
optimization = "release"

[deployment]
environment = "production"
region = "us-west-2"

[team]
lead = "Alice Smith"
members = ["Bob Jones", "Carol Williams"]

[urls]
homepage = "https://myproject.com"
documentation = "https://docs.myproject.com"
issues = "https://github.com/user/repo/issues"
```

## Variable Access in Liquid Templates

### Simple Variables

Simple key-value pairs are directly accessible:

```liquid
Project: {{ name }} v{{ version }}
Author: {{ author }}
License: {{ license }}
```

### Arrays

Arrays are accessible as Liquid arrays:

```liquid
Keywords: {% for keyword in keywords %}{{ keyword }}{% unless forloop.last %}, {% endunless %}{% endfor %}

Maintainers:
{% for maintainer in maintainers %}
- {{ maintainer }}
{% endfor %}
```

### TOML Sections as Objects

TOML sections become objects in Liquid:

```liquid
Project Type: {{ project.type }}
Language: {{ project.language }}
Framework: {{ project.framework }}

Build Configuration:
- Target: {{ build.target }}
- Features: {% for feature in build.features %}{{ feature }}{% unless forloop.last %}, {% endunless %}{% endfor %}
- Optimization: {{ build.optimization }}

Team Lead: {{ team.lead }}
Team Members: {{ team.members | size }} people
```

### Nested Objects

Deeply nested TOML structures are fully supported:

```toml
[database.primary]
host = "localhost"
port = 5432
name = "myapp"

[database.replica]
host = "replica.example.com"
port = 5432
name = "myapp_readonly"
```

Accessed as:

```liquid
Primary DB: {{ database.primary.host }}:{{ database.primary.port }}/{{ database.primary.name }}
Replica DB: {{ database.replica.host }}:{{ database.replica.port }}/{{ database.replica.name }}
```

## Variable Resolution Rules

1. **Case Sensitivity**: Variable names are case-sensitive
2. **Dot Notation**: TOML sections create nested objects accessible via dot notation
3. **Array Indexing**: Arrays support standard Liquid array operations and filters
4. **Type Preservation**: TOML types (string, integer, float, boolean, datetime) are preserved
5. **Missing Variables**: Missing variables render as empty strings (standard Liquid behavior)

## Documentation Requirements

### Inline Documentation

Use TOML comments to document configuration sections:

```toml
# Project identification and metadata
name = "SwissArmyHammer"
version = "2.0.0"

# Contact information for project maintainers
[contacts]
# Primary project maintainer
lead = "alice@example.com"
# Backup contacts for urgent issues
backup = ["bob@example.com", "carol@example.com"]

# Environment-specific configuration values
[environments.development]
# Local development database connection
database_url = "postgresql://localhost:5432/myapp_dev"
# Enable debug logging in development
debug = true

[environments.production]
# Production database with connection pooling
database_url = "postgresql://prod-db.example.com:5432/myapp"
# Disable debug logging in production
debug = false
```



## Advanced Features

### Environment Variable Substitution

Support environment variable substitution in values:

```toml
# Use environment variables with fallback defaults
database_url = "${DATABASE_URL:-postgresql://localhost:5432/myapp}"
api_key = "${API_KEY}"  # Required environment variable
debug = "${DEBUG:-false}"  # Boolean with default
```

## Validation Rules

The `sah.toml` is checked with `sah validate` on the CLI.

### File Validation

1. **TOML Syntax**: Must be valid TOML syntax
2. **UTF-8 Encoding**: File must be UTF-8 encoded
3. **Size Limits**: Maximum file size of 1MB
4. **Depth Limits**: Maximum nesting depth of 10 levels

### Variable Name Validation

1. **Valid Identifiers**: Variable names must be valid Liquid identifiers
2. **Reserved Names**: Cannot override built-in Liquid variables or filters
3. **Naming Convention**: Recommend snake_case for consistency

### Value Validation

1. **Type Safety**: Values must be valid TOML types
2. **String Limits**: Individual string values limited to 10KB
3. **Array Limits**: Arrays limited to 1000 elements
4. **Circular References**: No circular references in included files

## Error Handling

### Parse Errors

- **Syntax Errors**: Clear error messages with line numbers
- **Type Errors**: Descriptive type mismatch errors
- **Missing Files**: Graceful handling of missing include files

### Runtime Errors

- **Missing Variables**: Empty string rendering (standard Liquid behavior)
- **Type Mismatches**: Automatic type coercion where possible
- **Circular References**: Detection and prevention with clear error messages

### Fallback Behavior

- **Missing sah.toml**: Templates work normally without configuration file
- **Partial Loading**: Continue with successfully parsed sections if some sections fail
- **Environment Overrides**: Environment variables can override file values

## Security Considerations

### Input Validation

1. **Path Traversal**: Prevent directory traversal in include paths
2. **Content Filtering**: Sanitize potentially dangerous content
3. **Size Limits**: Enforce reasonable file and value size limits

## Integration with SwissArmyHammer

### Loading Priority

1. Repository root `sah.toml`
2. Environment variable overrides
3. Built-in defaults

### Template Context

Configuration variables are merged into the template context alongside:
- Built-in variables (git info, timestamps, etc.)
- Workflow state variables
- Prompt-specific variables

### Caching

There is NO CACHING, read the config each time you need it

## Examples

### Minimal Configuration

```toml
name = "HelloWorld"
author = "Developer"
```

### Complete Project Configuration

```toml
# Project metadata
name = "SwissArmyHammer"
version = "2.0.0"
description = "A flexible prompt and workflow management tool"
author = "The SwissArmyHammer Team"
license = "MIT"
homepage = "https://swissarmyhammer.dev"

# Project classification
keywords = ["cli", "automation", "templates", "workflows"]
categories = ["development-tools", "command-line-utilities"]

# Team information
[team]
lead = "alice@example.com"
maintainers = ["bob@example.com", "carol@example.com"]
contributors = ["https://github.com/user/repo/contributors"]

# Build configuration
[build]
language = "rust"
minimum_version = "1.70.0"
features = ["async", "cli", "templates"]
targets = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc", "x86_64-apple-darwin"]

# Deployment settings
[deployment]
registry = "crates.io"
documentation = "docs.rs"
repository = "github.com/user/swissarmyhammer"

# Environment configurations
[environments.development]
debug = true
log_level = "debug"
features = ["dev-tools", "hot-reload"]

[environments.production]
debug = false
log_level = "info"
optimizations = true

# Documentation metadata
[documentation]
description = "Configuration variables for SwissArmyHammer project templates"

[documentation.variables]
name = "Project name used in headers, titles, and generated content"
version = "Semantic version for release management and API compatibility"
author = "Primary author or organization credited in outputs"
team.lead = "Primary contact for project leadership and decisions"
build.language = "Programming language for syntax highlighting and tool selection"
```

This specification provides a comprehensive foundation for implementing `sah.toml` configuration support in SwissArmyHammer, ensuring both simplicity for basic use cases and flexibility for complex project configurations.