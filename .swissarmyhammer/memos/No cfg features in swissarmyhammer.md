# No cfg features in swissarmyhammer

This is a project rule that must be enforced: **Never introduce `#[cfg(feature = "...")]` conditional compilation in the swissarmyhammer codebase.**

## Why This Rule Exists

1. **Simplicity**: All code should work all the time, without conditional compilation
2. **No Feature Flags**: We don't want feature flags complicating the build or runtime
3. **Always Available**: All functionality should be available by default
4. **User Confusion**: Features create confusion about what's available
5. **Testing Complexity**: Features make testing more complex and error-prone

## What This Means

- ❌ Never use `#[cfg(feature = "dynamic-cli")]` or similar
- ❌ Don't create optional features in Cargo.toml  
- ❌ Don't make functions conditionally available
- ✅ All code should compile and be available by default
- ✅ Use runtime checks instead of compile-time features if needed

## The Problem We Saw

The codebase had `#[cfg(feature = "dynamic-cli")]` attributes scattered throughout, making essential CLI functions unavailable because the feature was never defined or enabled. This broke the CLI completely with silent failures.

## The Solution

Remove all cfg feature attributes and make all code unconditionally available. If you need to disable something, do it at runtime, not compile time.

## Examples

**❌ Don't do this:**
```rust
#[cfg(feature = "dynamic-cli")]
pub fn get_tool_registry(&self) -> &ToolRegistry {
    &self.tool_registry
}
```

**✅ Do this instead:**
```rust
pub fn get_tool_registry(&self) -> &ToolRegistry {
    &self.tool_registry
}
```

**If you need conditional behavior, use runtime checks:**
```rust
pub fn get_tool_registry(&self) -> Option<&ToolRegistry> {
    if self.is_tools_enabled() {
        Some(&self.tool_registry)
    } else {
        None
    }
}
```

## Remember

**All code needs to work all the time. No features 'in' swissarmyhammer. Ever.**