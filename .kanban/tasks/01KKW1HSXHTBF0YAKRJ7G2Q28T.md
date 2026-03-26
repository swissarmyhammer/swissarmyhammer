---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffd780
title: 'blocker: settings_path panics on missing home directory'
---
avp-common/src/install.rs:17-19\n\n`settings_path` calls `.expect(\"Could not find home directory\")` on the `User` scope path. This is a panic on an expected failure mode (user running in a minimal container, CI environment, or with a stripped environment). Per the Rust guidelines, panics are for bugs only — missing home directory is not a bug.\n\nSuggestion: Change `settings_path` to return `Result<PathBuf, String>` (or propagate the error) and use `dirs::home_dir().ok_or_else(|| \"Could not find home directory\".to_string())?`. Both call sites (`install` and `uninstall`) already return `Result<(), String>` so the `?` propagates cleanly.\n\nVerification: `cargo nextest run -E 'rdeps(avp-common)'` passes with no panics in a HOME-less environment." #review-finding