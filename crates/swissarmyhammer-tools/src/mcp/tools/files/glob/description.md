Fast file pattern matching with .gitignore support.

**Always scope patterns to specific directories** — never use `**/*.rs` or `**/*` which can match thousands of files. Use `src/**/*.rs`, `tests/**/*.py`, etc.

## Examples

```json
{"pattern": "src/**/*.rs"}
{"pattern": "*.toml"}
{"pattern": "tests/**/*.test.ts"}
```

## Returns

Returns file count and list of matching file paths sorted by modification time (up to 10,000 files).
