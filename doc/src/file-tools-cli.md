# File Tools CLI Usage

This document provides comprehensive examples and usage patterns for the SwissArmyHammer file tools through the command-line interface.

## Command Structure

All file tool commands follow the pattern:
```bash
sah file <tool-name> [OPTIONS] <ARGUMENTS>
```

Where `<tool-name>` is one of: `read`, `write`, `edit`, `glob`, `grep`

## File Read Tool

### Basic Usage

Read entire file:
```bash
sah file read /workspace/src/main.rs
```

Read specific section of large file:
```bash
sah file read /workspace/logs/app.log --offset 100 --limit 50
```

Read first 20 lines:
```bash
sah file read /workspace/README.md --limit 20
```

Read from line 50 onwards:
```bash
sah file read /workspace/data/large_file.csv --offset 50
```

### Advanced Options

```bash
# Read binary file (outputs base64)
sah file read /workspace/assets/logo.png

# Read with maximum offset and limit
sah file read /workspace/logs/debug.log --offset 999999 --limit 100000
```

### Use Cases

```bash
# Examine configuration files
sah file read /workspace/config/settings.toml

# View documentation
sah file read /workspace/docs/API.md --limit 30

# Check log file tail
sah file read /workspace/logs/error.log --offset 1000

# Read source code for analysis  
sah file read /workspace/src/parser.rs
```

## File Write Tool

### Basic Usage

Create new file:
```bash
sah file write /workspace/src/new_module.rs "//! New module\n\npub fn hello() {\n    println!(\"Hello, world!\");\n}"
```

Write configuration file:
```bash
sah file write /workspace/config.toml "[database]\nurl = \"postgresql://localhost:5432/mydb\"\nmax_connections = 10"
```

Create empty file:
```bash
sah file write /workspace/empty_file.txt ""
```

### Advanced Usage

```bash
# Write with Unicode content
sah file write /workspace/unicode.txt "Hello 🦀 Rust!\n你好世界\nПривет мир"

# Create nested directories automatically
sah file write /workspace/deeply/nested/directory/file.txt "Content in nested location"

# Overwrite existing files
sah file write /workspace/existing.txt "Completely new content"
```

### Use Cases

```bash
# Generate configuration files
sah file write /workspace/.env "DEBUG=true\nPORT=3000\nDATABASE_URL=postgresql://localhost/mydb"

# Create source files from templates
sah file write /workspace/src/model.rs "use serde::{Deserialize, Serialize};\n\n#[derive(Debug, Serialize, Deserialize)]\npub struct Model {\n    pub id: u64,\n    pub name: String,\n}"

# Write documentation
sah file write /workspace/docs/getting-started.md "# Getting Started\n\nThis guide will help you get started with our application.\n\n## Installation\n\nRun the following command:\n\n\`\`\`bash\ncargo install myapp\n\`\`\`"

# Create test fixtures
sah file write /workspace/tests/fixtures/sample.json "{\"name\": \"test\", \"value\": 42}"
```

## File Edit Tool

### Basic Usage

Single replacement:
```bash
sah file edit /workspace/src/config.rs "const DEBUG: bool = true;" "const DEBUG: bool = false;"
```

Replace all occurrences:
```bash
sah file edit /workspace/src/main.rs "old_variable_name" "new_variable_name" --replace-all
```

### Advanced Usage

```bash
# Replace with Unicode content
sah file edit /workspace/greeting.txt "Hello World!" "Hello 🌍!"

# Replace multiline strings
sah file edit /workspace/src/lib.rs "fn old_implementation() {\n    todo!();\n}" "fn new_implementation() {\n    println!(\"Implemented!\");\n}"

# Case-sensitive replacement
sah file edit /workspace/code.js "myFunction" "myNewFunction" --replace-all
```

### Use Cases

