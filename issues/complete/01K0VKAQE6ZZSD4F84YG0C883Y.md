Memos are being stored as JSON. Store them as pure markdown instead.

The id can be computed from the file name `<title>.md`.

There is no need for title, eliminate it.

The content is the body of the file.

The create and update dates can be read from the filesystem.

Memos are being stored as JSON. Store them as pure markdown instead.

The id can be computed from the file name `<title>.md`.

There is no need for title, eliminate it.

The content is the body of the file.

The create and update dates can be read from the filesystem.

## Proposed Solution

Based on my analysis of the current implementation, here's my planned approach:

### Current System Analysis
- Memos are stored as JSON files in `.swissarmyhammer/memos/` directory  
- Each memo file is named `{ulid}.json` and contains the full `Memo` struct with metadata
- The `Memo` struct has: id (ULID), title, content, created_at, updated_at
- The `FileSystemMemoStorage` implements the `MemoStorage` trait

### Implementation Steps

1. **Create New Storage Implementation**
   - Implement a new `MarkdownMemoStorage` that stores memos as pure markdown files
   - Files will be named `{title}.md` instead of `{ulid}.json`
   - Content will be the raw markdown without JSON wrapping

2. **Update Data Model**
   - Modify the `Memo` struct to support markdown-based storage
   - ID will be computed from the filename (without .md extension)
   - Timestamps will be derived from filesystem metadata using `std::fs::metadata()`

3. **File Operations Changes**
   - **Create**: Save content as `{title}.md` file
   - **Read**: Load raw markdown content, get timestamps from filesystem
   - **Update**: Overwrite file content, filesystem will update mtime
   - **Delete**: Remove the `.md` file
   - **List**: Enumerate `.md` files in the directory

4. **ID Generation Strategy**
   - Use the filename (without .md extension) as the memo ID
   - Handle special characters and ensure filesystem safety
   - Provide mapping between human-readable titles and IDs

5. **Backwards Compatibility**
   - Add migration logic to convert existing JSON memos to markdown format
   - Maintain the same MCP API interface so clients continue to work

6. **Key Benefits**
   - Simpler file format (pure markdown)
   - Human-readable filenames
   - Easier to version control and edit externally
   - Automatic timestamp management via filesystem

### Files to Modify
- `swissarmyhammer/src/memoranda/storage.rs` - Add new markdown storage implementation
- `swissarmyhammer/src/memoranda/mod.rs` - Update exports and data structures
- `swissarmyhammer/src/mcp/tool_handlers.rs` - Ensure compatibility with new storage
- `swissarmyhammer/tests/mcp_memoranda_tests.rs` - Update tests for markdown format