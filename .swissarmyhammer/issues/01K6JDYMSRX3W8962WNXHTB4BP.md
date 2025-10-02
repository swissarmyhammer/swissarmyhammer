# Centralize FileSource emoji display mapping

## Problem
The emoji display for file sources (📦 Built-in, 📁 Project, 👤 User) is duplicated across multiple files. Each command module implements its own mapping from `FileSource` to emoji strings.

## Current Duplication
The mapping appears in at least 5 places:

1. **swissarmyhammer-cli/src/commands/agent/display.rs:17-19**
   ```rust
   match source {
       AgentSource::Builtin => "📦 Built-in",
       AgentSource::Project => "📁 Project",
       AgentSource::User => "👤 User",
   }
   ```

2. **swissarmyhammer-cli/src/commands/flow/display.rs:12-15**
   ```rust
   match source {
       Some(FileSource::Builtin) => "📦 Built-in",
       Some(FileSource::Local) => "📁 Project",
       Some(FileSource::User) => "👤 User",
       Some(FileSource::Dynamic) | None => "📦 Built-in",
   }
   ```

3. **swissarmyhammer-cli/src/commands/prompt/display.rs:10-12**
   ```rust
   const BUILTIN_EMOJI: &str = "📦 Built-in";
   const PROJECT_EMOJI: &str = "📁 Project";
   const USER_EMOJI: &str = "👤 User";
   ```

4. **swissarmyhammer-cli/src/commands/rule/display.rs:10-12**
   ```rust
   const BUILTIN_EMOJI: &str = "📦 Built-in";
   const PROJECT_EMOJI: &str = "📁 Project";
   const USER_EMOJI: &str = "👤 User";
   ```

5. **swissarmyhammer-cli/src/commands/rule/validate.rs:68-71**
   ```rust
   FileSource::Builtin => "📦 Built-in",
   FileSource::Local => "📁 Project",
   FileSource::User => "👤 User",
   FileSource::Dynamic => "📦 Built-in",
   ```

## Issues
1. **Inconsistent naming**: Some use `FileSource`, some use `AgentSource`, some use constants
2. **Maintenance burden**: Changes require updates in 5+ locations
3. **Risk of inconsistency**: Easy to update one place and miss others
4. **No single source of truth**: Different modules may diverge over time

## Proposed Solution

### Option 1: Add display method to FileSource in swissarmyhammer-common
Add a method to the existing `FileSource` enum in `swissarmyhammer-common`:

```rust
// In swissarmyhammer-common/src/file_loader.rs
impl FileSource {
    /// Get emoji-based display string for the file source
    ///
    /// - 📦 Built-in: System-provided built-in items
    /// - 📁 Project: Project-specific items from .swissarmyhammer directory
    /// - 👤 User: User-specific items from user's home directory
    pub fn display_emoji(&self) -> &'static str {
        match self {
            FileSource::Builtin | FileSource::Dynamic => "📦 Built-in",
            FileSource::Local => "📁 Project",
            FileSource::User => "👤 User",
        }
    }
}
```

### Option 2: Create a common display module
Create `swissarmyhammer-cli/src/display_common.rs` with shared display utilities:

```rust
pub fn file_source_emoji(source: &FileSource) -> &'static str {
    match source {
        FileSource::Builtin | FileSource::Dynamic => "📦 Built-in",
        FileSource::Local => "📁 Project",
        FileSource::User => "👤 User",
    }
}
```

## Recommendation
**Option 1** is preferred because:
1. Keeps display logic close to the data type
2. Makes it available to all consumers of `FileSource`
3. Follows Rust idiom of adding display methods to types
4. Easier to discover (IDE autocomplete)
5. No need for separate display module imports

## Implementation Steps
1. Add `display_emoji()` method to `FileSource` in swissarmyhammer-common
2. Update all 5+ locations to use `source.display_emoji()`
3. Remove duplicate constants and inline match expressions
4. Update tests to verify consistent display

## Impact
- Low risk refactoring
- No breaking changes to user interface
- Improves maintainability
- Ensures consistency across all commands
