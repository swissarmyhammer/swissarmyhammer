I just created memos and they were json files not markdown files as demanded in the last issue.

Fix it.

Memoranda storage is still clearly json in code. It is wrong. Use markdown.


## Proposed Solution

Based on my analysis of the codebase, the issue is that while the system has both JSON-based storage (`FileSystemMemoStorage`) and markdown-based storage (`MarkdownMemoStorage`), the main entry points are still using the legacy JSON storage.

### Root Cause
The `MarkdownMemoStorage` implementation exists and is working correctly, but these key components are still using `FileSystemMemoStorage`:

1. **MCP Server** (`src/mcp.rs:151`) - All MCP memo operations use JSON storage
2. **CLI Implementation** (`swissarmyhammer-cli/src/memo.rs:23`) - All CLI memo commands use JSON storage  
3. **Examples** (`examples/memoranda_usage.rs`) - Documentation examples show JSON usage
4. **Benchmarks** (`benches/memo_benchmarks.rs:11`) - Performance tests use JSON storage

### Implementation Steps

1. **Update MCP Server** - Replace `FileSystemMemoStorage` with `MarkdownMemoStorage` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/mcp.rs`
2. **Update CLI** - Replace `FileSystemMemoStorage` with `MarkdownMemoStorage` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer-cli/src/memo.rs`
3. **Update Examples** - Replace `FileSystemMemoStorage` with `MarkdownMemoStorage` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/examples/memoranda_usage.rs`
4. **Update Benchmarks** - Replace `FileSystemMemoStorage` with `MarkdownMemoStorage` in `/Users/wballard/github/swissarmyhammer/benches/memo_benchmarks.rs`
5. **Run tests** - Ensure all existing tests pass with markdown storage
6. **Add migration support** - Use the existing `migrate_from_json()` method to handle any existing JSON memos

### Expected Outcome
After these changes, all memo operations will store data as markdown files instead of JSON files, fulfilling the requirement in the issue.