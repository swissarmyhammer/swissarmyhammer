---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffd980
title: Add test for shelltool-cli cli::InstallTarget Display impl
---
shelltool-cli/src/cli.rs:21-27

Coverage: 0/6 (0.0%)

Uncovered lines: 21-25, 27

```rust
impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallTarget::Project => write!(f, "project"),
            InstallTarget::Local => write!(f, "local"),
            InstallTarget::User => write!(f, "user"),
        }
    }
}
```

The `Display` impl on `InstallTarget` is completely uncovered. Trivially testable.

**What to test:**
- `InstallTarget::Project.to_string() == "project"`
- `InstallTarget::Local.to_string() == "local"`
- `InstallTarget::User.to_string() == "user"`
- Optionally: `format!("{}", InstallTarget::Project)` for the formatter path.

Add a `#[cfg(test)] mod tests` to `cli.rs` (does not currently exist) with a single test asserting each variant's string representation. #coverage-gap