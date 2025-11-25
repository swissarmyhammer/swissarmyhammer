Create project-specific rules from specifications.

## Parameters

- `name` (required): Rule name with optional subdirectory path (e.g., "code-quality/no-global-state")
- `content` (required): Rule checking instructions in markdown
- `severity` (required): Severity level - "error", "warning", "info", or "hint"
- `tags` (optional): Array of tags for filtering and organization (e.g., ["maintainability", "testing"])

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

## File Format

Rules are created in `.swissarmyhammer/rules/` with minimal YAML frontmatter:

```yaml
---
severity: error
tags: ["maintainability", "testing"]
---

Check for global mutable state...
```

## Directory Structure

- Rule names can include subdirectories separated by forward slashes
- Parent directories are created automatically
- Category is derived from directory structure, not stored in frontmatter
- Example: `"code-quality/no-magic-numbers"` creates `.swissarmyhammer/rules/code-quality/no-magic-numbers.md`

## Returns

Returns confirmation with:
- File path where the rule was created
- Rule name
- Severity level

## Validation

The tool validates:
- Name is not empty
- Content is not empty
- Severity is one of: "error", "warning", "info", "hint"

Invalid input returns an error without creating any files.
