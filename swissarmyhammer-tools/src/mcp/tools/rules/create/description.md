Create project-specific rules from specifications.

## Examples

Basic rule creation:
```json
{
  "name": "no-global-state",
  "content": "Check for global mutable state that makes testing difficult...",
  "severity": "error"
}
```

Rule with subdirectory and tags:
```json
{
  "name": "code-quality/cognitive-complexity",
  "content": "Check for functions with high cognitive complexity...",
  "severity": "warning",
  "tags": ["maintainability", "readability"]
}
```

## Directory Structure

- Rule names can include subdirectories separated by forward slashes
- Parent directories are created automatically
- Category is derived from directory structure, not stored in frontmatter
- Example: `"code-quality/no-magic-numbers"` creates `.swissarmyhammer/rules/code-quality/no-magic-numbers.md`
