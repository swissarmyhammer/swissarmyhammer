---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbe80
title: '[blocker] AttachmentRevealCmd hardcodes macOS `open -R` -- breaks on Linux/Windows'
---
**File**: `swissarmyhammer-kanban/src/commands/entity_commands.rs:241-251`\n\n**What**: `AttachmentRevealCmd` calls `std::process::Command::new(\"open\").arg(\"-R\")` which is macOS-only. On Linux this would need `xdg-open` (or `nautilus --select`), on Windows `explorer /select,`.\n\n**Why**: The Tauri app already runs cross-platform. This will crash or fail silently on non-macOS platforms. The `open` crate used by `AttachmentOpenCmd` IS cross-platform, but there is no cross-platform crate for \"reveal in file manager\".\n\n**Suggestion**: Either (a) use `#[cfg(target_os)]` to branch per platform, (b) use the `showItemInFolder` from `tauri-plugin-shell` or `tauri-plugin-opener` which handles this cross-platform, or (c) at minimum document that this is macOS-only and add a `cfg` guard that returns an informative error on other platforms." #review-finding