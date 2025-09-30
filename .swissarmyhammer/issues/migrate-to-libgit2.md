# Migrate All Git Operations to libgit2

## Problem

Currently, the codebase shells out to git commands using shell execution. This approach has several drawbacks:

- **Performance**: Process spawning overhead for each git operation
- **Portability**: Depends on git being installed and in PATH
- **Error Handling**: Parsing text output is fragile and error-prone
- **Security**: Shell injection risks if inputs aren't properly sanitized
- **Reliability**: Different git versions may have different output formats

## Solution

Replace all git shell commands with libgit2 (via `git2` crate in Rust).

## Benefits

- **Performance**: Direct library calls, no process spawning
- **Type Safety**: Structured data instead of parsing text output
- **Portability**: No external git dependency required
- **Reliability**: Consistent behavior across environments
- **Better Error Handling**: Proper error types instead of exit codes
- **Feature Rich**: Access to full git internals programmatically

## Affected Components

Audit the codebase for all instances of:
- Shell commands calling `git` (via `shell_execute` or similar)
- Parsing git command output (status, diff, log, etc.)
- Git operations in tools like `git_changes`

## Implementation Steps

1. Add `git2` crate dependency
2. Identify all git shell commands in codebase
3. Replace each with equivalent libgit2 API calls
4. Update error handling to use git2 error types
5. Add tests to verify equivalent behavior
6. Remove shell-based git helpers

## Examples of Migration

**Before** (shell):
```rust
Command::new("git")
    .args(["diff", "--name-only", "HEAD"])
    .output()
```

**After** (libgit2):
```rust
let repo = Repository::open(".")?;
let head = repo.head()?.peel_to_tree()?;
// Use libgit2 diff APIs
```

## Related

- Improves `git_changes` tool reliability
- Enables better git integration throughout the system
- Foundation for future git-based features