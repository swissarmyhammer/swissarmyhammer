Git operations for analyzing branch changes and semantic diffs.

## Operations

### get changes

List files changed on a branch relative to its parent, including uncommitted changes.

```json
{"op": "get changes"}
{"op": "get changes", "branch": "issue/feature-123"}
{"op": "get changes", "range": "HEAD~3..HEAD"}
```

**Behavior**: Feature branch → diff from parent + uncommitted. Main + uncommitted → uncommitted only. Main + clean → defaults to `HEAD~1..HEAD`. `range` always takes precedence.

### get diff

Semantic diff at the entity level (functions, classes) using tree-sitter.

```json
{"op": "get diff"}
{"op": "get diff", "left": "src/main.rs@HEAD~1", "right": "src/main.rs"}
{"op": "get diff", "left_text": "fn foo() {}", "right_text": "fn foo(x: i32) {}", "language": "rust"}
```

Auto-detect mode (no params) diffs dirty/staged files. Returns summary counts and semantic changes (added, modified, deleted, moved, renamed).
