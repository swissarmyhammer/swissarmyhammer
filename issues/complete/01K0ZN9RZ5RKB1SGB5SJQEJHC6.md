search index globbing needs to honor .gitignore. the ignore crate is the ticket

## Proposed Solution

The issue is in the `FileIndexer::expand_glob_pattern` method in `swissarmyhammer/src/semantic/indexer.rs:135`. Currently it uses the standard `glob` crate which doesn't respect .gitignore files.

### Implementation Steps:

1. **Add ignore crate dependency**: Add `ignore = "0.4"` to `swissarmyhammer/Cargo.toml`

2. **Replace glob with ignore WalkBuilder**: Update `expand_glob_pattern` method to use `ignore::WalkBuilder` which respects .gitignore, .git/info/exclude, and global gitignore files

3. **Update method signature**: The `ignore` crate uses directory walking rather than glob patterns, so we'll need to modify the approach to:
   - Walk directories starting from the current directory (or specified root)
   - Apply glob pattern matching on the discovered files
   - Respect gitignore rules automatically

4. **Maintain compatibility**: Ensure the existing API still works while adding gitignore support

5. **Add tests**: Write tests to verify that:
   - Files listed in .gitignore are excluded from indexing
   - Files not in .gitignore are included
   - Nested .gitignore files are respected
   - Global and repository-level ignore rules work

### Key Benefits:
- Prevents indexing of build artifacts, dependencies, and temporary files
- Improves privacy by excluding files that developers don't want tracked
- Reduces index size and improves performance
- Follows standard git conventions users expect