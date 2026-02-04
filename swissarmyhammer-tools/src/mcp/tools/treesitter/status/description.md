Get the current status of the tree-sitter code index.

This tool reports whether the index is running, how many files have been indexed, and the current progress of indexing operations.

## Examples

Check status for current workspace:
```json
{}
```

Check status for a specific project:
```json
{
  "path": "/path/to/project"
}
```

## Returns

Returns index status information including:
- Whether the index is ready or still building
- Total files discovered
- Files indexed and embedded
- Indexing progress percentage
