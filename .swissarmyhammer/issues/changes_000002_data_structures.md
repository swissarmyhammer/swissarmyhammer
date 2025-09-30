# Step 2: Define Request and Response Data Structures

Refer to ideas/changes.md

## Objective

Create the data structures for the git_changes tool request and response.

## Tasks

1. Define `GitChangesRequest` struct
   - `branch: String` - Branch name to analyze
   - Add serde derives for JSON serialization
   - Add documentation

2. Define `GitChangesResponse` struct
   - `branch: String` - The analyzed branch
   - `parent_branch: Option<String>` - Parent branch if determined
   - `files: Vec<String>` - List of changed file paths
   - Add serde derives for JSON serialization
   - Add documentation

3. Add unit tests for serialization/deserialization

## Success Criteria

- Structs compile and serialize/deserialize correctly
- Tests pass with `cargo nextest run`
- Documentation is complete

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/git/changes/mod.rs`

## Estimated Code Changes

~60 lines