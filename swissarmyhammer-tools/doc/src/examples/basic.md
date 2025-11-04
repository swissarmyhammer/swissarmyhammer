# Basic Examples

This page contains basic examples of using SwissArmyHammer Tools.

## File Operations

Reading a file:
```
Ask Claude: "Read the file src/main.rs"
Claude uses: files_read
```

## Semantic Search

Searching code:
```
Ask Claude: "Index all Rust files then search for error handling"
Claude uses: search_index, then search_query
```

## Issue Tracking

Creating an issue:
```
Ask Claude: "Create an issue for implementing OAuth support"
Claude uses: issue_create
```

See [Advanced Examples](advanced.md) for more complex patterns.
