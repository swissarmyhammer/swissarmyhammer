---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffc580
title: 'blocker: file_mtime takes &PathBuf instead of &Path'
---
tool_config.rs:233\n\n`fn file_mtime(path: &PathBuf) -> Option<SystemTime>` takes a concrete `&PathBuf` instead of the more general `&Path`. Per Rust API guidelines, functions should accept `impl AsRef<Path>` or `&Path` so callers aren't forced to allocate a `PathBuf`. All call sites already have a `PathBuf`, but the guideline is clear: accept generics, not concrete types. This is a nit that becomes a blocker when used in public APIs — here the function is private so it's a warning-level concern.\n\nSuggestion: Change to `fn file_mtime(path: &Path) -> Option<SystemTime>`. The call sites `self.global_path.as_ref().and_then(file_mtime)` already produce `Option<&PathBuf>` and `.and_then(|p| file_mtime(p.as_path()))` or coercion via `as_ref()` handles it automatically since `PathBuf: AsRef<Path>`.\n\nVerification: `cargo clippy` will flag this. Confirm no call-site changes break.\n\n#review-finding #review-finding