```bash
# Update configuration values
sah file edit /workspace/config.toml "debug = false" "debug = true"

# Refactor code
sah file edit /workspace/src/parser.rs "parse_old_format" "parse_new_format" --replace-all

# Fix bugs with targeted changes
sah file edit /workspace/src/calculator.rs "result = a + b + 1" "result = a + b"

# Update version numbers
sah file edit /workspace/Cargo.toml "version = \"0.1.0\"" "version = \"0.2.0\""

# Update import statements
sah file edit /workspace/src/main.rs "use old::module" "use new::module"
```

## File Glob Tool

### Basic Usage

Find all Rust files:
```bash
sah file glob "**/*.rs"
```

Find files in specific directory:
```bash
sah file glob "**/*.js" --path /workspace/src
```

Case-sensitive search:
```bash
sah file glob "**/README*" --case-sensitive
```

### Advanced Usage

```bash
# Find test files with complex patterns
sah file glob "**/*{test,spec}.{js,ts,rs}"

# Search ignoring git patterns
sah file glob "**/*" --no-git-ignore

# Find configuration files
sah file glob "**/*.{toml,yaml,yml,json}" --path /workspace/config

# Find recently modified files (sorted by modification time)
sah file glob "**/*.md"
```

### Use Cases

```bash
# Find source files for compilation
sah file glob "src/**/*.rs"

# Locate all configuration files
sah file glob "**/*.{toml,yaml,json,env}"

# Find documentation files
sah file glob "**/*.{md,rst,txt}" --path /workspace/docs

# Discover test files
sah file glob "**/*test*" --case-insensitive

# Find all TypeScript definition files
sah file glob "**/*.d.ts"

# Locate shell scripts
sah file glob "**/*.{sh,bash}"

# Find image assets
sah file glob "**/*.{png,jpg,jpeg,gif,svg}" --path /workspace/assets
```

## File Grep Tool

### Basic Usage

Find function definitions:
```bash
sah file grep "fn\\s+\\w+\\s*\\(" --type rust
```

Search for TODO comments:
```bash
sah file grep "TODO|FIXME" --case-insensitive --context-lines 2
```

Find import statements:
```bash
sah file grep "import.*React" --glob "**/*.{js,jsx,ts,tsx}"
```

### Advanced Usage

```bash
# Count error handling patterns
sah file grep "catch\\s*\\(|Result<.*,.*>" --output-mode count

# Find configuration keys
sah file grep "config\\.[A-Z_]+" --path /workspace/src/config --type js

# Search for API endpoints
sah file grep "(GET|POST|PUT|DELETE)\\s+['\"/]" --context-lines 1

# Find security-related patterns
sah file grep "(password|secret|token|key)\\s*=" --case-insensitive
```

### Output Modes

```bash
# Show matching content (default)
sah file grep "error" --output-mode content

# Show only filenames with matches
sah file grep "deprecated" --output-mode files-with-matches

# Count matches only
sah file grep "TODO" --output-mode count
```

### Use Cases

```bash
# Code analysis and review
sah file grep "unsafe" --type rust --context-lines 3
sah file grep "TODO|FIXME|HACK" --case-insensitive --context-lines 1

# Security auditing
sah file grep "(password|secret|api[_-]?key)" --case-insensitive
sah file grep "eval\\s*\\(" --type js --context-lines 2

# API discovery
sah file grep "async\\s+fn" --type rust
sah file grep "export\\s+(function|const|let)" --type js

# Dependency analysis
sah file grep "use\\s+[a-zA-Z]" --type rust --output-mode files-with-matches
sah file grep "import.*from" --glob "**/*.{js,ts}"

# Documentation analysis
sah file grep "//!|///|#\\[doc" --type rust
sah file grep "TODO:|NOTE:|WARNING:" --context-lines 1

# Performance analysis
sah file grep "(clone|unwrap)\\s*\\(" --type rust --context-lines 1
sah file grep "console\\.(log|debug|info)" --type js
```

## Tool Composition Examples

### Sequential Operations

```bash
# Find and read configuration files
sah file glob "**/*.toml" | xargs -I {} sah file read {}

# Search for functions and examine them
sah file grep "pub fn" --type rust --output-mode files-with-matches | \
  xargs -I {} sah file read {} --limit 20

# Find test files and check for coverage
sah file glob "**/test*.rs" | \
  xargs -I {} sah file grep "assert" --path {}
```

