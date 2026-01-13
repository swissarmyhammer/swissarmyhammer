Fast file pattern matching with .gitignore support.

## Pattern Guidelines

**CRITICAL: Avoid overly broad patterns.** Never use patterns that match recursively by extension alone:

❌ **NEVER use these patterns:**
- `*` - matches all files in directory
- `**/*` - matches all files recursively
- `*.*` - matches all files with extensions
- `**/*.rs` - matches ALL Rust files in entire project
- `**/*.py` - matches ALL Python files in entire project
- `**/*.js` - matches ALL JavaScript files in entire project

These patterns can match thousands of files, causing:
- Performance issues and rate limiting (max 10,000 files)
- Context overflow from excessive results
- Difficulty processing the output

✅ **Instead: Scope patterns to specific directories:**
- `src/**/*.rs` - Rust files in src directory only
- `tests/**/*.py` - Python files in tests directory only
- `*.json` - JSON files in root directory only

**Better approach: Use multiple small, targeted globs:**

### Project Discovery Strategy

When exploring a new codebase, use multiple small, targeted globs in sequence:

1. **Start with root configuration files** (one glob per file type):
   ```json
   {"pattern": "*.json"}        // package.json, tsconfig.json, etc.
   {"pattern": "*.toml"}        // Cargo.toml, pyproject.toml, etc.
   {"pattern": "*.yaml"}        // .github/workflows, docker-compose.yaml
   {"pattern": "*.yml"}         // Alternative YAML extension
   {"pattern": "*.lock"}        // package-lock.json, Cargo.lock, etc.
   ```

2. **Then explore by project type** (examples for common ecosystems):

   **JavaScript/TypeScript:**
   ```json
   {"pattern": "src/**/*.ts"}
   {"pattern": "src/**/*.tsx"}
   {"pattern": "test/**/*.test.js"}
   ```

   **Rust:**
   ```json
   {"pattern": "src/**/*.rs"}
   {"pattern": "tests/**/*.rs"}
   ```

   **Python:**
   ```json
   {"pattern": "src/**/*.py"}
   {"pattern": "tests/**/*.py"}
   {"pattern": "lib/**/*.py"}
   ```

   **Go:**
   ```json
   {"pattern": "cmd/**/*.go"}
   {"pattern": "pkg/**/*.go"}
   {"pattern": "internal/**/*.go"}
   ```

3. **Then look for specific directories or file types:**
   ```json
   {"pattern": "docs/**/*.md"}
   {"pattern": ".github/**/*.yml"}
   {"pattern": "scripts/**/*.sh"}
   ```

### Examples

Good (specific, targeted):
```json
{
  "pattern": "src/**/*.rs"
}
```

Bad (too broad, avoid):
```json
{
  "pattern": "*"           // Matches everything in current dir
}
{
  "pattern": "**/*"        // Matches everything recursively
}
{
  "pattern": "*.*"         // Matches all files with extensions
}
```

## Returns

Returns file count and list of matching file paths sorted by modification time (up to 10,000 files).
