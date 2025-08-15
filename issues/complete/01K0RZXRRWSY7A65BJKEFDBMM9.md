Get rid of CachedIssueStorage -- no caching, it is just trouble. Reading the filesystem will be fine and fast.

## Proposed Solution

After analyzing the codebase, I can see that `CachedIssueStorage` is not actually used in production code - the main application uses `FileSystemIssueStorage` directly. The caching layer adds unnecessary complexity for minimal benefit.

My implementation plan:

1. **Remove the `CachedIssueStorage` struct and module** - Delete `cached_storage.rs` entirely
2. **Remove exports from `mod.rs`** - Remove the `cached_storage` module and `CachedIssueStorage` export
3. **Update benchmarks** - Replace `CachedIssueStorage` usage in benchmarks with `FileSystemIssueStorage`
4. **Remove the cache module** - Delete `cache.rs` since it's only used by `CachedIssueStorage`
5. **Clean up dependencies** - Remove any cache-related dependencies from `Cargo.toml` if they're no longer needed

The filesystem operations are already fast enough for typical issue management workloads, and removing the caching layer simplifies the architecture significantly.