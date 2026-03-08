Git operations for analyzing branch changes and semantic diffs.

## Operations

### get changes

List files that have changed on a branch relative to its parent branch, including uncommitted changes.

```json
{"op": "get changes"}
```

```json
{"op": "get changes", "branch": "issue/feature-123"}
```

Returns branch name, parent branch (if detected), and array of changed file paths.

### get diff

Semantic diff at the entity level (functions, classes, etc.) using tree-sitter parsing.

**Inline text mode** -- compare two code snippets directly:

```json
{"op": "get diff", "left_text": "fn foo() {}", "right_text": "fn foo(x: i32) {}", "language": "rust"}
```

**File mode** -- compare files, optionally at different git refs:

```json
{"op": "get diff", "left": "src/main.rs@HEAD~1", "right": "src/main.rs"}
```

**Auto-detect mode** -- no extra parameters, diffs dirty/staged files:

```json
{"op": "get diff"}
```

Returns summary counts and an array of semantic changes (added, modified, deleted, moved, renamed entities).