### Complex Workflows

```bash
# Refactoring workflow
# 1. Find all files using old API
sah file grep "old_api_function" --output-mode files-with-matches

# 2. Update each file (iterate over results)
sah file edit /workspace/src/module1.rs "old_api_function" "new_api_function" --replace-all
sah file edit /workspace/src/module2.rs "old_api_function" "new_api_function" --replace-all

# 3. Verify changes
sah file grep "new_api_function" --output-mode count
```

### Batch Processing

```bash
# Process all configuration files
for file in $(sah file glob "**/*.toml"); do
  echo "Processing $file"
  sah file edit "$file" "old_setting = false" "old_setting = true"
done

# Update copyright headers in source files
sah file glob "**/*.rs" | while read file; do
  sah file edit "$file" "Copyright 2023" "Copyright 2024"
done
```

## Error Handling and Troubleshooting

### Common Errors

```bash
# Path validation errors
sah file read "relative/path.txt"  # Error: path must be absolute

# Permission errors  
sah file write /etc/passwd "content"  # Error: permission denied

# File not found
sah file read /nonexistent/file.txt  # Error: file not found

# Invalid parameters
sah file read /workspace/file.txt --offset -1  # Error: invalid offset
sah file read /workspace/file.txt --limit 0    # Error: invalid limit
```

### Validation Messages

```bash
# Content size limits
sah file write /workspace/huge.txt "$(yes | head -c 10000001)"  # Error: content exceeds 10MB limit

# Pattern validation
sah file glob ""  # Error: pattern cannot be empty

# String replacement validation
sah file edit /workspace/file.txt "nonexistent" "replacement"  # Error: string not found
sah file edit /workspace/file.txt "duplicate" "replacement"    # Error: multiple matches found (use --replace-all)
```

### Security Warnings

```bash
# Workspace boundary violations
sah file read /etc/passwd  # Error: outside workspace boundaries

# Path traversal attempts
sah file read "/workspace/../../../etc/passwd"  # Error: blocked pattern '../'
```

## Performance Tips

### Large File Handling

```bash
# Use offset and limit for large files
sah file read /workspace/large.log --offset 10000 --limit 100

# Use specific file types in grep for better performance
sah file grep "pattern" --type rust  # Faster than --glob "**/*.rs"
```

### Batch Operations

```bash
# Use glob patterns efficiently
sah file glob "**/*.{rs,js,py}"  # Better than multiple separate globs

# Limit results when appropriate
sah file grep "common_pattern" --output-mode count  # Faster than full content
```

### Memory Management

```bash
# Process files individually rather than loading all results
sah file glob "**/*.rs" | head -10 | xargs -I {} sah file read {}

# Use appropriate context limits
sah file grep "error" --context-lines 1  # Instead of excessive context
```

## Integration with Other Tools

### Version Control Integration

```bash
# Find modified files and check their content
git diff --name-only | xargs -I {} sah file read {}

# Search for patterns in staged files
git diff --cached --name-only | xargs -I {} sah file grep "TODO" --path {}
```

### Build System Integration  

```bash
# Find source files for compilation
sah file glob "src/**/*.rs" > build_files.txt

# Check for common issues before build
sah file grep "(unwrap|expect|panic)" --type rust --output-mode count
```

### CI/CD Integration

```bash
# Validate configuration files
sah file glob "**/*.{toml,yaml,json}" | while read config; do
  echo "Validating $config"
  sah file read "$config" > /dev/null || echo "Invalid config: $config"
done

# Check for security patterns
if sah file grep "(password|secret)" --case-insensitive --output-mode count | grep -q -v "^0$"; then
  echo "WARNING: Potential security issues found"
  exit 1
fi
```

The file tools provide powerful CLI capabilities for file system operations with comprehensive security, validation, and error handling. Use these examples as starting points for your specific development workflows and automation needs.